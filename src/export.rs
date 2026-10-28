use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use age::secrecy::SecretString;
use console::style;
use inquire::{Confirm, InquireError, Select};
use serde::Serialize;

use crate::check::{self, Found};
use crate::config::Config;
use crate::crypt::{self, Bundle};
use crate::progress::Progress;
use crate::run::{EXIT_CANCELLED, plural};
use crate::snapshot::{self, ConfigFile, Entry, FILE, FILES, Snapshot};
use crate::{ExportArgs, print_json};

#[derive(Serialize)]
struct Report {
    output: PathBuf,
    managers: Vec<ManagerReport>,
}

#[derive(Serialize)]
struct ManagerReport {
    id: &'static str,
    #[serde(skip)]
    name: &'static str,
    packages: usize,
    config: Vec<ConfigReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct ConfigReport {
    path: String,
    /// `copied`, `redacted`, `encrypted`, `dotfiles` or `left_out`.
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

enum Secrets {
    Remove,
    LeaveOut,
    Keep,
    Encrypt,
}

/// Collects the files that go into `files.age`; the password is asked the first time one does.
struct Sealer {
    all: bool,
    interactive: bool,
    password: Option<SecretString>,
    bundle: Bundle,
}

impl Sealer {
    fn password(&mut self) -> Result<SecretString, String> {
        if let Some(password) = &self.password {
            return Ok(password.clone());
        }
        let password = match crypt::from_env() {
            Some(password) => password,
            None if self.interactive => {
                crypt::ask("Password for the encrypted config files:", true)
                    .map_err(|e| e.to_string())?
            }
            None => {
                return Err(format!(
                    "set {} to encrypt the config files without a terminal",
                    crypt::ENV
                ));
            }
        };
        self.password = Some(password.clone());
        Ok(password)
    }
}

pub fn run(args: &ExportArgs, config: &Config) -> Result<ExitCode, String> {
    let interactive =
        !args.yes && !args.json && io::stdin().is_terminal() && io::stdout().is_terminal();
    let home = snapshot::home().ok_or("cannot find the home directory")?;
    let mut snapshot = Snapshot::new();
    let output = args.output.clone().unwrap_or_else(|| {
        let host = snapshot.host.as_deref().unwrap_or("machine").to_lowercase();
        PathBuf::from(format!(
            "repoke-{host}-{}",
            snapshot::date(snapshot.created)
        ))
    });
    if output.join(FILE).exists() && !args.force {
        return Err(format!(
            "{} already holds a snapshot, pass --force to replace it",
            output.display()
        ));
    }

    let mut sealer = Sealer {
        all: args.encrypt,
        interactive,
        password: None,
        bundle: Bundle::new(),
    };
    if args.encrypt && !args.no_config {
        sealer.password()?;
    }

    let progress = Progress::new(args.json);
    let listed = check::parallel(check::find(&args.managers, config)?, |found| {
        let bar = progress.spinner(found.manager.id(), "listing what is installed");
        let packages = found.manager.installed(&found.path);
        bar.finish_and_clear();
        (found, packages)
    });

    let mut reports = Vec::new();
    for (found, packages) in listed {
        let m = found.manager;
        let mut report = ManagerReport {
            id: m.id(),
            name: m.name(),
            packages: 0,
            config: Vec::new(),
            error: None,
        };
        let mut entry = Entry::default();
        match packages {
            Ok(packages) => {
                report.packages = packages.len();
                entry.packages = packages;
            }
            Err(e) => report.error = Some(e),
        }

        let present: Vec<&str> = m
            .config_files()
            .iter()
            .copied()
            .filter(|rel| home.join(rel).exists())
            .collect();
        if !present.is_empty() {
            match include_config(args, interactive, &found, &present, &home) {
                Ok(true) => {
                    for rel in present {
                        let (file, config_report) =
                            save_config(&home, &output, rel, interactive, &mut sealer)?;
                        entry.config.extend(file);
                        report.config.push(config_report);
                    }
                }
                Ok(false) => {}
                Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                    println!("Cancelled, nothing saved.");
                    return Ok(ExitCode::from(EXIT_CANCELLED));
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        if !entry.packages.is_empty() || !entry.config.is_empty() {
            snapshot.managers.insert(m.id().to_string(), entry);
        }
        reports.push(report);
    }

    snapshot.write(&output)?;
    let sealed = output.join(crypt::FILE);
    if sealer.bundle.is_empty() {
        // A snapshot replaced with --force must not keep the old one's encrypted files.
        let _ = fs::remove_file(&sealed);
    } else {
        let password = sealer.password()?;
        let data = crypt::seal(&sealer.bundle, &password)?;
        fs::write(&sealed, data).map_err(|e| format!("{}: {e}", sealed.display()))?;
    }
    let report = Report {
        output,
        managers: reports,
    };
    if args.json {
        print_json(&report);
    } else {
        print(&report);
    }
    Ok(if report.managers.iter().any(|m| m.error.is_some()) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn include_config(
    args: &ExportArgs,
    interactive: bool,
    found: &Found,
    present: &[&str],
    home: &Path,
) -> Result<bool, InquireError> {
    if args.no_config {
        return Ok(false);
    }
    if args.config || args.encrypt {
        return Ok(true);
    }
    if !interactive {
        return Ok(false);
    }
    let files: Vec<String> = present
        .iter()
        .map(|rel| snapshot::tilde(&home.join(rel), home))
        .collect();
    Confirm::new(&format!(
        "Include the {} config ({})?",
        found.manager.name(),
        files.join(", ")
    ))
    .with_default(true)
    .prompt()
}

/// Copies one config into `files/` or the sealer, or records where it comes from when it is a
/// dotfiles link.
fn save_config(
    home: &Path,
    output: &Path,
    rel: &str,
    interactive: bool,
    sealer: &mut Sealer,
) -> Result<(Option<ConfigFile>, ConfigReport), String> {
    let source = home.join(rel);
    let shown = snapshot::tilde(&source, home);
    let report = |result, detail: Option<String>| ConfigReport {
        path: shown.clone(),
        result,
        detail,
    };

    if let Some(dotfiles) = snapshot::dotfiles_link(&source, home) {
        let detail = Some(format!("link to {}", dotfiles.target));
        let file = ConfigFile {
            path: rel.to_string(),
            redacted: false,
            encrypted: false,
            dotfiles: Some(dotfiles),
        };
        return Ok((Some(file), report("dotfiles", detail)));
    }

    let Ok(text) = fs::read_to_string(&source) else {
        return Ok((None, report("left_out", Some("not a text file".into()))));
    };
    let secrets = snapshot::secret_lines(&text);
    let choice = if secrets.is_empty() {
        if sealer.all {
            Secrets::Encrypt
        } else {
            Secrets::Keep
        }
    } else if interactive {
        let lines: Vec<String> = secrets.iter().map(|i| (i + 1).to_string()).collect();
        let options = if sealer.all {
            vec![
                "Keep them, encrypted (recommended)",
                "Remove their values",
                "Leave this file out",
            ]
        } else {
            vec![
                "Remove their values (recommended)",
                "Keep them, encrypted with a password",
                "Leave this file out",
                "Keep them as they are",
            ]
        };
        let picked = Select::new(
            &format!(
                "{shown} has {} that look{} like secrets (line {}). What should be saved?",
                plural(secrets.len(), "line"),
                if secrets.len() == 1 { "s" } else { "" },
                lines.join(", ")
            ),
            options,
        )
        .prompt()
        .map_err(|e| e.to_string())?;
        match picked {
            "Leave this file out" => Secrets::LeaveOut,
            "Keep them as they are" => Secrets::Keep,
            "Keep them, encrypted (recommended)" | "Keep them, encrypted with a password" => {
                Secrets::Encrypt
            }
            _ => Secrets::Remove,
        }
    } else if sealer.all {
        Secrets::Encrypt
    } else {
        Secrets::Remove
    };

    let (text, redacted) = match choice {
        Secrets::LeaveOut => {
            return Ok((None, report("left_out", Some("holds secrets".into()))));
        }
        Secrets::Remove => (snapshot::redact(&text), true),
        Secrets::Keep | Secrets::Encrypt => (text, false),
    };

    if matches!(choice, Secrets::Encrypt) || sealer.all {
        sealer.password()?;
        sealer.bundle.insert(rel.to_string(), text);
        let file = ConfigFile {
            path: rel.to_string(),
            redacted,
            encrypted: true,
            dotfiles: None,
        };
        let detail = redacted.then(|| format!("{} removed", plural(secrets.len(), "secret")));
        return Ok((Some(file), report("encrypted", detail)));
    }

    let target = output.join(FILES).join(rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
    // Same permissions as the original: an .npmrc is often readable by its owner only.
    if let Ok(meta) = fs::metadata(&source) {
        let _ = fs::set_permissions(&target, meta.permissions());
    }

    let file = ConfigFile {
        path: rel.to_string(),
        redacted,
        encrypted: false,
        dotfiles: None,
    };
    Ok(if redacted {
        let detail = format!("{} removed", plural(secrets.len(), "secret"));
        (Some(file), report("redacted", Some(detail)))
    } else {
        (Some(file), report("copied", None))
    })
}

fn print(report: &Report) {
    let width = report
        .managers
        .iter()
        .map(|m| m.name.len())
        .max()
        .unwrap_or(0);
    println!(
        "\n{} {}",
        style("Snapshot saved to").bold(),
        style(report.output.display()).bold()
    );
    for m in &report.managers {
        match &m.error {
            Some(e) => println!(
                "  {} {:<width$}  {}",
                style("-").yellow(),
                m.name,
                style(format!("skipped: {}", e.lines().next().unwrap_or_default())).yellow()
            ),
            None => println!(
                "  {} {:<width$}  {}",
                style("✓").green(),
                m.name,
                plural(m.packages, "package")
            ),
        }
        for c in &m.config {
            let mark = match c.result {
                "left_out" => style("-").yellow(),
                "redacted" => style("!").yellow(),
                _ => style("✓").green(),
            };
            let detail = c
                .detail
                .as_deref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default();
            let what = match c.result {
                "dotfiles" => "recorded, comes from your dotfiles",
                "left_out" => "left out",
                "encrypted" => "saved, encrypted",
                _ => "saved",
            };
            println!("      {mark} {} {what}{}", c.path, style(detail).dim());
        }
    }
    println!(
        "\nRestore it on another machine with: {}",
        style(format!("repoke init --from {}", report.output.display())).bold()
    );
}
