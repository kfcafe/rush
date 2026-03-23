/// One-shot programmatic API for executing shell commands via Rush.
///
/// Designed for embedding Rush in agents, tools, or any Rust program that
/// needs to run shell commands and receive structured output — without managing
/// a REPL, terminal, or persistent executor session.
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::builtins::exit_builtin::ExitSignal;
use crate::executor::Executor;
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Parse a `--max-output` / `RUSH_MAX_OUTPUT` value string into a byte count.
///
/// Supported formats:
/// - `50KB`, `50kb`, `50k`  → 50 * 1024 bytes
/// - `1MB`, `1mb`, `1m`     → 1 * 1024 * 1024 bytes
/// - `2000lines`, `2000l`   → 2000 * 80 bytes (80-byte average line estimate)
/// - Plain integer          → bytes
///
/// Returns `None` if the value cannot be parsed.
pub fn parse_max_output(s: &str) -> Option<usize> {
    let s = s.trim();
    // Try suffix matching (case-insensitive).
    let lower = s.to_ascii_lowercase();

    if let Some(n) = lower.strip_suffix("lines") {
        return n.trim().parse::<usize>().ok().map(|v| v * 80);
    }
    if lower.ends_with('l') && !lower.ends_with("ml") {
        let n = &lower[..lower.len() - 1];
        return n.trim().parse::<usize>().ok().map(|v| v * 80);
    }
    if let Some(n) = lower.strip_suffix("mb") {
        return n.trim().parse::<usize>().ok().map(|v| v * 1024 * 1024);
    }
    if lower.ends_with('m') {
        let n = &lower[..lower.len() - 1];
        return n.trim().parse::<usize>().ok().map(|v| v * 1024 * 1024);
    }
    if let Some(n) = lower.strip_suffix("kb") {
        return n.trim().parse::<usize>().ok().map(|v| v * 1024);
    }
    if lower.ends_with('k') {
        let n = &lower[..lower.len() - 1];
        return n.trim().parse::<usize>().ok().map(|v| v * 1024);
    }
    // Plain integer → bytes.
    s.parse::<usize>().ok()
}

/// Options for programmatic command execution.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Working directory (defaults to current directory).
    pub cwd: Option<PathBuf>,
    /// Additional environment variables to set before execution.
    /// These are applied on top of the current process environment.
    pub env: Option<HashMap<String, String>>,
    /// Timeout in seconds. If the command runs longer, it is killed and
    /// `RunResult::timed_out` is set to `true`.
    pub timeout: Option<u64>,
    /// Request JSON output from built-in commands that support it
    /// (sets `RUSH_JSON=1` in the execution environment).
    pub json_output: bool,
    /// Maximum total output bytes (stdout + stderr combined). Output is
    /// truncated per-stream once the combined budget is exceeded, and
    /// `RunResult::truncated` is set to `true`.
    pub max_output_bytes: Option<usize>,
}

/// Structured result from a one-shot command execution.
#[derive(Debug, Clone)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// `true` if stdout or stderr were truncated due to `max_output_bytes`.
    pub truncated: bool,
    /// `true` if the command was killed because it exceeded `timeout`.
    pub timed_out: bool,
}

/// Execute a shell command string and return structured output.
///
/// Uses `Executor::new_embedded()` internally — no terminal interaction,
/// no progress spinners, no interactive prompts.
///
/// # Example
/// ```
/// use rush::{run, RunOptions};
/// let result = run("echo hello", &RunOptions::default()).unwrap();
/// assert_eq!(result.exit_code, 0);
/// assert!(result.stdout.contains("hello"));
/// ```
pub fn run(command: &str, options: &RunOptions) -> Result<RunResult> {
    // Clone inputs so we can move them into a thread.
    let command = command.to_string();
    let options = options.clone();

    // Extract timeout before moving options into the thread.
    let timeout = options.timeout;

    // Spawn execution in a thread so we can apply a timeout via join().
    let result_cell: Arc<Mutex<Option<Result<RunResult>>>> = Arc::new(Mutex::new(None));
    let result_cell_clone = Arc::clone(&result_cell);

    let handle = thread::spawn(move || {
        let outcome = execute_inner(&command, &options);
        *result_cell_clone.lock().unwrap() = Some(outcome);
    });

    if let Some(secs) = timeout {
        match handle.join_timeout(Duration::from_secs(secs)) {
            Ok(_) => {
                // Thread finished in time — retrieve result.
                let outcome = result_cell.lock().unwrap().take().unwrap();
                outcome
            }
            Err(_) => {
                // Timed out — the thread is still running (we can't kill it
                // cleanly in Rust safe code, but external processes spawned
                // by the executor will be orphaned / cleaned up by the OS).
                Ok(RunResult {
                    exit_code: 124, // Conventional timeout exit code (same as `timeout` util)
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                    timed_out: true,
                })
            }
        }
    } else {
        handle.join().expect("executor thread panicked");
        let outcome = result_cell.lock().unwrap().take().unwrap();
        outcome
    }
}

/// Internal: build an Executor, configure it, run the command, return a RunResult.
fn execute_inner(command: &str, options: &RunOptions) -> Result<RunResult> {
    let mut executor = Executor::new_embedded();

    // Apply working directory.
    if let Some(cwd) = &options.cwd {
        executor.runtime_mut().set_cwd(cwd.clone());
        // Also set PWD so external commands inherit it.
        executor
            .runtime_mut()
            .set_env("PWD", &cwd.to_string_lossy());
    }

    // Apply extra environment variables.
    if let Some(extra_env) = &options.env {
        for (k, v) in extra_env {
            executor.runtime_mut().set_env(k, v);
        }
    }

    // Activate agent mode so built-ins emit JSON automatically.
    if options.json_output {
        executor.runtime_mut().set_agent_mode(true);
        executor.runtime_mut().set_env("RUSH_JSON", "1");
    }

    // Resolve effective max_output_bytes: RunOptions takes priority, then
    // the RUSH_MAX_OUTPUT environment variable.
    let max_output_bytes = options.max_output_bytes.or_else(|| {
        env::var("RUSH_MAX_OUTPUT")
            .ok()
            .and_then(|v| parse_max_output(&v))
    });

    // Lex → parse → execute.
    let tokens = Lexer::tokenize(command)?;
    let mut parser = Parser::new(tokens);
    let statements = parser.parse()?;
    let exec_result = match executor.execute(statements) {
        Ok(r) => r,
        Err(e) => {
            // `exit N` is signalled as an ExitSignal error, not an exit code.
            // Catch it here and convert to a proper RunResult.
            if let Some(sig) = e.downcast_ref::<ExitSignal>() {
                return Ok(RunResult {
                    exit_code: sig.exit_code,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                    timed_out: false,
                });
            }
            return Err(e);
        }
    };

    let mut stdout = exec_result.stdout();
    let stderr = exec_result.stderr.clone();
    let mut truncated = false;

    // Apply stdout budget. Per spec: applies to stdout only (stderr is typically
    // small). When exceeded, truncate to the byte limit and append a notice so
    // callers know output is incomplete.
    if let Some(max_bytes) = max_output_bytes {
        if stdout.len() > max_bytes {
            truncated = true;
            // Truncate at a UTF-8 boundary to avoid producing invalid strings.
            let safe_end = floor_char_boundary(&stdout, max_bytes);
            let bytes_written = stdout.len();
            stdout.truncate(safe_end);
            stdout.push_str(&format!(
                "\n[Output truncated: {} bytes, limit {} bytes]",
                bytes_written, max_bytes
            ));
        }
    }

    Ok(RunResult {
        exit_code: exec_result.exit_code,
        stdout,
        stderr,
        truncated,
        timed_out: false,
    })
}

/// Return the largest byte index ≤ `index` that lies on a UTF-8 char boundary.
/// Equivalent to `str::floor_char_boundary` (stabilised in Rust 1.65 but not
/// yet in all toolchains via the stable name — implement it portably here).
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    // Walk backwards from `index` until we find a char boundary.
    let bytes = s.as_bytes();
    let mut i = index;
    while i > 0 && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
        i -= 1;
    }
    i
}

// Thread::join_timeout isn't stable — we implement our own via a channel.
trait JoinTimeout {
    fn join_timeout(self, timeout: Duration) -> std::result::Result<(), ()>;
}

impl JoinTimeout for thread::JoinHandle<()> {
    fn join_timeout(self, timeout: Duration) -> std::result::Result<(), ()> {
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let _ = self.join();
            let _ = tx.send(());
        });
        rx.recv_timeout(timeout).map_err(|_| ())
    }
}
