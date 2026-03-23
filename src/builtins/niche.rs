//! Niche bash builtins: `let`, `bind`, `suspend`, `enable`, `select`, `coproc`, `newgrp`, `logout`.
//!
//! These are rarely needed in practice but are expected by scripts and users
//! migrating from bash. Some are fully functional (let, suspend, logout);
//! others are intentional stubs with clear error messages explaining why
//! they cannot be supported or are deferred.

use crate::arithmetic;
use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

// ---------------------------------------------------------------------------
// let
// ---------------------------------------------------------------------------

/// Arithmetic evaluation builtin.
///
/// Usage: let EXPR [EXPR...]
///
/// Each EXPR is an arithmetic expression, optionally of the form `NAME=EXPR`
/// to assign the result to a variable. Evaluates each expression and returns
/// exit code 0 if the final result is non-zero, 1 if it is zero (matching
/// bash semantics — lets `let` be used as a condition).
///
/// Rush already has `$(( ))` arithmetic expansion; `let` is syntactic sugar
/// for the same engine.
pub fn builtin_let(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "let: usage: let EXPR [EXPR...]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let mut last_value: i64 = 0;

    for arg in args {
        // Assignment form: NAME=EXPR or NAME+=EXPR etc. — pass the whole thing to
        // the arithmetic evaluator which handles assignment operators.
        let value =
            arithmetic::evaluate_mut(arg, runtime).map_err(|e| anyhow!("let: {}: {}", arg, e))?;
        last_value = value;
    }

    // Exit 0 if the last value is non-zero (true), 1 if zero (false) — same as bash.
    let exit_code = if last_value != 0 { 0 } else { 1 };
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// bind
// ---------------------------------------------------------------------------

/// Key-binding builtin (bash readline).
///
/// Rush uses reedline for line editing, which has its own keybinding system
/// configured separately. This stub exists for script compatibility — scripts
/// that call `bind` won't fail, but the bindings have no effect.
pub fn builtin_bind(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    // -l lists all readline function names; -v lists current bindings.
    // Return empty output so scripts testing `bind -v` don't hard-fail.
    if args.iter().any(|a| a == "-l" || a == "-v" || a == "-p") {
        return Ok(ExecutionResult::success(String::new()));
    }

    // For anything else (e.g. bind '"\t": complete') silently succeed.
    // Reedline handles keybindings through its own config; bash bind calls
    // are a no-op in Rush.
    Ok(ExecutionResult::success(String::new()))
}

// ---------------------------------------------------------------------------
// suspend
// ---------------------------------------------------------------------------

/// Suspend the shell by sending SIGSTOP to the shell process.
///
/// Usage: suspend [-f]
///
/// Pauses the shell until it receives SIGCONT from a parent process (e.g. `fg`
/// in the parent shell). `-f` forces suspension even in a login shell.
pub fn builtin_suspend(_args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Send SIGSTOP to ourselves. The OS will pause us until SIGCONT arrives.
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        signal::kill(Pid::this(), Signal::SIGSTOP)
            .map_err(|e| anyhow!("suspend: failed to send SIGSTOP: {}", e))?;
    }
    #[cfg(not(unix))]
    {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "suspend: not supported on this platform\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }
    Ok(ExecutionResult::success(String::new()))
}

// ---------------------------------------------------------------------------
// enable
// ---------------------------------------------------------------------------

/// Enable or disable shell builtins.
///
/// Usage: enable [-n] [-a] [NAME...]
///
/// In bash this can shadow external commands by enabling builtins or load
/// dynamic builtin modules. Rush's builtin table is static (compiled in),
/// so this is a forward-compatible stub: `-n` (disable) and plain enable
/// are accepted without error so scripts don't fail, but no builtins are
/// actually toggled. Use this as a foundation when a dynamic builtin plugin
/// system is added later.
pub fn builtin_enable(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut disable = false;
    let mut list_all = false;
    let mut names: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-n" => disable = true,
            "-a" => list_all = true,
            "-p" => { /* print form — handled below */ }
            "-s" => { /* special builtins filter — ignored */ }
            arg if arg.starts_with('-') => {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: format!("enable: {}: invalid option\n", arg),
                    exit_code: 1,
                    error: None,
                });
            }
            name => names.push(name),
        }
        i += 1;
    }

    // List all builtins (or just enabled/disabled ones)
    if list_all || names.is_empty() {
        let mut output = String::new();
        let b = super::Builtins::new();
        let mut sorted = b.builtin_names();
        sorted.sort();
        for name in &sorted {
            // In Rush all builtins are always enabled; mirror bash's output format.
            output.push_str(&format!("enable {}\n", name));
        }
        return Ok(ExecutionResult::success(output));
    }

    // Acknowledge the request but note that Rush has a static builtin table.
    // Scripts calling `enable -n something` to disable a builtin won't get the
    // actual disable effect, but they also won't fail with an error.
    let _ = disable;
    let b = super::Builtins::new();
    for name in &names {
        // Warn only for unknown names so scripts can detect typos.
        if !b.is_builtin(name) {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("enable: {}: not a shell builtin\n", name),
                exit_code: 1,
                error: None,
            });
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

// ---------------------------------------------------------------------------
// select
// ---------------------------------------------------------------------------

/// Interactive menu-selection loop.
///
/// Usage: select NAME [in WORD...]; do COMMANDS; done
///
/// `select` is a compound statement (parsed as a loop construct) rather than
/// a simple builtin in a full implementation. This stub exists so that
/// `select` appearing as a standalone command (not in a loop context) returns
/// a meaningful error instead of "command not found".
pub fn builtin_select(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let _ = args;
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr:
            "select: must be used as a compound statement (select NAME in WORDS; do ...; done)\n"
                .to_string(),
        exit_code: 1,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// coproc
// ---------------------------------------------------------------------------

/// Coprocess creation (bidirectional pipe to a background command).
///
/// Usage: coproc [NAME] command [redirections]
///
/// Creates a coprocess: spawns `command` with two pipes so the shell can
/// read its stdout and write to its stdin via `${NAME[0]}` and `${NAME[1]}`.
/// This requires deep integration with the parser and file-descriptor table.
///
/// This stub ensures `coproc` appearing as a bare command gives a clear
/// error rather than "command not found".
pub fn builtin_coproc(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let _ = args;
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: "coproc: not yet implemented — use `command &` with explicit pipes for now\n"
            .to_string(),
        exit_code: 1,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// newgrp
// ---------------------------------------------------------------------------

/// Change the effective group ID of the shell.
///
/// Usage: newgrp [GROUP]
///
/// POSIX requires `newgrp` to replace the shell with a new shell running
/// under the specified group. This cannot be implemented as a builtin that
/// modifies only the shell's own GID without exec'ing; the canonical
/// implementation calls `exec newgrp` to the external binary.
pub fn builtin_newgrp(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Delegate to the external `newgrp` binary (setuid root on most systems).
    let mut cmd = std::process::Command::new("newgrp");
    if !args.is_empty() {
        cmd.args(args);
    }
    match cmd.status() {
        Ok(status) => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: String::new(),
            exit_code: status.code().unwrap_or(1),
            error: None,
        }),
        Err(e) => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("newgrp: {}\n", e),
            exit_code: 1,
            error: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// logout
// ---------------------------------------------------------------------------

/// Exit a login shell.
///
/// Usage: logout [N]
///
/// Identical to `exit` — terminates the shell with exit code N (default 0).
/// In bash, `logout` only works in a login shell; Rush accepts it everywhere
/// for simplicity (matching zsh behavior).
pub fn builtin_logout(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    super::exit_builtin::builtin_exit(args, runtime)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> Runtime {
        Runtime::new()
    }

    // --- let ---

    #[test]
    fn let_nonzero_exits_0() {
        let mut r = rt();
        let res = builtin_let(&["1+1".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
    }

    #[test]
    fn let_zero_exits_1() {
        let mut r = rt();
        let res = builtin_let(&["0".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
    }

    #[test]
    fn let_no_args_exits_1() {
        let mut r = rt();
        let res = builtin_let(&[], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
    }

    #[test]
    fn let_multiple_exprs_uses_last() {
        let mut r = rt();
        // First expr is non-zero, second is zero — exit should be 1 (last is zero)
        let res = builtin_let(&["1".to_string(), "0".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
    }

    // --- bind ---

    #[test]
    fn bind_list_succeeds() {
        let mut r = rt();
        let res = builtin_bind(&["-l".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
    }

    #[test]
    fn bind_silent_no_args() {
        let mut r = rt();
        let res = builtin_bind(&[], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
    }

    // --- enable ---

    #[test]
    fn enable_no_args_lists_builtins() {
        let mut r = rt();
        let res = builtin_enable(&[], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout().contains("enable "));
    }

    #[test]
    fn enable_known_builtin_succeeds() {
        let mut r = rt();
        let res = builtin_enable(&["echo".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
    }

    #[test]
    fn enable_unknown_builtin_fails() {
        let mut r = rt();
        let res = builtin_enable(&["notabuiltin123".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
    }

    #[test]
    fn enable_n_flag_known_builtin_succeeds() {
        let mut r = rt();
        let res = builtin_enable(&["-n".to_string(), "echo".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 0);
    }

    // --- select ---

    #[test]
    fn select_returns_error() {
        let mut r = rt();
        let res = builtin_select(&["x".to_string()], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
        assert!(res.stderr.contains("compound statement"));
    }

    // --- coproc ---

    #[test]
    fn coproc_returns_not_implemented() {
        let mut r = rt();
        let res = builtin_coproc(&[], &mut r).unwrap();
        assert_eq!(res.exit_code, 1);
        assert!(res.stderr.contains("not yet implemented"));
    }

    // --- logout ---

    #[test]
    fn logout_exits_with_signal() {
        let mut r = rt();
        let res = builtin_logout(&[], &mut r);
        // logout delegates to exit which returns an Err(ExitSignal)
        assert!(res.is_err());
    }

    #[test]
    fn logout_exits_with_code() {
        let mut r = rt();
        let res = builtin_logout(&["3".to_string()], &mut r);
        assert!(res.is_err());
        let code = res
            .unwrap_err()
            .downcast::<crate::builtins::exit_builtin::ExitSignal>()
            .unwrap()
            .exit_code;
        assert_eq!(code, 3);
    }
}
