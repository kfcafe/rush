use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::io::IsTerminal;

/// Implement the `status` builtin (fish-style).
///
/// Usage:
///   status                  — print a shell status summary
///   status is-interactive   — exit 0 if interactive, 1 otherwise
///   status is-login         — exit 0 if login shell, 1 otherwise
///   status filename         — print the current script filename ($0)
///   status line-number      — print the current line number (always 0 in rush)
pub fn builtin_status(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let subcommand = args.first().map(|s| s.as_str()).unwrap_or("");

    match subcommand {
        "" => {
            // Print a brief summary of the shell status.
            let interactive = is_interactive();
            let login = is_login();
            let filename = runtime.get_variable("0").unwrap_or_else(|| "rush".to_string());

            let mut out = String::new();
            out.push_str(&format!(
                "This is a {}{}shell\n",
                if login { "login, " } else { "" },
                if interactive { "interactive " } else { "non-interactive " },
            ));
            out.push_str(&format!("Current script: {}\n", filename));
            Ok(ExecutionResult::success(out))
        }

        "is-interactive" => {
            let code = if is_interactive() { 0 } else { 1 };
            Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: String::new(),
                exit_code: code,
                error: None,
            })
        }

        "is-login" => {
            let code = if is_login() { 0 } else { 1 };
            Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: String::new(),
                exit_code: code,
                error: None,
            })
        }

        "filename" => {
            let name = runtime.get_variable("0").unwrap_or_else(|| "rush".to_string());
            Ok(ExecutionResult::success(format!("{}\n", name)))
        }

        "line-number" => {
            // Rush does not track line numbers at runtime; return 0 as a safe default.
            Ok(ExecutionResult::success("0\n".to_string()))
        }

        other => Err(anyhow!("status: unknown subcommand: {}", other)),
    }
}

/// Returns true when stdin is a terminal (interactive session).
fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
}

/// Returns true when the shell was started as a login shell.
/// Checks process argv[0] for a leading '-' (POSIX convention) or the
/// RUSH_LOGIN environment variable.
fn is_login() -> bool {
    std::env::args()
        .next()
        .map(|a| a.starts_with('-'))
        .unwrap_or(false)
        || std::env::var("RUSH_LOGIN").map(|v| v == "1").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_status_filename() {
        let mut runtime = Runtime::new();
        runtime.set_variable("0".to_string(), "myscript.sh".to_string());
        let result = builtin_status(&["filename".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "myscript.sh\n");
    }

    #[test]
    fn test_status_line_number() {
        let mut runtime = Runtime::new();
        let result = builtin_status(&["line-number".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "0\n");
    }

    #[test]
    fn test_status_unknown_subcommand() {
        let mut runtime = Runtime::new();
        let result = builtin_status(&["bogus".to_string()], &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_status_no_args() {
        let mut runtime = Runtime::new();
        // Should not panic and should produce some output.
        let result = builtin_status(&[], &mut runtime).unwrap();
        assert!(!result.stdout().is_empty());
    }

    #[test]
    fn test_status_is_interactive_returns_valid_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_status(&["is-interactive".to_string()], &mut runtime).unwrap();
        // Exit code must be 0 or 1 — both are valid depending on the test environment.
        assert!(result.exit_code == 0 || result.exit_code == 1);
    }

    #[test]
    fn test_status_is_login_returns_valid_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_status(&["is-login".to_string()], &mut runtime).unwrap();
        assert!(result.exit_code == 0 || result.exit_code == 1);
    }
}
