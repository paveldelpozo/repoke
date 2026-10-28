use std::io::{self, IsTerminal};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// Spinners go to stderr and only when it is a terminal, so pipes and `--json` output stay clean.
pub struct Progress(Option<MultiProgress>);

impl Progress {
    pub fn new(json: bool) -> Self {
        Self((!json && io::stderr().is_terminal()).then(MultiProgress::new))
    }

    pub fn spinner(&self, prefix: &str, message: &str) -> ProgressBar {
        let Some(multi) = &self.0 else {
            return ProgressBar::hidden();
        };
        let style = ProgressStyle::with_template(
            "{spinner:.cyan} {prefix:.bold} {elapsed:>3}  {wide_msg:.dim}",
        )
        .expect("valid template");
        let bar = multi.add(
            ProgressBar::new_spinner()
                .with_style(style)
                .with_prefix(format!("{prefix:<5}"))
                .with_message(message.to_string()),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        bar
    }

    /// Hides the spinners while something else needs the terminal, like a sudo password prompt.
    pub fn suspend<R>(&self, f: impl FnOnce() -> R) -> R {
        match &self.0 {
            Some(multi) => multi.suspend(f),
            None => f(),
        }
    }
}
