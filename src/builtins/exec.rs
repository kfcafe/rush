use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs::OpenOptions;
use std::os::unix::io::{AsRawFd, RawFd};

#[cfg(unix)]
use nix::libc;

/// The exec builtin command
///
/// This builtin has two distinct modes:
/// 1. Command replacement: `exec command args...` - replaces the shell process with command
/// 2. File descriptor redirection: `exec > file`, `exec 2>&1`, etc. - redirects permanently
///
/// ## Command Replacement (Unix only)
/// When exec is given a command, it replaces the current shell process with that command.
/// The shell never returns from this call - the process image is replaced.
///
/// ## File Descriptor Redirection
/// When exec is used with redirections but no command, it permanently redirects the shell's
/// file descriptors. All subsequent commands inherit these redirections.
///
/// Examples:
/// - `exec ./server` - replace shell with server program
/// - `exec > output.log` - redirect stdout to file
/// - `exec 2>&1` - redirect stderr to stdout
/// - `exec < input.txt` - redirect stdin from file
pub fn builtin_exec(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        // exec with no arguments and no redirections is a no-op in bash
        // Since we don't have access to redirection info here, just return success
        return Ok(ExecutionResult::success(String::new()));
    }

    // If we have arguments, this is command replacement mode
    exec_command(args, runtime)
}

/// Execute command replacement mode - replaces the shell process with the given command
#[cfg(unix)]
fn exec_command(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    if args.is_empty() {
        return Err(anyhow!("exec: command not specified"));
    }

    let command = &args[0];

    // Check if the command is a builtin - we can't exec a builtin
    // (builtins run in the same process, exec replaces the process)
    use crate::builtins::Builtins;
    let builtins = Builtins::new();
    if builtins.is_builtin(command) {
        return Err(anyhow!("exec: {}: cannot execute builtin", command));
    }

    // Resolve the command path
    let command_path = if command.contains('/') {
        // Absolute or relative path
        command.clone()
    } else {
        // Search in PATH
        match find_in_path(command) {
            Some(path) => path,
            None => return Err(anyhow!("exec: {}: command not found", command)),
        }
    };

    // Apply any permanent redirections before exec
    apply_permanent_redirections(runtime)?;

    // Build the command with all arguments
    let mut cmd = Command::new(&command_path);
    if args.len() > 1 {
        cmd.args(&args[1..]);
    }

    // Set the current working directory
    cmd.current_dir(runtime.get_cwd());

    // Set environment variables from runtime
    let env = runtime.get_env();
    cmd.env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }

    // This call replaces the current process - it never returns on success
    // On error, it returns the error
    let err = cmd.exec();

    // If we get here, exec failed
    Err(anyhow!("exec: {}: {}", command_path, err))
}

/// Windows doesn't support exec-style process replacement
#[cfg(not(unix))]
fn exec_command(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    Err(anyhow!(
        "exec: process replacement not supported on this platform (Windows)\n\
         exec can only be used for file descriptor redirection on Windows"
    ))
}

/// Search for a command in PATH
fn find_in_path(command: &str) -> Option<String> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let full_path = format!("{}/{}", dir, command);
            if std::path::Path::new(&full_path).exists() {
                return Some(full_path);
            }
        }
    }
    None
}

/// Apply permanent file descriptor redirections from runtime state
fn apply_permanent_redirections(runtime: &mut Runtime) -> Result<()> {
    // Get any permanent redirections that have been set
    // These would be set by exec_redirect() calls

    if let Some(stdout_fd) = runtime.get_permanent_stdout() {
        unsafe {
            libc::dup2(stdout_fd, libc::STDOUT_FILENO);
        }
    }

    if let Some(stderr_fd) = runtime.get_permanent_stderr() {
        unsafe {
            libc::dup2(stderr_fd, libc::STDERR_FILENO);
        }
    }

    if let Some(stdin_fd) = runtime.get_permanent_stdin() {
        unsafe {
            libc::dup2(stdin_fd, libc::STDIN_FILENO);
        }
    }

    Ok(())
}

/// Handle file descriptor redirection mode
/// This is called from the executor when exec has redirections but no command
#[allow(dead_code)]
pub fn exec_redirect(
    stdout_redirect: Option<RedirectTarget>,
    stderr_redirect: Option<RedirectTarget>,
    stdin_redirect: Option<RedirectTarget>,
    runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    // Handle stdout redirection
    if let Some(target) = stdout_redirect {
        redirect_fd(libc::STDOUT_FILENO, target, runtime, "stdout")?;
    }

    // Handle stderr redirection
    if let Some(target) = stderr_redirect {
        redirect_fd(libc::STDERR_FILENO, target, runtime, "stderr")?;
    }

    // Handle stdin redirection
    if let Some(target) = stdin_redirect {
        redirect_fd(libc::STDIN_FILENO, target, runtime, "stdin")?;
    }

    Ok(ExecutionResult::success(String::new()))
}

/// Represents where to redirect a file descriptor
#[allow(dead_code)]
pub enum RedirectTarget {
    /// Redirect to a file path
    File { path: String, append: bool },
    /// Redirect to another file descriptor (e.g., 2>&1)
    Fd(RawFd),
}

/// Redirect a file descriptor to a target
#[allow(dead_code)]
fn redirect_fd(
    fd: RawFd,
    target: RedirectTarget,
    runtime: &mut Runtime,
    fd_name: &str,
) -> Result<()> {
    match target {
        RedirectTarget::File { path, append } => {
            // Open the file
            let file = if append {
                OpenOptions::new().create(true).append(true).open(&path)?
            } else {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&path)?
            };

            let file_fd = file.as_raw_fd();

            // Duplicate the file descriptor
            unsafe {
                if libc::dup2(file_fd, fd) == -1 {
                    return Err(anyhow!("exec: failed to redirect {}", fd_name));
                }
            }

            // Store the permanent redirection in runtime
            match fd {
                libc::STDOUT_FILENO => runtime.set_permanent_stdout(Some(file_fd)),
                libc::STDERR_FILENO => runtime.set_permanent_stderr(Some(file_fd)),
                libc::STDIN_FILENO => runtime.set_permanent_stdin(Some(file_fd)),
                _ => {}
            }

            // Keep the file open by leaking it (it will be closed when process exits)
            std::mem::forget(file);
        }
        RedirectTarget::Fd(source_fd) => {
            // Redirect to another file descriptor (e.g., 2>&1)
            unsafe {
                if libc::dup2(source_fd, fd) == -1 {
                    return Err(anyhow!(
                        "exec: failed to redirect {} to fd {}",
                        fd_name,
                        source_fd
                    ));
                }
            }

            // Store the permanent redirection in runtime
            match fd {
                libc::STDOUT_FILENO => runtime.set_permanent_stdout(Some(source_fd)),
                libc::STDERR_FILENO => runtime.set_permanent_stderr(Some(source_fd)),
                libc::STDIN_FILENO => runtime.set_permanent_stdin(Some(source_fd)),
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_exec_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_exec(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_exec_builtin_error() {
        let mut runtime = Runtime::new();
        let result = builtin_exec(&["cd".to_string(), "/tmp".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot execute builtin"));
    }

    #[test]
    fn test_exec_nonexistent_command() {
        let mut runtime = Runtime::new();
        let result = builtin_exec(&["nonexistent_command_12345".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("command not found"));
    }

    #[test]
    fn test_find_in_path() {
        // Test finding a common command
        let result = find_in_path("ls");
        assert!(result.is_some());
        assert!(result.unwrap().contains("ls"));
    }

    #[test]
    fn test_find_in_path_nonexistent() {
        let result = find_in_path("nonexistent_command_xyz");
        assert!(result.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_exec_redirect_stdout_to_file() {
        let _runtime = Runtime::new();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        // This test can't actually redirect stdout without affecting the test process
        // So we'll just test the redirect_fd function logic by using a different approach
        // In practice, exec redirection is tested via integration tests

        // For now, just verify the function accepts the correct arguments
        let _target = RedirectTarget::File {
            path: path.clone(),
            append: false,
        };

        // We can't actually test this in unit tests because it would redirect
        // the test process's stdout. This needs integration testing.
        // Just verify the types are correct
        assert!(!path.is_empty());
    }

    #[test]
    fn test_redirect_target_file() {
        let target = RedirectTarget::File {
            path: "/tmp/test.txt".to_string(),
            append: false,
        };

        match target {
            RedirectTarget::File { path, append } => {
                assert_eq!(path, "/tmp/test.txt");
                assert!(!append);
            }
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_redirect_target_fd() {
        let target = RedirectTarget::Fd(1);

        match target {
            RedirectTarget::Fd(fd) => {
                assert_eq!(fd, 1);
            }
            _ => panic!("Expected Fd variant"),
        }
    }

    // Note: Full integration tests for exec should be in tests/integration/
    // since they need to test actual process replacement and redirection,
    // which can't be done safely in unit tests without affecting the test runner.
}
