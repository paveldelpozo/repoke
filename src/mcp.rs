//! `repoke mcp`: a Model Context Protocol server over stdio, so agents can check and upgrade
//! through tools. Each tool runs this same binary with `--json` and returns what it printed.

use std::env;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::manager::capture;

/// Newest first; the client's version is echoed back when it is one of these.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn serve() -> Result<ExitCode, String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = handle(&line) else {
            continue;
        };
        writeln!(stdout, "{response}").map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The response to one JSON-RPC message, or none for a notification.
fn handle(line: &str) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let id = message.get("id").cloned()?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => initialize(&params),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => call(&params),
        method => return Some(error(id, -32601, &format!("unknown method {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or_default();
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == asked)
        .unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "repoke", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Check what is outdated with check_updates before upgrading. upgrade only previews unless dry_run is false, and never uses sudo: managers that need it are skipped and the user has to run `repoke upgrade` in a terminal."
    })
}

fn selection_properties() -> Value {
    let list = |description: &str| json!({ "type": "array", "items": { "type": "string" }, "description": description });
    json!({
        "only": list("Only these managers, by id (see list_managers)"),
        "exclude": list("Skip these managers"),
        "hold": list("Leave these packages alone, as name or manager:name"),
    })
}

fn tools() -> Value {
    let mut check = selection_properties();
    check["refresh"] = json!({ "type": "boolean", "description": "Refresh package indexes first (brew update); slower" });
    let mut upgrade = selection_properties();
    upgrade["packages"] = json!({
        "type": "array",
        "items": { "type": "string" },
        "description": "Only these packages, as name or manager:name"
    });
    upgrade["dry_run"] = json!({
        "type": "boolean",
        "default": true,
        "description": "Only list the commands that would run. Pass false to actually upgrade, after the user agreed"
    });
    json!([
        {
            "name": "list_managers",
            "description": "Package managers repoke supports on this machine, whether each is installed and its version.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "check_updates",
            "description": "Pending updates per package manager (current and latest version), without changing anything.",
            "inputSchema": { "type": "object", "properties": check },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "upgrade",
            "description": "Upgrades pending packages and reports each result. Previews by default; pass dry_run false to change the system. Managers that need sudo are skipped.",
            "inputSchema": { "type": "object", "properties": upgrade },
            "annotations": { "destructiveHint": true, "readOnlyHint": false }
        },
        {
            "name": "schedule_status",
            "description": "Whether a periodic run is scheduled, and how the last one went.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        }
    ])
}

fn call(params: &Value) -> Value {
    let arguments = &params["arguments"];
    let mut args: Vec<String> = match params["name"].as_str().unwrap_or_default() {
        "list_managers" => vec!["list".into(), "--json".into()],
        "check_updates" => {
            let mut args = vec!["check".into(), "--json".into()];
            if arguments["refresh"].as_bool() == Some(true) {
                args.push("--refresh".into());
            }
            args
        }
        "upgrade" => {
            let mut args = vec![
                "upgrade".into(),
                "--yes".into(),
                "--no-sudo".into(),
                "--json".into(),
            ];
            if arguments["dry_run"].as_bool() != Some(false) {
                args.push("--dry-run".into());
            }
            push_list(&mut args, "--package", &arguments["packages"]);
            args
        }
        "schedule_status" => vec!["schedule".into(), "status".into(), "--json".into()],
        name => return tool_error(&format!("unknown tool {name}")),
    };
    if args[0] == "check" || args[0] == "upgrade" {
        push_list(&mut args, "--only", &arguments["only"]);
        push_list(&mut args, "--exclude", &arguments["exclude"]);
        push_list(&mut args, "--hold", &arguments["hold"]);
    }

    let exe = match env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return tool_error(&format!("cannot find repoke's own path: {e}")),
    };
    let out = match capture(&exe, &args) {
        Ok(out) => out,
        Err(e) => return tool_error(&e),
    };
    let code = out.code.unwrap_or(1);
    let meaning = match (args[0].as_str(), code) {
        ("check", 100) => "updates are available",
        (_, 0) => "ok",
        (_, 1) => "a manager failed, see its error",
        _ => "repoke could not run",
    };
    let mut text = format!(
        "$ repoke {}\nexit {code}: {meaning}\n{}",
        args.join(" "),
        out.stdout
    );
    if !out.stderr.trim().is_empty() {
        text.push_str(&format!("\nstderr:\n{}", out.stderr.trim()));
    }
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": code != 0 && code != 100
    })
}

fn push_list(args: &mut Vec<String>, flag: &str, value: &Value) {
    let items: Vec<&str> = value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !items.is_empty() {
        args.extend([flag.to_string(), items.join(",")]);
    }
}

fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_the_protocol_version() {
        let response = handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#).unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2025-03-26");
        let response = handle(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).unwrap();
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_answer() {
        assert!(handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    }

    #[test]
    fn lists_tools_and_rejects_unknown_methods() {
        let response = handle(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#).unwrap();
        let names: Vec<&str> = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "list_managers",
                "check_updates",
                "upgrade",
                "schedule_status"
            ]
        );
        let response = handle(r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn unknown_tools_are_tool_errors() {
        let response = call(&json!({ "name": "rm", "arguments": {} }));
        assert_eq!(response["isError"], true);
    }

    #[test]
    fn joins_list_arguments() {
        let mut args = Vec::new();
        push_list(&mut args, "--only", &json!(["brew", "npm"]));
        push_list(&mut args, "--hold", &json!([]));
        assert_eq!(args, ["--only", "brew,npm"]);
    }
}
