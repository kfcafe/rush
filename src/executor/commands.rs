//! Command execution for the Rush shell executor.
//!
//! This module handles executing commands including:
//! - Simple command dispatch (builtins, functions, externals)
//! - External command spawning with redirections
//! - User-defined function execution
//! - Subshell and brace group execution
//! - Background job execution
//! - Redirect and stdin handling

use super::*;

/// Maximum bytes captured from a child process's stdout or stderr.
/// Defaults to 50 MB; override with the `RUSH_MAX_CMD_OUTPUT` environment variable.
const MAX_CAPTURE_BYTES_DEFAULT: usize = 50 * 1024 * 1024;

/// Read child process output with a per-stream size cap to prevent OOM.
///
/// Reads stdout and stderr sequentially. Note: sequential reading can deadlock
/// if the child fills the stderr pipe buffer while we drain stdout. This is
/// acceptable for the common single-stream case; a future redesign should use
/// concurrent readers (threads or poll) to handle both streams safely.
///
/// Returns `(stdout, stderr, exit_code)`.
fn wait_with_capped_output(
    mut child: std::process::Child,
    stderr_to_stdout: bool,
) -> Result<(String, String, i32)> {
    use std::io::Read;

    let max_capture_bytes: usize = std::env::var("RUSH_MAX_CMD_OUTPUT")
        .ok()
        .and_then(|s| crate::run_api::parse_max_output(&s))
        .unwrap_or(MAX_CAPTURE_BYTES_DEFAULT);

    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let mut buf = [0u8; 8192];

    if let Some(ref mut pipe) = stdout_pipe {
        loop {
            match pipe.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if stdout_bytes.len() + n > max_capture_bytes {
                        let remaining = max_capture_bytes.saturating_sub(stdout_bytes.len());
                        stdout_bytes.extend_from_slice(&buf[..remaining]);
                        stdout_truncated = true;
                        break;
                    }
                    stdout_bytes.extend_from_slice(&buf[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }

    if let Some(ref mut pipe) = stderr_pipe {
        loop {
            match pipe.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if stderr_bytes.len() + n > max_capture_bytes {
                        let remaining = max_capture_bytes.saturating_sub(stderr_bytes.len());
                        stderr_bytes.extend_from_slice(&buf[..remaining]);
                        stderr_truncated = true;
                        break;
                    }
                    stderr_bytes.extend_from_slice(&buf[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }

    // Drop pipes before wait to avoid any residual blocking on the write side.
    drop(stdout_pipe);
    drop(stderr_pipe);

    let status = child
        .wait()
        .map_err(|e| anyhow!("Failed to wait for child: {}", e))?;

    let mut stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
    let mut stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();

    if stdout_truncated {
        eprintln!("rush: warning: command stdout truncated at {} bytes", max_capture_bytes);
    }
    if stderr_truncated {
        eprintln!("rush: warning: command stderr truncated at {} bytes", max_capture_bytes);
    }

    if stderr_to_stdout && !stderr_str.is_empty() {
        stdout_str.push_str(&stderr_str);
        stderr_str.clear();
    }

    Ok((stdout_str, stderr_str, status.code().unwrap_or(1)))
}

impl Executor {
    pub(crate) fn execute_command(&mut self, command: Command) -> Result<ExecutionResult> {
        // Clean up FIFOs from any previous process substitutions
        self.cleanup_process_subs();

        // Print command if xtrace is enabled
        if self.runtime.options.xtrace {
            let args_str = command.args.iter()
                .map(|arg| match arg {
                    Argument::Literal(s) | Argument::Variable(s) | Argument::BracedVariable(s) |
                    Argument::CommandSubstitution(s) | Argument::Flag(s) | Argument::Path(s) |
                    Argument::Glob(s) | Argument::ProcessSubIn(s) | Argument::ProcessSubOut(s) => s.clone(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            if args_str.is_empty() {
                eprintln!("+ {}", command.name);
            } else {
                eprintln!("+ {} {}", command.name, args_str);
            }
        }

        // Handle prefix environment assignments (e.g., FOO=bar cmd args)
        // Save old values to restore after command execution
        let saved_env: Vec<(String, Option<String>)> = command.prefix_env.iter()
            .map(|(k, _)| (k.clone(), self.runtime.get_variable(k)))
            .collect();

        // Set prefix env vars before command execution
        for (key, value) in &command.prefix_env {
            let expanded_value = self.expand_string_value(value)?;
            self.runtime.set_variable(key.clone(), expanded_value.clone());
            self.runtime.set_env(key, &expanded_value);
        }

        // Check if it's an alias and expand it
        let (command_name, command_args) = if let Some(alias_value) = self.runtime.get_alias(&command.name) {
            // Split the alias value into command and args
            let parts: Vec<&str> = alias_value.split_whitespace().collect();
            if parts.is_empty() {
                return Err(anyhow!("Empty alias expansion for '{}'", command.name));
            }

            // First part is the new command name
            let new_name = parts[0].to_string();

            // Remaining parts become additional arguments (prepended to original args)
            let mut new_args = Vec::new();
            for part in parts.iter().skip(1) {
                new_args.push(Argument::Literal(part.to_string()));
            }
            new_args.extend(command.args.clone());

            (new_name, new_args)
        } else {
            (command.name.clone(), command.args.clone())
        };

        // Check if it's a user-defined function first
        if self.runtime.get_function(&command_name).is_some() {
            let args = self.expand_and_resolve_arguments(&command_args)?;
            // Track last argument for $_
            if let Some(last) = args.last() {
                self.runtime.set_last_arg(last.clone());
            }
            let result = self.execute_user_function(&command_name, args);
            self.restore_prefix_env(&saved_env);
            return result;
        }

        // Autoload: if the command matches a .rush file in the function_path directory,
        // source that file (which defines the function) then call it.
        if let Some(autoload_file) = self.runtime.get_autoload_file(&command_name) {
            // Source the file — this should define the function in the runtime.
            if let Err(e) = self.source_file(&autoload_file) {
                eprintln!("autoload: failed to load '{}': {}", autoload_file.display(), e);
            } else if self.runtime.get_function(&command_name).is_some() {
                // Function is now defined; call it with the supplied arguments.
                let args = self.expand_and_resolve_arguments(&command_args)?;
                if let Some(last) = args.last() {
                    self.runtime.set_last_arg(last.clone());
                }
                let result = self.execute_user_function(&command_name, args);
                self.restore_prefix_env(&saved_env);
                return result;
            }
        }

        // Check if it's a builtin command
        if self.builtins.is_builtin(&command_name) {
            let args = self.expand_and_resolve_arguments(&command_args)?;
            // Track last argument for $_
            if let Some(last) = args.last() {
                self.runtime.set_last_arg(last.clone());
            }

            // Check for stdin redirects (heredoc or file) that provide stdin to builtins
            let stdin_content = self.extract_stdin_content(&command.redirects)?;
            // Also check for piped stdin from compound command in pipeline
            let piped_stdin = self.runtime.get_piped_stdin().map(|s| s.to_vec());
            
            // Helper to convert builtin errors to stderr in result (for redirect handling)
            // Flow-control signals (break, continue, return, exit) are re-propagated as errors.
            let builtin_result_to_stderr = |res: Result<ExecutionResult>, cmd_name: &str| -> Result<ExecutionResult> {
                match res {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        // Propagate flow-control signals so loops/functions/exit can catch them
                        if e.downcast_ref::<crate::builtins::break_builtin::BreakSignal>().is_some()
                            || e.downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>().is_some()
                            || e.downcast_ref::<crate::builtins::return_builtin::ReturnSignal>().is_some()
                            || e.downcast_ref::<crate::builtins::exit_builtin::ExitSignal>().is_some()
                        {
                            Err(e)
                        } else {
                            Ok(ExecutionResult::error(format!("{}: {}\n", cmd_name, e)))
                        }
                    }
                }
            };
            
            let mut result = if let Some(ref stdin_data) = stdin_content {
                builtin_result_to_stderr(
                    self.builtins.execute_with_stdin(
                        &command_name,
                        args,
                        &mut self.runtime,
                        Some(stdin_data.as_bytes()),
                    ),
                    &command_name,
                )?
            } else if let Some(ref piped_data) = piped_stdin {
                // Use piped stdin from compound command in pipeline
                // For 'read' builtin, consume one line and keep the rest
                if command_name == "read" {
                    // Find the first newline to determine how much to consume
                    let line_end = piped_data.iter().position(|&b| b == b'\n')
                        .map(|p| p + 1)
                        .unwrap_or(piped_data.len());
                    
                    // Execute read with just this portion
                    let result = builtin_result_to_stderr(
                        self.builtins.execute_with_stdin(
                            &command_name,
                            args,
                            &mut self.runtime,
                            Some(&piped_data[..line_end]),
                        ),
                        &command_name,
                    )?;
                    
                    // Update piped_stdin to remaining data
                    if line_end < piped_data.len() {
                        self.runtime.set_piped_stdin(piped_data[line_end..].to_vec());
                    } else {
                        // All data consumed - clear it (will cause EOF on next read)
                        let _ = self.runtime.take_piped_stdin();
                    }
                    
                    result
                } else {
                    builtin_result_to_stderr(
                        self.builtins.execute_with_stdin(
                            &command_name,
                            args,
                            &mut self.runtime,
                            Some(piped_data),
                        ),
                        &command_name,
                    )?
                }
            } else {
                builtin_result_to_stderr(
                    self.builtins.execute(&command_name, args, &mut self.runtime),
                    &command_name,
                )?
            };

            // Handle redirects for builtins
            if !command.redirects.is_empty() {
                result = self.apply_redirects(result, &command.redirects)?;
            }

            self.runtime.set_last_exit_code(result.exit_code);
            self.restore_prefix_env(&saved_env);
            return Ok(result);
        }

        // Execute external command with the potentially expanded command name and args
        let mut expanded_command = command;
        expanded_command.name = command_name;
        expanded_command.args = command_args;
        let result = self.execute_external_command(expanded_command)?;
        self.runtime.set_last_exit_code(result.exit_code);
        self.restore_prefix_env(&saved_env);
        Ok(result)
    }

    /// Restore prefix environment variables to their previous values after command execution.
    fn restore_prefix_env(&mut self, saved: &[(String, Option<String>)]) {
        for (key, old_value) in saved {
            match old_value {
                Some(val) => {
                    self.runtime.set_variable(key.clone(), val.clone());
                    self.runtime.set_env(key, val);
                }
                None => {
                    self.runtime.remove_variable(key);
                    std::env::remove_var(key);
                }
            }
        }
    }

    pub(crate) fn apply_redirects(&self, mut result: ExecutionResult, redirects: &[Redirect]) -> Result<ExecutionResult> {
        use std::fs::{File, OpenOptions};
        use std::io::Write;
        use std::path::Path;
        
        // Helper to resolve paths relative to cwd
        let resolve_path = |target: &str| -> std::path::PathBuf {
            let path = Path::new(target);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.runtime.get_cwd().join(target)
            }
        };
        
        for redirect in redirects {
            match &redirect.kind {
                RedirectKind::Stdout => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let mut file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        file.write_all(result.stdout().as_bytes())
                            .map_err(|e| anyhow!("Failed to write to '{}': {}", target, e))?;
                        result.clear_stdout(); // Clear stdout as it's been redirected
                    }
                }
                RedirectKind::StdoutAppend => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let mut file = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&resolved)
                            .map_err(|e| anyhow!("Failed to open '{}': {}", target, e))?;
                        file.write_all(result.stdout().as_bytes())
                            .map_err(|e| anyhow!("Failed to write to '{}': {}", target, e))?;
                        result.clear_stdout(); // Clear stdout as it's been redirected
                    }
                }
                RedirectKind::Stdin => {
                    // Stdin redirect doesn't make sense for builtins that have already executed
                    // This would need to be handled before execution
                }
                RedirectKind::Stderr => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let mut file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        file.write_all(result.stderr.as_bytes())
                            .map_err(|e| anyhow!("Failed to write to '{}': {}", target, e))?;
                        result.stderr.clear(); // Clear stderr as it's been redirected
                    }
                }
                RedirectKind::StderrToStdout => {
                    // Merge stderr into stdout
                    result.push_stdout(&result.stderr.clone());
                    result.stderr.clear();
                }
                RedirectKind::Both => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let mut file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        // Clone file descriptor for both stdout and stderr
                        file.write_all(result.stdout().as_bytes())
                            .map_err(|e| anyhow!("Failed to write to '{}': {}", target, e))?;
                        file.write_all(result.stderr.as_bytes())
                            .map_err(|e| anyhow!("Failed to write to '{}': {}", target, e))?;
                        result.clear_stdout();
                        result.stderr.clear();
                    }
                }
                RedirectKind::HereDoc | RedirectKind::HereDocLiteral => {
                    // Here-documents provide stdin content - for builtins that already
                    // executed, this is a no-op (stdin would need to be provided before execution)
                }
                RedirectKind::HereString => {
                    // Here-strings provide stdin content - for builtins that already
                    // executed, this is a no-op (stdin was provided before execution)
                }
                RedirectKind::FdDup { src, dst } => {
                    if *src == 1 && *dst == 2 {
                        result.stderr.push_str(&result.stdout());
                        result.clear_stdout();
                    } else if *src == 2 && *dst == 1 {
                        result.push_stdout(&result.stderr.clone());
                        result.stderr.clear();
                    }
                }
            }
        }
        
        Ok(result)
    }

    pub(crate) fn execute_user_function(&mut self, name: &str, args: Vec<String>) -> Result<ExecutionResult> {
        // Get the function definition (we know it exists because we checked earlier)
        let func = self.runtime.get_function(name)
            .ok_or_else(|| anyhow!("Function '{}' not found", name))?
            .clone(); // Clone to avoid borrow issues

        // Check recursion depth
        self.runtime.push_call(name.to_string())
            .map_err(|e| anyhow!(e))?;

        // Track function entry in call stack for error reporting
        self.call_stack.push(name.to_string());

        // Create a new scope for the function
        self.runtime.push_scope();

        // Bind arguments to parameters
        for (i, param) in func.params.iter().enumerate() {
            let arg_value = args.get(i).cloned().unwrap_or_default();
            self.runtime.set_variable(param.name.clone(), arg_value);
        }

        // Push positional parameters scope ($1, $2, $#, $@, $*) for the function
        // This preserves the caller's positional params on a stack
        self.runtime.push_positional_scope(args.clone());

        // Enter function context (allows return builtin)
        self.runtime.enter_function_context();

        // Execute function body
        let mut last_result = ExecutionResult::default();
        for statement in func.body {
            match self.execute_statement(statement) {
                Ok(stmt_result) => {
                    // Accumulate stdout from all statements
                    last_result.push_stdout(&stmt_result.stdout());
                    last_result.stderr.push_str(&stmt_result.stderr);
                    // Keep the last exit code
                    last_result.exit_code = stmt_result.exit_code;
                }
                Err(e) => {
                    // Check if this is a return signal
                    if let Some(return_signal) = e.downcast_ref::<crate::builtins::return_builtin::ReturnSignal>() {
                        // Early return from function
                        last_result.exit_code = return_signal.exit_code;
                        break;
                    } else {
                        // Some other error - propagate it
                        self.runtime.exit_function_context();
                        self.runtime.pop_positional_scope();
                        self.runtime.pop_scope();
                        self.runtime.pop_call();
                        self.call_stack.pop();
                        return Err(e);
                    }
                }
            }
        }

        // Exit function context
        self.runtime.exit_function_context();

        // Restore caller's positional parameters
        self.runtime.pop_positional_scope();

        // Clean up scope and call stack
        self.runtime.pop_scope();
        self.runtime.pop_call();
        self.call_stack.pop();

        Ok(last_result)
    }

    pub(crate) fn execute_external_command(&mut self, command: Command) -> Result<ExecutionResult> {
        let args = self.expand_and_resolve_arguments(&command.args)?;

        // Track last argument for $_
        if let Some(last) = args.last() {
            self.runtime.set_last_arg(last.clone());
        }

        // Set up command with redirects
        let mut cmd = StdCommand::new(&command.name);
        cmd.args(&args)
            .current_dir(self.runtime.get_cwd())
            .envs(self.runtime.get_env());

        // Handle redirections
        use std::fs::{File, OpenOptions};
        use std::process::Stdio;
        use std::path::Path;
        
        let mut stdout_redirect = false;
        let mut stderr_redirect = false;
        let mut stderr_to_stdout = false;
        let mut stdin_redirect = false;
        
        // Helper to resolve paths relative to cwd
        let resolve_path = |target: &str| -> std::path::PathBuf {
            let path = Path::new(target);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.runtime.get_cwd().join(target)
            }
        };
        
        for redirect in &command.redirects {
            match &redirect.kind {
                RedirectKind::Stdout => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        cmd.stdout(Stdio::from(file));
                        stdout_redirect = true;
                    }
                }
                RedirectKind::StdoutAppend => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let file = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&resolved)
                            .map_err(|e| anyhow!("Failed to open '{}': {}", target, e))?;
                        cmd.stdout(Stdio::from(file));
                        stdout_redirect = true;
                    }
                }
                RedirectKind::Stdin => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let file = File::open(&resolved)
                            .map_err(|e| anyhow!("Failed to open '{}': {}", target, e))?;
                        cmd.stdin(Stdio::from(file));
                        stdin_redirect = true;
                    }
                }
                RedirectKind::Stderr => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        cmd.stderr(Stdio::from(file));
                        stderr_redirect = true;
                    }
                }
                RedirectKind::StderrToStdout => {
                    // Redirect stderr to stdout
                    stderr_to_stdout = true;
                }
                RedirectKind::Both => {
                    if let Some(raw_target) = &redirect.target {
                        let target = expand_redirect_target(raw_target, &self.runtime);
                        let resolved = resolve_path(&target);
                        let file = File::create(&resolved)
                            .map_err(|e| anyhow!("Failed to create '{}': {}", target, e))?;
                        // Clone file descriptor for both stdout and stderr
                        cmd.stdout(Stdio::from(file.try_clone()
                            .map_err(|e| anyhow!("Failed to clone file descriptor: {}", e))?));
                        cmd.stderr(Stdio::from(file));
                        stdout_redirect = true;
                        stderr_redirect = true;
                    }
                }
                RedirectKind::HereDoc | RedirectKind::HereDocLiteral | RedirectKind::HereString => {
                    // Here-documents and here-strings provide stdin content
                    cmd.stdin(Stdio::piped());
                    stdin_redirect = true;
                }
                RedirectKind::FdDup { src, dst } => {
                    // Fd duplication handled via dup2 in pre_exec below
                    if *src == 1 {
                        stdout_redirect = true;
                    }
                    if *src == 2 {
                        stderr_redirect = true;
                    }
                    if *src == 2 && *dst == 1 {
                        stderr_to_stdout = true;
                    }
                }
            }
        }
        
        // Collect heredoc/here-string body before spawning (needs mutable borrow of self for expansion)
        let heredoc_body: Option<String> = {
            let mut body = None;
            for redirect in &command.redirects {
                match &redirect.kind {
                    RedirectKind::HereDoc => {
                        if let Some(b) = &redirect.target {
                            body = Some(self.expand_heredoc_body(b)?);
                        }
                    }
                    RedirectKind::HereDocLiteral => {
                        if let Some(b) = &redirect.target {
                            body = Some(b.clone());
                        }
                    }
                    RedirectKind::HereString => {
                        if let Some(b) = &redirect.target {
                            let expanded = self.expand_variables_in_literal(b)?;
                            body = Some(format!("{}\n", expanded));
                        }
                    }
                    _ => {}
                }
            }
            body
        };

        // Set default stdin to inherit from parent if not redirected
        if !stdin_redirect {
            cmd.stdin(Stdio::inherit());
        }
        
        // For commands with no redirects, check if we should run in full interactive mode
        // This allows interactive programs (like editors, REPLs, claude) to work properly
        // NEVER inherit IO in embedded mode (TUI usage) - always pipe
        let should_inherit_io = self.show_progress && 
                                !stdout_redirect && !stderr_redirect && 
                                command.redirects.is_empty() &&
                                std::io::stdout().is_terminal();
        
        // Set default piped outputs if not redirected
        if !stdout_redirect {
            if should_inherit_io {
                cmd.stdout(Stdio::inherit());
            } else {
                cmd.stdout(Stdio::piped());
            }
        }
        if !stderr_redirect && !stderr_to_stdout {
            if should_inherit_io {
                cmd.stderr(Stdio::inherit());
            } else {
                cmd.stderr(Stdio::piped());
            }
        } else if stderr_to_stdout && !stderr_redirect {
            // Redirect stderr to stdout for the process
            cmd.stderr(Stdio::piped());
        }

        // Collect fd duplication pairs for pre_exec
        let fd_dups: Vec<(u32, u32)> = command.redirects.iter()
            .filter_map(|r| match &r.kind {
                RedirectKind::FdDup { src, dst } => Some((*src, *dst)),
                _ => None,
            })
            .collect();

        // Use pre_exec to set the process group and fd dups before the child executes
        // This is required for proper job control and signal handling
        // SAFETY: pre_exec closure only calls async-signal-safe setpgid and dup2 after fork
        unsafe { // SAFETY
            cmd.pre_exec(move || {
                // Put this process in its own process group (PGID = PID)
                let pid = getpid();
                setpgid(pid, pid).map_err(|e| {
                    std::io::Error::other(format!("setpgid failed: {}", e))
                })?;

                // Apply fd duplications (e.g. >&2 → dup2(2, 1))
                for (src, dst) in &fd_dups {
                    if libc::dup2(*dst as i32, *src as i32) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }

                Ok(())
            });
        }

        // Collect data for suggestions before spawning (needed for error handling closure)
        let builtin_names: Vec<String> = self.builtins.builtin_names();
        let alias_names: Vec<String> = self.runtime
            .get_all_aliases()
            .keys()
            .cloned()
            .collect();
        let history_commands: Vec<String> = self.runtime
            .history()
            .entries()
            .iter()
            .rev()
            .take(100) // Use last 100 commands for suggestions
            .map(|e| e.command.clone())
            .collect();
        let current_dir = self.runtime.get_cwd().to_path_buf();
        let command_name = command.name.clone();

        // Spawn the command
        let mut child = cmd.spawn()
            .map_err(|e| {
                // If command not found, provide suggestions
                if e.kind() == std::io::ErrorKind::NotFound {
                    // Use suggestion engine for context-aware suggestions
                    let suggestions = self.suggestion_engine.suggest_command(
                        &command_name,
                        &builtin_names,
                        &alias_names,
                        &history_commands,
                        &current_dir,
                    );

                    let mut error_msg = format!("Command not found: '{}'", command_name);
                                
                    if !suggestions.is_empty() {
                        error_msg.push_str("\n\nDid you mean:\n");
                        for suggestion in suggestions.iter().take(3) {
                            error_msg.push_str(&format!("  {}\n", suggestion.text));
                        }
                    }

                    anyhow!(error_msg)
                } else {
                    anyhow!("Failed to execute '{}': {}", command_name, e)
                }
            })?;

        // Give terminal control to the child process group for interactive commands
        // This is required for programs like sudo, ssh, vim that need terminal access
        if should_inherit_io {
            let child_pgid = nix::unistd::Pid::from_raw(child.id() as i32);
            let _ = self.terminal_control.give_terminal_to(child_pgid);
        }

        // Write heredoc body to child's stdin if present
        if let Some(body) = heredoc_body {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(body.as_bytes())
                    .map_err(|e| anyhow!("Failed to write here-document to stdin: {}", e))?;
                drop(stdin); // Close stdin so child sees EOF
            }
        }

        // Wait for command to complete
        let (stdout_str, stderr_str, exit_code) = if should_inherit_io {
            // Interactive mode - IO is inherited, child has terminal control
            // Don't show progress indicator as it would interfere with child's display

            loop {
                // Check for signals
                if let Some(handler) = &self.signal_handler {
                    if handler.should_shutdown() {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(anyhow!("Command interrupted by signal"));
                    }
                }

                // Try to get the status
                match child.try_wait() {
                    Ok(Some(status)) => {
                        break (String::new(), String::new(), status.code().unwrap_or(1));
                    }
                    Ok(None) => {
                        // Short sleep to avoid busy-waiting
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => {
                        return Err(anyhow!("Failed to check status for '{}': {}", command.name, e));
                    }
                }
            }
        } else {
            // Non-interactive mode — read with a cap to prevent OOM from
            // unbounded child output.
            let (stdout_str, stderr_str, exit_code) =
                wait_with_capped_output(child, stderr_to_stdout)
                    .map_err(|e| anyhow!("Failed to wait for '{}': {}", command.name, e))?;
            (stdout_str, stderr_str, exit_code)
        };

        // Reclaim terminal control after child exits
        if should_inherit_io {
            let _ = self.terminal_control.reclaim_terminal();
        }

        Ok(ExecutionResult {
            output: Output::Text(stdout_str),
            stderr: stderr_str,
            exit_code,
            error: None,
        })
    }

    /// Extract stdin content from redirects, if any.
    /// For HereDoc (unquoted delimiter), performs variable expansion.
    /// For HereDocLiteral (quoted delimiter), returns body as-is.
    /// For Stdin (<), reads file content.
    pub(crate) fn extract_stdin_content(&mut self, redirects: &[Redirect]) -> Result<Option<String>> {
        for redirect in redirects {
            match &redirect.kind {
                RedirectKind::HereDoc => {
                    if let Some(body) = &redirect.target {
                        return Ok(Some(self.expand_heredoc_body(body)?));
                    }
                }
                RedirectKind::HereDocLiteral => {
                    if let Some(body) = &redirect.target {
                        return Ok(Some(body.clone()));
                    }
                }
                RedirectKind::Stdin => {
                    if let Some(target) = &redirect.target {
                        let path = std::path::Path::new(target);
                        let resolved = if path.is_absolute() {
                            path.to_path_buf()
                        } else {
                            self.runtime.get_cwd().join(target)
                        };
                        let content = std::fs::read_to_string(&resolved)
                            .map_err(|e| anyhow!("Failed to read '{}': {}", target, e))?;
                        return Ok(Some(content));
                    }
                }
                RedirectKind::HereString => {
                    if let Some(value) = &redirect.target {
                        let expanded = self.expand_variables_in_literal(value)?;
                        return Ok(Some(format!("{}\n", expanded)));
                    }
                }
                _ => {}
            }
        }
        Ok(None)
    }

    pub(crate) fn execute_subshell(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        // Clone the runtime to create an isolated environment
        let mut child_runtime = self.runtime.clone();

        // Increment SHLVL in the subshell
        let current_shlvl = child_runtime
            .get_variable("SHLVL")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(1);
        child_runtime.set_variable("SHLVL".to_string(), (current_shlvl + 1).to_string());

        // Create a new executor with the cloned runtime
        let mut child_executor = Executor {
            runtime: child_runtime,
            builtins: self.builtins.clone(),
            corrector: self.corrector.clone(),
            suggestion_engine: self.suggestion_engine.clone(),
            signal_handler: None, // Subshells don't need their own signal handlers
            show_progress: self.show_progress, // Inherit progress setting from parent
            terminal_control: self.terminal_control.clone(),
            call_stack: CallStack::new(),
            profile_data: None,
            enable_profiling: false,
            process_sub_fifos: Vec::new(),
            hook_manager: Default::default(),
        };

        // Execute all statements in the subshell, catching ExitSignal
        let result = match child_executor.execute(statements) {
            Ok(r) => r,
            Err(e) => {
                if let Some(exit_sig) = e.downcast_ref::<crate::builtins::exit_builtin::ExitSignal>() {
                    ExecutionResult {
                        output: Output::Text(String::new()),
                        stderr: String::new(),
                        exit_code: exit_sig.exit_code,
                        error: None,
                    }
                } else {
                    return Err(e);
                }
            }
        };

        // The subshell's runtime changes (variables, cwd) are discarded
        // Only the execution result (stdout, stderr, exit code) is returned
        Ok(result)
    }

    /// Execute a brace group { commands; }
    /// Unlike subshells, brace groups execute in the current shell context.
    /// Variable changes, directory changes, etc. persist after execution.
    pub(crate) fn execute_brace_group(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        // Execute statements in current context (not isolated like subshell)
        self.execute(statements)
    }

    /// Check if a statement is an exec command (which replaces the process).
    /// This is used to flush accumulated output before exec, since exec
    /// replaces the process and any buffered output would be lost.
    pub(crate) fn is_exec_command(statement: &Statement) -> bool {
        match statement {
            Statement::Command(cmd) => cmd.name == "exec",
            // Handle case where exec might be in a conditional
            Statement::ConditionalAnd(cond) => Self::is_exec_command(&cond.right),
            Statement::ConditionalOr(cond) => Self::is_exec_command(&cond.right),
            _ => false,
        }
    }

    pub(crate) fn execute_background(&mut self, statement: Statement) -> Result<ExecutionResult> {
        use std::process::Stdio;

        // For background jobs, we need to spawn a separate process
        // First, let's get the command string for tracking
        let command_str = self.statement_to_string(&statement);

        // Only handle Command statements in background for now
        match statement {
            Statement::Command(command) => {
                // Check if it's a builtin - builtins can't run in background
                if self.builtins.is_builtin(&command.name) {
                    return Err(anyhow!("Builtin commands cannot be run in background"));
                }

                // Resolve arguments
                let args: Result<Vec<String>> = command
                    .args
                    .iter()
                    .map(|arg| self.resolve_argument(arg))
                    .collect();
                
                let args = args?;

                // Spawn the process
                let mut cmd = StdCommand::new(&command.name);
                cmd.args(&args)
                    .current_dir(self.runtime.get_cwd())
                    .envs(self.runtime.get_env())
                    .stdin(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());

                // SAFETY: pre_exec closure only calls async-signal-safe setpgid after fork
                unsafe { // SAFETY
                    cmd.pre_exec(|| {
                        // Put this process in its own process group (PGID = PID)
                        let pid = getpid();
                        setpgid(pid, pid).map_err(|e| {
                            std::io::Error::other(format!("setpgid failed: {}", e))
                        })?;
                        Ok(())
                    });
                }

                let child = cmd.spawn()
                    .map_err(|e| anyhow!("Failed to spawn background process '{}': {}", command.name, e))?;

                let pid = child.id();

                // Add to job manager
                let job_id = self.runtime.job_manager().add_job(pid, command_str);

                // Track last background PID for $!
                self.runtime.set_last_bg_pid(pid);

                // Return success with job information
                Ok(ExecutionResult::success(format!("[{}] {}\n", job_id, pid)))
            }
            Statement::Pipeline(_) | Statement::Subshell(_) => {
                self.execute_background_via_sh(&command_str)
            }
            _ => Err(anyhow!("Only simple commands and pipelines can be run in background")),
        }
    }

    /// Execute a complex statement in background by wrapping it in sh -c
    pub(crate) fn execute_background_via_sh(&mut self, command_str: &str) -> Result<ExecutionResult> {
        use nix::unistd::{getpid, setpgid};
        use std::os::unix::process::CommandExt;
        use std::process::{Command as StdCommand, Stdio};

        let mut cmd = StdCommand::new("sh");
        cmd.arg("-c")
            .arg(command_str)
            .current_dir(self.runtime.get_cwd())
            .envs(self.runtime.get_env())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // SAFETY: pre_exec closure only calls async-signal-safe setpgid after fork
        unsafe { // SAFETY
            cmd.pre_exec(|| {
                let pid = getpid();
                setpgid(pid, pid).map_err(|e| {
                    std::io::Error::other(format!("setpgid failed: {}", e))
                })?;
                Ok(())
            });
        }

        let child = cmd.spawn()
            .map_err(|e| anyhow!("Failed to spawn background process: {}", e))?;

        let pid = child.id();
        let job_id = self.runtime.job_manager().add_job(pid, command_str.to_string());
        self.runtime.set_last_bg_pid(pid);

        Ok(ExecutionResult::success(format!("[{}] {}\n", job_id, pid)))
    }

    /// Clean up FIFO files from previous process substitutions.
    pub(crate) fn cleanup_process_subs(&mut self) {
        for path in self.process_sub_fifos.drain(..) {
            let _ = std::fs::remove_file(&path);
        }
    }
}
