pub mod error_formatter;
mod flow_signals;
pub mod pipeline;
pub mod profile;
pub mod stack;
pub mod structured_ops;
pub mod suggestions;
pub mod value;

// Re-export Value type for convenience
pub use profile::{ProfileData, ProfileFormatter};
pub use stack::CallStack;
pub use suggestions::{SuggestionConfig, SuggestionEngine};

use crate::ai::client::{LlmClient, Message, Response};

/// Maximum bytes captured from a command substitution before truncation.
/// Configurable via RUSH_MAX_SUBST_OUTPUT env var. Default: 50MB.
const DEFAULT_MAX_SUBSTITUTION_OUTPUT: usize = 50 * 1024 * 1024;

fn max_substitution_output() -> usize {
    std::env::var("RUSH_MAX_SUBST_OUTPUT")
        .ok()
        .and_then(|s| crate::run_api::parse_max_output(&s))
        .unwrap_or(DEFAULT_MAX_SUBSTITUTION_OUTPUT)
}
use crate::arithmetic;
use crate::builtins::Builtins;
use crate::correction::Corrector;
use crate::daemon::pi_rpc::{PiEvent, PiRpcError, PiRpcManager};
use crate::glob_expansion;
use crate::parser::ast::*;
use crate::runtime::Runtime;
// Progress indicator removed from interactive mode to avoid interfering with TUI apps
use crate::signal::SignalHandler;
use crate::terminal::TerminalControl;
use anyhow::{anyhow, Result};
use nix::unistd::{getpid, setpgid};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct Executor {
    runtime: Runtime,
    builtins: Builtins,
    corrector: Corrector,
    suggestion_engine: SuggestionEngine,
    signal_handler: Option<SignalHandler>,
    terminal_control: TerminalControl,
    call_stack: CallStack,
    show_progress: bool,
    pub profile_data: Option<ProfileData>,
    pub enable_profiling: bool,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new(),
            builtins: Builtins::new(),
            corrector: Corrector::new(),
            suggestion_engine: SuggestionEngine::new(),
            signal_handler: None,
            terminal_control: TerminalControl::new(),
            show_progress: true, // Default to true for CLI usage
            call_stack: CallStack::new(),
            profile_data: None,
            enable_profiling: false,
        }
    }

    /// Create executor without progress indicators (for embedded/TUI usage)
    pub fn new_embedded() -> Self {
        Self {
            runtime: Runtime::new(),
            builtins: Builtins::new(),
            corrector: Corrector::new(),
            suggestion_engine: SuggestionEngine::new(),
            signal_handler: None,
            terminal_control: TerminalControl::new(),
            show_progress: false,
            call_stack: CallStack::new(),
            profile_data: None,
            enable_profiling: false,
        }
    }

    pub fn new_with_signal_handler(signal_handler: SignalHandler) -> Self {
        Self {
            runtime: Runtime::new(),
            builtins: Builtins::new(),
            corrector: Corrector::new(),
            suggestion_engine: SuggestionEngine::new(),
            signal_handler: Some(signal_handler),
            terminal_control: TerminalControl::new(),
            show_progress: true,
            call_stack: CallStack::new(),
            profile_data: None,
            enable_profiling: false,
        }
    }

    /// Enable profiling for this executor
    pub fn with_profiling(mut self, enable: bool) -> Self {
        self.enable_profiling = enable;
        if enable {
            self.profile_data = Some(ProfileData::new());
        }
        self
    }

    /// Get mutable access to the suggestion engine
    pub fn suggestion_engine_mut(&mut self) -> &mut SuggestionEngine {
        &mut self.suggestion_engine
    }

    /// Get immutable access to the suggestion engine
    pub fn suggestion_engine(&self) -> &SuggestionEngine {
        &self.suggestion_engine
    }

    pub fn execute(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;
        // Preserve structured output from the last statement (e.g. a structured pipeline).
        // When the final statement produces structured data we return it instead of Text,
        // so callers can inspect the typed output without round-tripping through JSON text.
        let mut last_output: Output = Output::Text(String::new());

        for statement in statements {
            // Check for signals before each statement
            if let Some(handler) = &self.signal_handler {
                if handler.should_shutdown() {
                    // Execute signal trap if set
                    let signal_num = handler.signal_number();
                    let trap_signal = match signal_num {
                        2 => Some(crate::builtins::trap::TrapSignal::Int), // SIGINT
                        15 => Some(crate::builtins::trap::TrapSignal::Term), // SIGTERM
                        1 => Some(crate::builtins::trap::TrapSignal::Hup), // SIGHUP
                        _ => None,
                    };

                    if let Some(sig) = trap_signal {
                        let _ = self.execute_trap(sig);
                    }

                    return Err(anyhow!("Interrupted by signal"));
                }
            }

            // Before executing an exec command, flush accumulated output.
            // exec replaces the process, so any buffered output would be lost.
            if Self::is_exec_command(&statement) {
                use std::io::Write;
                if !accumulated_stdout.is_empty() {
                    print!("{}", accumulated_stdout);
                    let _ = std::io::stdout().flush();
                    accumulated_stdout.clear();
                }
                if !accumulated_stderr.is_empty() {
                    eprint!("{}", accumulated_stderr);
                    let _ = std::io::stderr().flush();
                    accumulated_stderr.clear();
                }
            }

            let result = self.execute_statement(statement)?;
            accumulated_stdout.push_str(&result.stdout());
            accumulated_stderr.push_str(&result.stderr);
            last_exit_code = result.exit_code;
            last_output = result.output;

            // Cap accumulated output to prevent unbounded memory growth.
            // This matters most in command substitution contexts where all
            // output is captured into a String.
            const MAX_ACCUMULATED: usize = 50 * 1024 * 1024; // 50MB
            if accumulated_stdout.len() > MAX_ACCUMULATED {
                accumulated_stdout.truncate(MAX_ACCUMULATED);
                // Ensure we're at a UTF-8 boundary
                while !accumulated_stdout.is_char_boundary(accumulated_stdout.len()) {
                    accumulated_stdout.pop();
                }
                // Only warn once
                if accumulated_stderr.is_empty() || !accumulated_stderr.contains("output truncated")
                {
                    accumulated_stderr
                        .push_str("rush: warning: accumulated output truncated at 50MB\n");
                }
            }

            // Update $? after each statement
            self.runtime.set_last_exit_code(last_exit_code);

            // Execute ERR trap if command failed
            if last_exit_code != 0 {
                let _ = self.execute_trap(crate::builtins::trap::TrapSignal::Err);
            }

            // Check errexit option: exit if command failed
            if self.runtime.options.errexit && last_exit_code != 0 {
                return Ok(ExecutionResult {
                    output: Output::Text(accumulated_stdout),
                    stderr: accumulated_stderr,
                    exit_code: last_exit_code,
                    error: None,
                });
            }
        }

        // When the final statement produced structured data, return it as-is so that
        // callers (e.g. tests, interactive rendering) can work with the typed output.
        // For text output, return the accumulated string as before.
        let final_output = match last_output {
            Output::Structured(_) => last_output,
            Output::Text(_) => Output::Text(accumulated_stdout),
        };

        Ok(ExecutionResult {
            output: final_output,
            stderr: accumulated_stderr,
            exit_code: last_exit_code,
            error: None,
        })
    }

    pub fn execute_statement(&mut self, statement: Statement) -> Result<ExecutionResult> {
        match statement {
            Statement::Command(cmd) => self.execute_command(cmd),
            Statement::Pipeline(pipeline) => self.execute_pipeline(pipeline),
            Statement::ParallelExecution(parallel) => self.execute_parallel(parallel),
            Statement::Assignment(assignment) => self.execute_assignment(assignment),
            Statement::FunctionDef(func) => self.execute_function_def(func),
            Statement::IfStatement(if_stmt) => self.execute_if_statement(if_stmt),
            Statement::ForLoop(for_loop) => self.execute_for_loop(for_loop),
            Statement::WhileLoop(while_loop) => self.execute_while_loop(while_loop),
            Statement::UntilLoop(until_loop) => self.execute_until_loop(until_loop),
            Statement::MatchExpression(match_expr) => self.execute_match(match_expr),
            Statement::CaseStatement(case_stmt) => self.execute_case(case_stmt),
            Statement::ConditionalAnd(cond_and) => self.execute_conditional_and(cond_and),
            Statement::ConditionalOr(cond_or) => self.execute_conditional_or(cond_or),
            Statement::Subshell(statements) => self.execute_subshell(statements),
            Statement::BackgroundCommand(cmd) => self.execute_background(*cmd),
            Statement::BraceGroup(statements) => self.execute_brace_group(statements),
            Statement::PipeAsk(pipe_ask) => self.execute_pipe_ask(pipe_ask),
        }
    }

    fn execute_pipe_ask(&mut self, pipe_ask: PipeAsk) -> Result<ExecutionResult> {
        // Execute the left-hand side and capture its output (structured or text)
        let cmd_result = self.execute_statement(*pipe_ask.command)?;

        let user_prompt = if pipe_ask.prompt.is_empty() {
            "Analyze this output".to_string()
        } else {
            pipe_ask.prompt.clone()
        };

        // Format the piped content, preserving structured data as JSON when available
        let piped_content = match &cmd_result.output {
            Output::Structured(value) => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| cmd_result.stdout())
            }
            Output::Text(_) => cmd_result.stdout(),
        };

        let full_prompt = if piped_content.is_empty() {
            user_prompt
        } else {
            format!("{}\n\nInput:\n```\n{}\n```", user_prompt, piped_content)
        };

        // Try the configured LLM provider first (Ollama, OpenAI, Anthropic)
        match LlmClient::from_config() {
            Ok(client) => {
                let messages = vec![Message::user(full_prompt)];
                match client.chat(&messages, None) {
                    Ok(Response::Text(text)) => {
                        print!("{}", text);
                        if !text.ends_with('\n') {
                            println!();
                        }
                        Ok(ExecutionResult {
                            output: Output::Text(text),
                            stderr: String::new(),
                            exit_code: 0,
                            error: None,
                        })
                    }
                    Ok(Response::ToolCall { name, .. }) => {
                        // |? is read-only analysis — tool calls are unexpected
                        let msg = format!("Unexpected tool call '{}' from AI in |? context", name);
                        eprintln!("{}", msg);
                        Ok(ExecutionResult {
                            output: Output::Text(String::new()),
                            stderr: msg.clone(),
                            exit_code: 1,
                            error: Some(msg),
                        })
                    }
                    Err(e) => {
                        let msg = format!("AI error: {}", e);
                        eprintln!("{}", msg);
                        Ok(ExecutionResult {
                            output: Output::Text(String::new()),
                            stderr: msg.clone(),
                            exit_code: 1,
                            error: Some(msg),
                        })
                    }
                }
            }
            Err(_) => {
                // No rush AI config — fall back to the pi subprocess if available
                self.execute_pipe_ask_via_pi(full_prompt)
            }
        }
    }

    /// Fallback: send a prompt through the pi --rpc subprocess when no local LLM is configured.
    fn execute_pipe_ask_via_pi(&mut self, full_prompt: String) -> Result<ExecutionResult> {
        let mut manager = PiRpcManager::new();

        if let Err(e) = manager.ensure_running() {
            let error_msg = match e {
                PiRpcError::SpawnFailed(_) => {
                    "AI not configured. Run `rush ai setup` or install pi: npm install -g @mariozechner/pi-coding-agent".to_string()
                }
                other => format!("Pi error: {}", other),
            };
            eprintln!("{}", error_msg);
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: error_msg,
                exit_code: 1,
                error: None,
            });
        }

        let mut response_text = String::new();
        let mut final_exit_code = 0;

        match manager.prompt(&full_prompt) {
            Ok(events) => {
                for event_result in events {
                    match event_result {
                        Ok(PiEvent::ContentDelta { content }) => {
                            print!("{}", content);
                            let _ = std::io::stdout().flush();
                            response_text.push_str(&content);
                        }
                        Ok(PiEvent::AgentEnd {}) => {
                            if !response_text.is_empty() && !response_text.ends_with('\n') {
                                println!();
                            }
                            break;
                        }
                        Ok(PiEvent::Error { message }) => {
                            eprintln!("Pi error: {}", message);
                            final_exit_code = 1;
                            break;
                        }
                        Ok(PiEvent::Ready {}) | Ok(PiEvent::Unknown) => {}
                        Err(e) => {
                            eprintln!("Error reading from pi: {}", e);
                            final_exit_code = 1;
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let msg = format!("Failed to send prompt to pi: {}", e);
                eprintln!("{}", msg);
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: msg,
                    exit_code: 1,
                    error: None,
                });
            }
        }

        Ok(ExecutionResult {
            output: Output::Text(response_text),
            stderr: String::new(),
            exit_code: final_exit_code,
            error: None,
        })
    }

    fn execute_command(&mut self, command: Command) -> Result<ExecutionResult> {
        // Print command if xtrace is enabled
        if self.runtime.options.xtrace {
            let args_str = command
                .args
                .iter()
                .map(|arg| match arg {
                    Argument::Literal(s)
                    | Argument::SingleQuoted(s)
                    | Argument::Variable(s)
                    | Argument::BracedVariable(s)
                    | Argument::CommandSubstitution(s)
                    | Argument::Flag(s)
                    | Argument::Path(s)
                    | Argument::Glob(s) => s.clone(),
                    Argument::DoubleQuoted(parts) => parts
                        .iter()
                        .map(|p| match p {
                            ArgumentPart::Literal(s) => s.clone(),
                            ArgumentPart::Variable(s)
                            | ArgumentPart::BracedVariable(s)
                            | ArgumentPart::CommandSubstitution(s) => s.clone(),
                        })
                        .collect::<Vec<_>>()
                        .join(""),
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
        let saved_env: Vec<(String, Option<String>)> = command
            .prefix_env
            .iter()
            .map(|(k, _)| (k.clone(), self.runtime.get_variable(k)))
            .collect();

        // Set prefix env vars before command execution
        for (key, value) in &command.prefix_env {
            let expanded_value = self.expand_string_value(value)?;
            self.runtime
                .set_variable(key.clone(), expanded_value.clone());
            self.runtime.set_env(key, &expanded_value);
        }

        // Check if it's an alias and expand it
        let (command_name, command_args) =
            if let Some(alias_value) = self.runtime.get_alias(&command.name) {
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

            // Helper to convert builtin errors to stderr in result (for redirect handling).
            // Flow-control signals must propagate so loops, functions, and exit handling can
            // catch them instead of treating them as ordinary command failures.
            let builtin_result_to_stderr =
                |res: Result<ExecutionResult>, cmd_name: &str| -> Result<ExecutionResult> {
                    match res {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            if crate::executor::flow_signals::is_flow_control_signal(&e) {
                                return Err(e);
                            }
                            Ok(ExecutionResult::error(format!("{}: {}\n", cmd_name, e)))
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
                    let line_end = piped_data
                        .iter()
                        .position(|&b| b == b'\n')
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
                        self.runtime
                            .set_piped_stdin(piped_data[line_end..].to_vec());
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
                    self.builtins
                        .execute(&command_name, args, &mut self.runtime),
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

    /// Expand a string value that may contain variable references ($VAR, ${VAR}, etc.)
    fn expand_string_value(&self, value: &str) -> Result<String> {
        if value.contains("$(") || value.contains('`') {
            // String contains command substitution(s) - expand them
            self.expand_command_substitutions_in_string(value)
        } else if value.starts_with('$') {
            // Variable reference - expand it
            if value.starts_with("${") && value.ends_with('}') {
                // Braced variable ${VAR}
                let var_name = value.trim_start_matches("${").trim_end_matches('}');
                Ok(self.runtime.get_variable(var_name).unwrap_or_default())
            } else {
                // Simple variable $VAR
                let var_name = value.trim_start_matches('$');
                Ok(self.runtime.get_variable(var_name).unwrap_or_default())
            }
        } else {
            Ok(value.to_string())
        }
    }

    fn expand_variables_in_literal(&mut self, input: &str) -> Result<String> {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' {
                if let Some(next_char) = chars.peek() {
                    match next_char {
                        '(' => {
                            // Command substitution $(...) or arithmetic expansion $((expr))
                            let mut cmd_str = String::from("$(");
                            chars.next(); // consume '('
                            let mut depth = 1;
                            while let Some(ch) = chars.peek() {
                                if *ch == '(' {
                                    depth += 1;
                                } else if *ch == ')' {
                                    depth -= 1;
                                    if depth == 0 {
                                        cmd_str.push(')');
                                        chars.next(); // consume ')'
                                        break;
                                    }
                                }
                                cmd_str.push(*ch);
                                chars.next();
                            }
                            let expanded = self.expand_command_substitutions_in_string(&cmd_str)?;
                            result.push_str(&expanded);
                        }
                        '{' => {
                            // Braced variable ${...}
                            chars.next(); // consume '{'
                            let mut braced_content = String::new();
                            let mut depth = 1;
                            while let Some(ch) = chars.next() {
                                if ch == '{' {
                                    depth += 1;
                                    braced_content.push(ch);
                                } else if ch == '}' {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                    braced_content.push(ch);
                                } else {
                                    braced_content.push(ch);
                                }
                            }
                            // Use parse_braced_var_expansion and expand_variable
                            let braced_var = format!("${{{}}}", braced_content);
                            let expansion = self.parse_braced_var_expansion(&braced_var)?;
                            let value = self.runtime.expand_variable(&expansion)?;
                            result.push_str(&value);
                        }
                        // Special variables
                        '#' => {
                            chars.next();
                            result.push_str(&self.runtime.param_count().to_string());
                        }
                        '@' => {
                            chars.next();
                            result.push_str(&self.runtime.get_positional_params().join(" "));
                        }
                        '*' => {
                            chars.next();
                            result.push_str(&self.runtime.get_positional_params().join(" "));
                        }
                        '?' => {
                            chars.next();
                            result.push_str(&self.runtime.get_last_exit_code().to_string());
                        }
                        '!' => {
                            chars.next();
                            if let Some(pid) = self.runtime.get_last_bg_pid() {
                                result.push_str(&pid.to_string());
                            }
                        }
                        '$' => {
                            chars.next();
                            result.push_str(&std::process::id().to_string());
                        }
                        '-' => {
                            chars.next();
                            result.push_str(&self.runtime.get_option_flags());
                        }
                        '_' => {
                            chars.next();
                            result.push_str(&self.runtime.get_last_arg());
                        }
                        // Alphanumeric variables
                        c if c.is_ascii_digit() || c.is_ascii_alphabetic() || *c == '_' => {
                            let mut var_name = String::new();
                            while let Some(ch) = chars.peek() {
                                if ch.is_ascii_alphanumeric() || *ch == '_' {
                                    var_name.push(*ch);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            // Check if it's a positional parameter
                            if let Ok(index) = var_name.parse::<usize>() {
                                if index > 0 {
                                    if let Some(value) = self.runtime.get_positional_param(index) {
                                        result.push_str(&value);
                                    }
                                } else if index == 0 {
                                    if let Some(val) = self.runtime.get_variable("0") {
                                        result.push_str(&val);
                                    } else {
                                        result.push_str("rush");
                                    }
                                }
                            } else if let Some(value) = self.runtime.get_variable(&var_name) {
                                result.push_str(&value);
                            }
                        }
                        _ => {
                            result.push(c);
                        }
                    }
                } else {
                    result.push(c);
                }
            } else if c == '`' {
                // Backtick command substitution
                let mut cmd_str = String::from("`");
                while let Some(ch) = chars.next() {
                    cmd_str.push(ch);
                    if ch == '`' {
                        break;
                    } else if ch == '\\' {
                        // Handle escaped characters inside backticks
                        if let Some(escaped) = chars.next() {
                            cmd_str.push(escaped);
                        }
                    }
                }
                let expanded = self.expand_command_substitutions_in_string(&cmd_str)?;
                result.push_str(&expanded);
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }

    fn apply_redirects(
        &self,
        mut result: ExecutionResult,
        redirects: &[Redirect],
    ) -> Result<ExecutionResult> {
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
            }
        }

        Ok(result)
    }

    fn execute_user_function(&mut self, name: &str, args: Vec<String>) -> Result<ExecutionResult> {
        // Get the function definition (we know it exists because we checked earlier)
        let func = self
            .runtime
            .get_function(name)
            .ok_or_else(|| anyhow!("Function '{}' not found", name))?
            .clone(); // Clone to avoid borrow issues

        // Check recursion depth
        self.runtime
            .push_call(name.to_string())
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
                    if let Some(return_signal) =
                        e.downcast_ref::<crate::builtins::return_builtin::ReturnSignal>()
                    {
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

    fn execute_external_command(&mut self, command: Command) -> Result<ExecutionResult> {
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
        use std::path::Path;
        use std::process::Stdio;

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
                        cmd.stdout(Stdio::from(
                            file.try_clone()
                                .map_err(|e| anyhow!("Failed to clone file descriptor: {}", e))?,
                        ));
                        cmd.stderr(Stdio::from(file));
                        stdout_redirect = true;
                        stderr_redirect = true;
                    }
                }
                RedirectKind::HereDoc | RedirectKind::HereDocLiteral => {
                    // Here-documents provide stdin content
                    cmd.stdin(Stdio::piped());
                    stdin_redirect = true;
                }
            }
        }

        // Collect heredoc body before spawning (needs mutable borrow of self for expansion)
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
        let should_inherit_io = self.show_progress
            && !stdout_redirect
            && !stderr_redirect
            && command.redirects.is_empty()
            && atty::is(atty::Stream::Stdout);

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

        // Use pre_exec to set the process group before the child executes
        // This is required for proper job control and signal handling
        unsafe {
            cmd.pre_exec(|| {
                // Put this process in its own process group (PGID = PID)
                let pid = getpid();
                setpgid(pid, pid).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("setpgid failed: {}", e))
                })?;
                Ok(())
            });
        }

        // Collect data for suggestions before spawning (needed for error handling closure)
        let builtin_names: Vec<String> = self.builtins.builtin_names();
        let alias_names: Vec<String> = self.runtime.get_all_aliases().keys().cloned().collect();
        let history_commands: Vec<String> = self
            .runtime
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
        let mut child = cmd.spawn().map_err(|e| {
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
                stdin
                    .write_all(body.as_bytes())
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
                        return Err(anyhow!(
                            "Failed to check status for '{}': {}",
                            command.name,
                            e
                        ));
                    }
                }
            }
        } else {
            // Non-interactive mode - use blocking wait (most efficient)
            let output = child
                .wait_with_output()
                .map_err(|e| anyhow!("Failed to wait for '{}': {}", command.name, e))?;

            let mut stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

            if stderr_to_stdout && !stderr_str.is_empty() {
                stdout_str.push_str(&stderr_str);
            }

            (
                stdout_str,
                if stderr_to_stdout {
                    String::new()
                } else {
                    stderr_str
                },
                output.status.code().unwrap_or(1),
            )
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
    fn extract_stdin_content(&mut self, redirects: &[Redirect]) -> Result<Option<String>> {
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
                _ => {}
            }
        }
        Ok(None)
    }

    /// Expand variables and command substitutions in a heredoc body.
    fn expand_heredoc_body(&mut self, body: &str) -> Result<String> {
        let mut result = String::with_capacity(body.len());
        let chars: Vec<char> = body.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                '$' if i + 1 < chars.len() => {
                    match chars[i + 1] {
                        '(' => {
                            // Command substitution $(...)
                            let start = i + 2;
                            if let Some(end) = self.find_matching_paren_in_str(&chars, start) {
                                let cmd_str: String = chars[start..end].iter().collect();
                                let sub_result = self.execute_command_substitution_str(&cmd_str)?;
                                result.push_str(sub_result.trim_end_matches('\n'));
                                i = end + 1;
                            } else {
                                result.push('$');
                                i += 1;
                            }
                        }
                        '{' => {
                            // Braced variable ${...}
                            if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                                let var_name: String = chars[i + 2..i + 2 + close].iter().collect();
                                let value = self.expand_braced_variable(&var_name);
                                result.push_str(&value);
                                i = i + 3 + close;
                            } else {
                                result.push('$');
                                i += 1;
                            }
                        }
                        c if c.is_ascii_alphabetic() || c == '_' => {
                            // Simple variable $VAR
                            let start = i + 1;
                            let mut end = start;
                            while end < chars.len()
                                && (chars[end].is_ascii_alphanumeric() || chars[end] == '_')
                            {
                                end += 1;
                            }
                            let var_name: String = chars[start..end].iter().collect();
                            let value = self.runtime.get_variable(&var_name).unwrap_or_default();
                            result.push_str(&value);
                            i = end;
                        }
                        '?' => {
                            let code = self.runtime.get_last_exit_code();
                            result.push_str(&code.to_string());
                            i += 2;
                        }
                        '$' => {
                            result.push_str(&std::process::id().to_string());
                            i += 2;
                        }
                        _ => {
                            result.push('$');
                            i += 1;
                        }
                    }
                }
                '`' => {
                    let start = i + 1;
                    if let Some(end) = chars[start..].iter().position(|&c| c == '`') {
                        let cmd_str: String = chars[start..start + end].iter().collect();
                        let sub_result = self.execute_command_substitution_str(&cmd_str)?;
                        result.push_str(sub_result.trim_end_matches('\n'));
                        i = start + end + 1;
                    } else {
                        result.push('`');
                        i += 1;
                    }
                }
                '\\' if i + 1 < chars.len() => match chars[i + 1] {
                    '$' => {
                        result.push('$');
                        i += 2;
                    }
                    '`' => {
                        result.push('`');
                        i += 2;
                    }
                    '\\' => {
                        result.push('\\');
                        i += 2;
                    }
                    'n' => {
                        result.push('\n');
                        i += 2;
                    }
                    't' => {
                        result.push('\t');
                        i += 2;
                    }
                    _ => {
                        result.push('\\');
                        result.push(chars[i + 1]);
                        i += 2;
                    }
                },
                c => {
                    result.push(c);
                    i += 1;
                }
            }
        }

        Ok(result)
    }

    fn find_matching_paren_in_str(&self, chars: &[char], start: usize) -> Option<usize> {
        let mut depth = 1;
        let mut pos = start;
        while pos < chars.len() && depth > 0 {
            match chars[pos] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(pos);
                    }
                }
                _ => {}
            }
            pos += 1;
        }
        None
    }

    fn expand_braced_variable(&self, expr: &str) -> String {
        // String length: ${#var}
        if expr.starts_with('#') {
            let var_name = &expr[1..];
            return self
                .runtime
                .get_variable(var_name)
                .map(|v| v.len().to_string())
                .unwrap_or_else(|| "0".to_string());
        }
        if expr.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return self.runtime.get_variable(expr).unwrap_or_default();
        }
        if let Some(pos) = expr.find(":-") {
            let var_name = &expr[..pos];
            let default_val = &expr[pos + 2..];
            return self
                .runtime
                .get_variable(var_name)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default_val.to_string());
        }
        if let Some(pos) = expr.find(":=") {
            let var_name = &expr[..pos];
            let default_val = &expr[pos + 2..];
            let val = self.runtime.get_variable(var_name);
            if val.as_deref().map_or(true, str::is_empty) {
                return default_val.to_string();
            }
            return val.unwrap_or_default();
        }
        // Use alternate if set and non-empty
        if let Some(pos) = expr.find(":+") {
            let var_name = &expr[..pos];
            let alternate = &expr[pos + 2..];
            return self
                .runtime
                .get_variable(var_name)
                .filter(|v| !v.is_empty())
                .map(|_| alternate.to_string())
                .unwrap_or_default();
        }
        // Error if unset or empty
        if let Some(pos) = expr.find(":?") {
            let var_name = &expr[..pos];
            let message = &expr[pos + 2..];
            if self
                .runtime
                .get_variable(var_name)
                .map_or(true, |v| v.is_empty())
            {
                eprintln!("{}: {}", var_name, message);
            }
            return self.runtime.get_variable(var_name).unwrap_or_default();
        }
        // Remove longest suffix: ${var%%pattern}
        if let Some(pos) = expr.find("%%") {
            let var_name = &expr[..pos];
            let pattern = &expr[pos + 2..];
            let value = self.runtime.get_variable(var_name).unwrap_or_default();
            return remove_longest_suffix(&value, pattern);
        }
        // Remove shortest suffix: ${var%pattern}
        if let Some(pos) = expr.find('%') {
            let var_name = &expr[..pos];
            let pattern = &expr[pos + 1..];
            let value = self.runtime.get_variable(var_name).unwrap_or_default();
            return remove_shortest_suffix(&value, pattern);
        }
        // Remove longest prefix: ${var##pattern}
        if let Some(pos) = expr.find("##") {
            let var_name = &expr[..pos];
            let pattern = &expr[pos + 2..];
            let value = self.runtime.get_variable(var_name).unwrap_or_default();
            return remove_longest_prefix(&value, pattern);
        }
        // Remove shortest prefix: ${var#pattern}
        if let Some(pos) = expr.find('#') {
            let var_name = &expr[..pos];
            let pattern = &expr[pos + 1..];
            let value = self.runtime.get_variable(var_name).unwrap_or_default();
            return remove_shortest_prefix(&value, pattern);
        }
        self.runtime.get_variable(expr).unwrap_or_default()
    }

    fn execute_command_substitution_str(&mut self, cmd_str: &str) -> Result<String> {
        use crate::lexer::Lexer;
        use crate::parser::Parser;

        let tokens = Lexer::tokenize(cmd_str)
            .map_err(|e| anyhow!("Heredoc command substitution lex error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse()?;
        let result = self.execute(stmts)?;
        let mut output = result.stdout();
        let max = max_substitution_output();
        if output.len() > max {
            output.truncate(max);
            while !output.is_char_boundary(output.len()) {
                output.pop();
            }
            eprintln!(
                "rush: warning: command substitution output truncated at {} bytes",
                max
            );
        }
        Ok(output)
    }

    fn execute_pipeline(&mut self, pipeline: Pipeline) -> Result<ExecutionResult> {
        pipeline::execute_pipeline(pipeline, &mut self.runtime, &self.builtins)
    }

    fn execute_parallel(&mut self, parallel: ParallelExecution) -> Result<ExecutionResult> {
        if parallel.commands.is_empty() {
            return Ok(ExecutionResult::default());
        }

        // Clone necessary data for thread safety
        let builtins = Arc::new(self.builtins.clone());
        let corrector = Arc::new(self.corrector.clone());
        let runtime_snapshot = Arc::new(self.runtime.clone());

        let mut handles = Vec::new();

        for command in parallel.commands {
            let builtins = Arc::clone(&builtins);
            let corrector = Arc::clone(&corrector);
            let runtime_snapshot = Arc::clone(&runtime_snapshot);

            let handle = thread::spawn(move || {
                let result = if builtins.is_builtin(&command.name) {
                    // Execute builtin
                    let args = expand_and_resolve_arguments_static(
                        &command.args,
                        &runtime_snapshot,
                        &builtins,
                        &corrector,
                    )?;

                    // We need a mutable runtime, but we can't safely share it across threads
                    // For now, create a temporary runtime for builtins in parallel execution
                    let mut temp_runtime = (*runtime_snapshot).clone();
                    builtins.execute(&command.name, args, &mut temp_runtime)
                } else {
                    // Execute external command
                    let args = expand_and_resolve_arguments_static(
                        &command.args,
                        &runtime_snapshot,
                        &builtins,
                        &corrector,
                    )?;

                    match StdCommand::new(&command.name)
                        .args(&args)
                        .current_dir(runtime_snapshot.get_cwd())
                        .envs(runtime_snapshot.get_env())
                        .output()
                    {
                        Ok(output) => Ok(ExecutionResult {
                            output: Output::Text(
                                String::from_utf8_lossy(&output.stdout).to_string(),
                            ),
                            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                            exit_code: output.status.code().unwrap_or(1),
                            error: None,
                        }),
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::NotFound {
                                let builtin_names: Vec<String> = builtins.builtin_names();

                                // Get aliases for suggestions
                                let alias_names: Vec<String> =
                                    runtime_snapshot.get_all_aliases().keys().cloned().collect();

                                // Use alias-aware suggestions
                                let suggestions = corrector.suggest_command_with_aliases(
                                    &command.name,
                                    &builtin_names,
                                    &alias_names,
                                );

                                let mut error_msg =
                                    format!("Command not found: '{}'", command.name);

                                if !suggestions.is_empty() {
                                    error_msg.push_str("\n\nDid you mean:\n");
                                    for suggestion in suggestions.iter().take(3) {
                                        error_msg.push_str(&format!("  {}\n", suggestion.text));
                                    }
                                }

                                Err(anyhow!(error_msg))
                            } else {
                                Err(anyhow!("Failed to execute '{}': {}", command.name, e))
                            }
                        }
                    }
                };

                result
            });

            handles.push(handle);
        }

        // Join all threads and collect results
        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        let mut max_exit_code = 0;

        for handle in handles {
            match handle.join() {
                Ok(Ok(result)) => {
                    combined_stdout.push_str(&result.stdout());
                    combined_stderr.push_str(&result.stderr);
                    max_exit_code = max_exit_code.max(result.exit_code);
                }
                Ok(Err(e)) => {
                    combined_stderr.push_str(&format!("{}\n", e));
                    max_exit_code = max_exit_code.max(1);
                }
                Err(_) => {
                    combined_stderr.push_str("Thread panicked during parallel execution\n");
                    max_exit_code = max_exit_code.max(1);
                }
            }
        }

        Ok(ExecutionResult {
            output: Output::Text(combined_stdout),
            stderr: combined_stderr,
            exit_code: max_exit_code,
            error: None,
        })
    }

    fn execute_assignment(&mut self, assignment: Assignment) -> Result<ExecutionResult> {
        let value = self.evaluate_expression(assignment.value)?;
        self.runtime.set_variable_checked(assignment.name, value)?;
        Ok(ExecutionResult::default())
    }

    fn execute_function_def(&mut self, func: FunctionDef) -> Result<ExecutionResult> {
        self.runtime.define_function(func);
        Ok(ExecutionResult::default())
    }

    fn execute_if_statement(&mut self, if_stmt: IfStatement) -> Result<ExecutionResult> {
        match if_stmt.condition {
            IfCondition::Commands(condition_stmts) => {
                // Shell-style: evaluate condition by running commands
                let condition_result = self.evaluate_condition_commands(&condition_stmts)?;

                if condition_result {
                    return self.execute_block(if_stmt.then_block);
                }

                // Try elif clauses
                for elif in if_stmt.elif_clauses {
                    let elif_result = self.evaluate_condition_commands(&elif.condition)?;
                    if elif_result {
                        return self.execute_block(elif.body);
                    }
                }

                // Else block
                if let Some(else_block) = if_stmt.else_block {
                    return self.execute_block(else_block);
                }

                Ok(ExecutionResult::default())
            }
            IfCondition::Expression(expr) => {
                // Rust-style: evaluate expression for truthiness
                let condition = self.evaluate_expression(expr)?;

                if self.is_truthy(&condition) {
                    self.execute_block(if_stmt.then_block)
                } else if let Some(else_block) = if_stmt.else_block {
                    self.execute_block(else_block)
                } else {
                    Ok(ExecutionResult::default())
                }
            }
        }
    }

    /// Evaluate a list of condition commands. Returns true if last command exits 0.
    fn evaluate_condition_commands(&mut self, commands: &[Statement]) -> Result<bool> {
        let mut last_exit_code = 0;
        for statement in commands {
            let result = self.execute_statement(statement.clone())?;
            last_exit_code = result.exit_code;
        }
        Ok(last_exit_code == 0)
    }

    /// Execute a block of statements and return the combined result.
    fn execute_block(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        for statement in statements {
            let result = self.execute_statement(statement)?;
            accumulated_stdout.push_str(&result.stdout());
            accumulated_stderr.push_str(&result.stderr);
            last_exit_code = result.exit_code;
        }

        Ok(ExecutionResult {
            output: Output::Text(accumulated_stdout),
            stderr: accumulated_stderr,
            exit_code: last_exit_code,
            error: None,
        })
    }

    fn execute_for_loop(&mut self, for_loop: ForLoop) -> Result<ExecutionResult> {
        // Build the list of items to iterate over
        let items: Vec<String> = if for_loop.words.is_empty() {
            // No word list: iterate over positional parameters ($@)
            self.runtime.get_positional_params().to_vec()
        } else {
            // Expand each word individually (handles variables, globs, etc.)
            self.expand_and_resolve_arguments(&for_loop.words)?
        };

        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            for item in items {
                self.runtime.set_variable(for_loop.variable.clone(), item);
                for statement in &for_loop.body {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            accumulated_stdout.push_str(&result.stdout());
                            accumulated_stderr.push_str(&result.stderr);
                            last_exit_code = result.exit_code;
                        }
                        Err(e) => {
                            // Check if this is a break signal
                            if let Some(break_signal) =
                                e.downcast_ref::<crate::builtins::break_builtin::BreakSignal>()
                            {
                                // First, add any accumulated output from the break signal itself
                                accumulated_stdout.push_str(&break_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&break_signal.accumulated_stderr);

                                if break_signal.levels == 1 {
                                    // Break from this loop, return accumulated output
                                    return Ok(ExecutionResult {
                                        output: Output::Text(accumulated_stdout),
                                        stderr: accumulated_stderr,
                                        exit_code: last_exit_code,
                                        error: None,
                                    });
                                } else {
                                    // Propagate to outer loop with decreased level and accumulated output
                                    return Err(anyhow::Error::new(
                                        crate::builtins::break_builtin::BreakSignal {
                                            levels: break_signal.levels - 1,
                                            accumulated_stdout: accumulated_stdout.clone(),
                                            accumulated_stderr: accumulated_stderr.clone(),
                                        },
                                    ));
                                }
                            }

                            // Check if this is a continue signal
                            if let Some(continue_signal) =
                                e.downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>(
                                )
                            {
                                // First, add any accumulated output from the continue signal itself
                                accumulated_stdout.push_str(&continue_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&continue_signal.accumulated_stderr);

                                if continue_signal.levels == 1 {
                                    // Continue in this loop - skip to next iteration
                                    break; // Break out of the statement loop, continue with next item
                                } else {
                                    // Propagate to outer loop with decreased level and accumulated output
                                    return Err(anyhow::Error::new(
                                        crate::builtins::continue_builtin::ContinueSignal {
                                            levels: continue_signal.levels - 1,
                                            accumulated_stdout: accumulated_stdout.clone(),
                                            accumulated_stderr: accumulated_stderr.clone(),
                                        },
                                    ));
                                }
                            }

                            // Not a break or continue signal, propagate the error
                            return Err(e);
                        }
                    }
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        // Exit loop context
        self.runtime.exit_loop();

        result
    }

    fn execute_while_loop(&mut self, while_loop: WhileLoop) -> Result<ExecutionResult> {
        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            loop {
                // Evaluate condition
                let mut condition_exit_code = 0;
                for statement in &while_loop.condition {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            condition_exit_code = result.exit_code;
                        }
                        Err(e) => return Err(e),
                    }
                }

                // While loop continues while condition is true (exit code 0)
                if condition_exit_code != 0 {
                    break;
                }

                // Execute body
                for statement in &while_loop.body {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            accumulated_stdout.push_str(&result.stdout());
                            accumulated_stderr.push_str(&result.stderr);
                            last_exit_code = result.exit_code;
                        }
                        Err(e) => {
                            // Check if this is a break signal
                            if let Some(break_signal) =
                                e.downcast_ref::<crate::builtins::break_builtin::BreakSignal>()
                            {
                                // First, add any accumulated output from the break signal itself
                                accumulated_stdout.push_str(&break_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&break_signal.accumulated_stderr);

                                if break_signal.levels == 1 {
                                    // Break from this loop, return accumulated output
                                    return Ok(ExecutionResult {
                                        output: Output::Text(accumulated_stdout),
                                        stderr: accumulated_stderr,
                                        exit_code: last_exit_code,
                                        error: None,
                                    });
                                } else {
                                    // Propagate to outer loop with decreased level and accumulated output
                                    return Err(anyhow::Error::new(
                                        crate::builtins::break_builtin::BreakSignal {
                                            levels: break_signal.levels - 1,
                                            accumulated_stdout: accumulated_stdout.clone(),
                                            accumulated_stderr: accumulated_stderr.clone(),
                                        },
                                    ));
                                }
                            }

                            // Check if this is a continue signal
                            if let Some(continue_signal) =
                                e.downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>(
                                )
                            {
                                // First, add any accumulated output from the continue signal itself
                                accumulated_stdout.push_str(&continue_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&continue_signal.accumulated_stderr);

                                if continue_signal.levels == 1 {
                                    // Continue in this loop - skip to next iteration
                                    break; // Break out of the statement loop, continue with next item
                                } else {
                                    // Propagate to outer loop with decreased level and accumulated output
                                    return Err(anyhow::Error::new(
                                        crate::builtins::continue_builtin::ContinueSignal {
                                            levels: continue_signal.levels - 1,
                                            accumulated_stdout: accumulated_stdout.clone(),
                                            accumulated_stderr: accumulated_stderr.clone(),
                                        },
                                    ));
                                }
                            }

                            return Err(e);
                        }
                    }
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        self.runtime.exit_loop();
        result
    }

    fn execute_until_loop(&mut self, until_loop: UntilLoop) -> Result<ExecutionResult> {
        // Enter loop context for break/continue
        self.runtime.enter_loop();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;

        let result = (|| -> Result<ExecutionResult> {
            loop {
                // Evaluate condition
                let mut condition_exit_code = 0;
                for statement in &until_loop.condition {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            condition_exit_code = result.exit_code;
                        }
                        Err(e) => return Err(e),
                    }
                }

                // Until loop continues until condition is true (exit code 0)
                // So we break when exit code is 0
                if condition_exit_code == 0 {
                    break;
                }

                // Execute body
                for statement in &until_loop.body {
                    match self.execute_statement(statement.clone()) {
                        Ok(result) => {
                            accumulated_stdout.push_str(&result.stdout());
                            accumulated_stderr.push_str(&result.stderr);
                            last_exit_code = result.exit_code;
                        }
                        Err(e) => {
                            // Check if this is a break signal
                            if let Some(break_signal) =
                                e.downcast_ref::<crate::builtins::break_builtin::BreakSignal>()
                            {
                                accumulated_stdout.push_str(&break_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&break_signal.accumulated_stderr);
                                self.runtime.exit_loop();
                                return Ok(ExecutionResult {
                                    output: Output::Text(accumulated_stdout),
                                    stderr: accumulated_stderr,
                                    exit_code: last_exit_code,
                                    error: None,
                                });
                            }

                            // Check if this is a continue signal
                            if let Some(continue_signal) =
                                e.downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>(
                                )
                            {
                                accumulated_stdout.push_str(&continue_signal.accumulated_stdout);
                                accumulated_stderr.push_str(&continue_signal.accumulated_stderr);
                                break; // Break inner loop to continue outer loop
                            }

                            return Err(e);
                        }
                    }
                }
            }
            Ok(ExecutionResult {
                output: Output::Text(accumulated_stdout),
                stderr: accumulated_stderr,
                exit_code: last_exit_code,
                error: None,
            })
        })();

        self.runtime.exit_loop();
        result
    }

    fn execute_match(&mut self, match_expr: MatchExpression) -> Result<ExecutionResult> {
        let value = self.evaluate_expression(match_expr.value)?;

        for arm in match_expr.arms {
            if self.pattern_matches(&arm.pattern, &value) {
                for statement in arm.body {
                    self.execute_statement(statement)?;
                }
                break;
            }
        }

        Ok(ExecutionResult::default())
    }
    fn execute_case(&mut self, case_stmt: CaseStatement) -> Result<ExecutionResult> {
        // Evaluate the word to match against
        let word_value = self.evaluate_expression(case_stmt.word)?;
        let word = word_value.trim();

        let mut accumulated_stdout = String::new();
        let mut accumulated_stderr = String::new();
        let mut last_exit_code = 0;
        let mut matched = false;

        // Try each case arm in order
        for arm in case_stmt.arms {
            // Check if any of the patterns match
            for pattern_str in &arm.patterns {
                if self.case_pattern_matches(pattern_str, word) {
                    matched = true;

                    // Execute the body statements
                    for statement in &arm.body {
                        let result = self.execute_statement(statement.clone())?;
                        accumulated_stdout.push_str(&result.stdout());
                        accumulated_stderr.push_str(&result.stderr);
                        last_exit_code = result.exit_code;
                    }

                    // Break from this arm after execution (POSIX: only first match executes)
                    break;
                }
            }

            // If we found a match, don't check remaining arms
            if matched {
                break;
            }
        }

        // POSIX: exit code is last command in executed list, or 0 if no match
        Ok(ExecutionResult {
            output: Output::Text(accumulated_stdout),
            stderr: accumulated_stderr,
            exit_code: if matched { last_exit_code } else { 0 },
            error: None,
        })
    }

    /// Match a pattern against a word for case statements
    /// Supports glob-style patterns: *, ?, [...]
    fn case_pattern_matches(&self, pattern: &str, word: &str) -> bool {
        // Use glob crate's Pattern for matching
        match glob::Pattern::new(pattern) {
            Ok(glob_pattern) => glob_pattern.matches(word),
            Err(_) => {
                // If pattern is invalid, fall back to literal match
                pattern == word
            }
        }
    }

    fn execute_conditional_and(&mut self, cond_and: ConditionalAnd) -> Result<ExecutionResult> {
        // Execute left side
        let left_result = self.execute_statement(*cond_and.left)?;
        self.runtime.set_last_exit_code(left_result.exit_code);

        // Only execute right side if left succeeded (exit code 0)
        if left_result.exit_code == 0 {
            let right_result = self.execute_statement(*cond_and.right)?;
            self.runtime.set_last_exit_code(right_result.exit_code);

            Ok(ExecutionResult {
                output: Output::Text(format!("{}{}", left_result.stdout(), right_result.stdout())),
                stderr: format!("{}{}", left_result.stderr, right_result.stderr),
                exit_code: right_result.exit_code,
                error: right_result.error,
            })
        } else {
            // Left failed, return its result
            Ok(left_result)
        }
    }

    fn execute_conditional_or(&mut self, cond_or: ConditionalOr) -> Result<ExecutionResult> {
        // Execute left side
        let left_result = self.execute_statement(*cond_or.left)?;
        self.runtime.set_last_exit_code(left_result.exit_code);

        // Only execute right side if left failed (exit code != 0)
        if left_result.exit_code != 0 {
            let right_result = self.execute_statement(*cond_or.right)?;
            self.runtime.set_last_exit_code(right_result.exit_code);

            Ok(ExecutionResult {
                output: Output::Text(format!("{}{}", left_result.stdout(), right_result.stdout())),
                stderr: format!("{}{}", left_result.stderr, right_result.stderr),
                exit_code: right_result.exit_code,
                error: right_result.error,
            })
        } else {
            // Left succeeded, return its result
            Ok(left_result)
        }
    }

    fn execute_subshell(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
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
        };

        // Execute all statements in the subshell, catching ExitSignal
        let result = match child_executor.execute(statements) {
            Ok(r) => r,
            Err(e) => {
                if let Some(exit_sig) =
                    e.downcast_ref::<crate::builtins::exit_builtin::ExitSignal>()
                {
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
    fn execute_brace_group(&mut self, statements: Vec<Statement>) -> Result<ExecutionResult> {
        // Execute statements in current context (not isolated like subshell)
        self.execute(statements)
    }

    /// Check if a statement is an exec command (which replaces the process).
    /// This is used to flush accumulated output before exec, since exec
    /// replaces the process and any buffered output would be lost.
    fn is_exec_command(statement: &Statement) -> bool {
        match statement {
            Statement::Command(cmd) => cmd.name == "exec",
            // Handle case where exec might be in a conditional
            Statement::ConditionalAnd(cond) => Self::is_exec_command(&cond.right),
            Statement::ConditionalOr(cond) => Self::is_exec_command(&cond.right),
            _ => false,
        }
    }

    fn execute_background(&mut self, statement: Statement) -> Result<ExecutionResult> {
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

                // Use pre_exec to set the process group before the child executes
                unsafe {
                    cmd.pre_exec(|| {
                        // Put this process in its own process group (PGID = PID)
                        let pid = getpid();
                        setpgid(pid, pid).map_err(|e| {
                            std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("setpgid failed: {}", e),
                            )
                        })?;
                        Ok(())
                    });
                }

                let child = cmd.spawn().map_err(|e| {
                    anyhow!(
                        "Failed to spawn background process '{}': {}",
                        command.name,
                        e
                    )
                })?;

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
            _ => Err(anyhow!(
                "Only simple commands and pipelines can be run in background"
            )),
        }
    }

    /// Execute a complex statement in background by wrapping it in sh -c
    fn execute_background_via_sh(&mut self, command_str: &str) -> Result<ExecutionResult> {
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

        unsafe {
            cmd.pre_exec(|| {
                let pid = getpid();
                setpgid(pid, pid).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("setpgid failed: {}", e))
                })?;
                Ok(())
            });
        }

        let child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn background process: {}", e))?;

        let pid = child.id();
        let job_id = self
            .runtime
            .job_manager()
            .add_job(pid, command_str.to_string());
        self.runtime.set_last_bg_pid(pid);

        Ok(ExecutionResult::success(format!("[{}] {}\n", job_id, pid)))
    }

    fn statement_to_string(&self, statement: &Statement) -> String {
        match statement {
            Statement::Command(cmd) => {
                let args_str = cmd
                    .args
                    .iter()
                    .map(|arg| match arg {
                        Argument::Literal(s)
                        | Argument::SingleQuoted(s)
                        | Argument::Variable(s)
                        | Argument::BracedVariable(s)
                        | Argument::CommandSubstitution(s)
                        | Argument::Flag(s)
                        | Argument::Path(s)
                        | Argument::Glob(s) => s.clone(),
                        Argument::DoubleQuoted(parts) => parts
                            .iter()
                            .map(|p| match p {
                                ArgumentPart::Literal(s) => s.clone(),
                                ArgumentPart::Variable(s)
                                | ArgumentPart::BracedVariable(s)
                                | ArgumentPart::CommandSubstitution(s) => s.clone(),
                            })
                            .collect::<Vec<_>>()
                            .join(""),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if args_str.is_empty() {
                    cmd.name.clone()
                } else {
                    format!("{} {}", cmd.name, args_str)
                }
            }
            Statement::WhileLoop(_) => "while loop".to_string(),
            Statement::UntilLoop(_) => "until loop".to_string(),
            _ => "complex command".to_string(),
        }
    }

    fn command_to_string(cmd: &crate::parser::ast::Command) -> String {
        let args_str = cmd
            .args
            .iter()
            .map(|arg| match arg {
                Argument::Literal(s)
                | Argument::SingleQuoted(s)
                | Argument::Variable(s)
                | Argument::BracedVariable(s)
                | Argument::CommandSubstitution(s)
                | Argument::Flag(s)
                | Argument::Path(s)
                | Argument::Glob(s) => s.clone(),
                Argument::DoubleQuoted(parts) => parts
                    .iter()
                    .map(|p| match p {
                        ArgumentPart::Literal(s) => s.clone(),
                        ArgumentPart::Variable(s)
                        | ArgumentPart::BracedVariable(s)
                        | ArgumentPart::CommandSubstitution(s) => s.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            })
            .collect::<Vec<_>>()
            .join(" ");
        if args_str.is_empty() {
            cmd.name.clone()
        } else {
            format!("{} {}", cmd.name, args_str)
        }
    }

    fn evaluate_expression(&mut self, expr: Expression) -> Result<String> {
        match expr {
            Expression::Literal(Literal::String(ref s))
                if s.starts_with("$((") && s.ends_with("))") =>
            {
                // Arithmetic expansion in string literal context (e.g., i=$((i+1)))
                let inner = &s[3..s.len() - 2];
                let result = arithmetic::evaluate_mut(inner, &mut self.runtime)?;
                Ok(result.to_string())
            }
            Expression::Literal(Literal::String(ref s))
                if s.starts_with("$(") && s.ends_with(')') =>
            {
                // Command substitution in string literal context
                self.execute_command_substitution(s)
            }
            Expression::Literal(Literal::String(ref s)) if s.contains("$(") || s.contains('`') => {
                // Embedded command substitution in string literal context
                self.expand_command_substitutions_in_string(s)
            }
            Expression::Literal(Literal::String(ref s)) if s.starts_with('$') => {
                // Variable expansion in string literal context
                let var_name = s.trim_start_matches('$');
                Ok(self.runtime.get_variable(var_name).unwrap_or_default())
            }
            Expression::Literal(lit) => Ok(self.literal_to_string(lit)),
            Expression::Variable(name) => {
                // Strip single $ from variable name (use strip_prefix to remove only one $)
                let var_name = name.strip_prefix('$').unwrap_or(&name);

                // Handle special variables first
                if var_name == "$" {
                    return Ok(std::process::id().to_string());
                } else if var_name == "!" {
                    return Ok(self
                        .runtime
                        .get_last_bg_pid()
                        .map(|pid| pid.to_string())
                        .unwrap_or_default());
                } else if var_name == "-" {
                    return Ok(self.runtime.get_option_flags());
                } else if var_name == "_" {
                    return Ok(self.runtime.get_last_arg().to_string());
                } else if var_name == "#" {
                    return Ok(self.runtime.param_count().to_string());
                } else if var_name == "@" {
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if var_name == "*" {
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if var_name == "0" {
                    if let Some(val) = self.runtime.get_variable("0") {
                        return Ok(val);
                    } else {
                        return Ok("rush".to_string());
                    }
                } else if var_name == "?" {
                    return Ok(self.runtime.get_last_exit_code().to_string());
                } else if let Ok(index) = var_name.parse::<usize>() {
                    if index > 0 {
                        return Ok(self.runtime.get_positional_param(index).unwrap_or_default());
                    }
                }

                // Regular variable expansion
                // Use get_variable_checked to respect nounset option
                if self.runtime.options.nounset {
                    self.runtime.get_variable_checked(var_name)
                } else {
                    Ok(self.runtime.get_variable(var_name).unwrap_or_default())
                }
            }
            Expression::VariableExpansion(expansion) => self.runtime.expand_variable(&expansion),
            Expression::CommandSubstitution(cmd) => {
                // Check for arithmetic expansion: $((expr))
                if cmd.starts_with("$((") && cmd.ends_with("))") {
                    let expr = &cmd[3..cmd.len() - 2];
                    let result = arithmetic::evaluate_mut(expr, &mut self.runtime)?;
                    return Ok(result.to_string());
                }
                // Strip $( and )
                let cmd_str = cmd.trim_start_matches("$(").trim_end_matches(')');
                // TODO: Parse and execute the command
                Ok(cmd_str.to_string())
            }
            Expression::FunctionCall(call) => {
                // Evaluate arguments
                let mut args = Vec::new();
                for arg_expr in call.args {
                    args.push(self.evaluate_expression(arg_expr)?);
                }
                // Execute the function and return its stdout
                let result = self.execute_user_function(&call.name, args)?;
                Ok(result.stdout())
            }
            _ => Err(anyhow!("Expression evaluation not yet implemented")),
        }
    }

    fn resolve_argument(&mut self, arg: &Argument) -> Result<String> {
        match arg {
            Argument::Literal(s) => {
                // Expand variables and command substitutions in literal strings
                self.expand_variables_in_literal(s)
            }
            Argument::Variable(var) => {
                // Strip single $ from variable name (use strip_prefix to remove only one $)
                let var_name = var.strip_prefix('$').unwrap_or(var);

                // Handle special variables first
                if var_name == "$" {
                    // $$ - process ID of the shell
                    return Ok(std::process::id().to_string());
                } else if var_name == "!" {
                    // $! - PID of last background command
                    return Ok(self
                        .runtime
                        .get_last_bg_pid()
                        .map(|pid| pid.to_string())
                        .unwrap_or_default());
                } else if var_name == "-" {
                    // $- - current option flags
                    return Ok(self.runtime.get_option_flags());
                } else if var_name == "_" {
                    // $_ - last argument of previous command
                    return Ok(self.runtime.get_last_arg().to_string());
                } else if var_name == "#" {
                    // $# - number of positional parameters
                    return Ok(self.runtime.param_count().to_string());
                } else if var_name == "@" {
                    // $@ - all positional parameters as separate words
                    // For now, return as space-separated string (proper quoting handled later)
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if var_name == "*" {
                    // $* - all positional parameters
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if var_name == "0" {
                    // $0 - shell name or script name
                    if let Some(val) = self.runtime.get_variable("0") {
                        return Ok(val);
                    } else {
                        return Ok("rush".to_string());
                    }
                } else if var_name == "?" {
                    return Ok(self.runtime.get_last_exit_code().to_string());
                } else if let Ok(index) = var_name.parse::<usize>() {
                    // $1, $2, etc. - positional parameters
                    if index > 0 {
                        return Ok(self.runtime.get_positional_param(index).unwrap_or_default());
                    }
                }

                // Regular variable - just get its value
                Ok(self.runtime.get_variable(var_name).unwrap_or_default())
            }
            Argument::BracedVariable(braced_var) => {
                // Parse the braced variable expansion
                let expansion = self.parse_braced_var_expansion(braced_var)?;

                // Handle special variables in braced expansions
                if expansion.name == "$" {
                    // ${$} - process ID of the shell (no operators allowed)
                    return Ok(std::process::id().to_string());
                } else if expansion.name == "!" {
                    // ${!} - PID of last background command (no operators allowed)
                    return Ok(self
                        .runtime
                        .get_last_bg_pid()
                        .map(|pid| pid.to_string())
                        .unwrap_or_default());
                } else if expansion.name == "-" {
                    // ${-} - current option flags (no operators allowed)
                    return Ok(self.runtime.get_option_flags());
                } else if expansion.name == "_" {
                    // ${_} - last argument of previous command (no operators allowed)
                    return Ok(self.runtime.get_last_arg().to_string());
                } else if expansion.name == "#" {
                    // ${#} - number of positional parameters
                    return Ok(self.runtime.param_count().to_string());
                } else if expansion.name == "@" {
                    // ${@} - all positional parameters
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if expansion.name == "*" {
                    // ${*} - all positional parameters
                    return Ok(self.runtime.get_positional_params().join(" "));
                } else if expansion.name == "0" {
                    // ${0} - shell name or script name
                    if let Some(val) = self.runtime.get_variable("0") {
                        return Ok(val);
                    } else {
                        return Ok("rush".to_string());
                    }
                } else if let Ok(index) = expansion.name.parse::<usize>() {
                    // ${1}, ${2}, ${10}, etc. - positional parameters
                    if index > 0 {
                        // Check if positional param exists
                        if let Some(value) = self.runtime.get_positional_param(index) {
                            // Param exists - set it in temp runtime and apply operator
                            let mut temp_runtime = self.runtime.clone();
                            temp_runtime.set_variable(expansion.name.clone(), value.clone());
                            return temp_runtime.expand_variable(&expansion);
                        } else {
                            // Param doesn't exist - apply operator to None
                            let mut temp_runtime = self.runtime.clone();
                            // Don't set the variable - let it be unset so operators work correctly
                            return temp_runtime.expand_variable(&expansion);
                        }
                    }
                }

                // Expand it using the runtime
                self.runtime.expand_variable(&expansion)
            }
            Argument::CommandSubstitution(cmd) => {
                // Check for arithmetic expansion: $((expr))
                if cmd.starts_with("$((") && cmd.ends_with("))") {
                    let expr = &cmd[3..cmd.len() - 2];
                    let result = arithmetic::evaluate_mut(expr, &mut self.runtime)?;
                    return Ok(result.to_string());
                }
                // Execute command substitution and return output
                Ok(self
                    .execute_command_substitution(cmd)
                    .unwrap_or_else(|_| String::new()))
            }
            Argument::Flag(f) => Ok(f.clone()),
            Argument::Path(p) => Ok(expand_tilde(p)),
            Argument::Glob(g) => Ok(g.clone()),
            Argument::SingleQuoted(s) => Ok(s.clone()),
            Argument::DoubleQuoted(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        ArgumentPart::Literal(s) => result.push_str(s),
                        ArgumentPart::Variable(v) => {
                            result.push_str(
                                &self
                                    .resolve_argument(&Argument::Variable(v.clone()))
                                    .unwrap_or_default(),
                            );
                        }
                        ArgumentPart::BracedVariable(v) => {
                            result.push_str(
                                &self
                                    .resolve_argument(&Argument::BracedVariable(v.clone()))
                                    .unwrap_or_default(),
                            );
                        }
                        ArgumentPart::CommandSubstitution(c) => {
                            result.push_str(
                                &self
                                    .resolve_argument(&Argument::CommandSubstitution(c.clone()))
                                    .unwrap_or_default(),
                            );
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    fn parse_braced_var_expansion(&self, braced_var: &str) -> Result<VarExpansion> {
        // Remove ${ and } from the string
        let inner = braced_var.trim_start_matches("${").trim_end_matches('}');

        // String length: ${#var}
        if inner.starts_with('#')
            && !inner.contains(':')
            && !inner[1..].contains('#')
            && !inner[1..].contains('%')
        {
            let var_name = &inner[1..];
            return Ok(VarExpansion {
                name: var_name.to_string(),
                operator: VarExpansionOp::StringLength,
            });
        }

        // Check for different operators in order
        if let Some(pos) = inner.find(":-") {
            let (name, default) = inner.split_at(pos);
            let default = &default[2..]; // Skip :-
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::UseDefault(default.to_string()),
            });
        }

        if let Some(pos) = inner.find(":=") {
            let (name, default) = inner.split_at(pos);
            let default = &default[2..]; // Skip :=
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::AssignDefault(default.to_string()),
            });
        }

        if let Some(pos) = inner.find(":?") {
            let (name, error_msg) = inner.split_at(pos);
            let error_msg = &error_msg[2..]; // Skip :?
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::ErrorIfUnset(error_msg.to_string()),
            });
        }

        if let Some(pos) = inner.find(":+") {
            let (name, alternate) = inner.split_at(pos);
            let alternate = &alternate[2..]; // Skip :+
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::UseAlternate(alternate.to_string()),
            });
        }

        if let Some(pos) = inner.find("##") {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[2..]; // Skip ##
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveLongestPrefix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find('#') {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[1..]; // Skip #
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveShortestPrefix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find("%%") {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[2..]; // Skip %%
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveLongestSuffix(pattern.to_string()),
            });
        }

        if let Some(pos) = inner.find('%') {
            let (name, pattern) = inner.split_at(pos);
            let pattern = &pattern[1..]; // Skip %
            return Ok(VarExpansion {
                name: name.to_string(),
                operator: VarExpansionOp::RemoveShortestSuffix(pattern.to_string()),
            });
        }

        // No operator, just simple expansion
        Ok(VarExpansion {
            name: inner.to_string(),
            operator: VarExpansionOp::Simple,
        })
    }

    /// Expand globs and resolve arguments
    fn expand_and_resolve_arguments(&mut self, args: &[Argument]) -> Result<Vec<String>> {
        let mut expanded_args = Vec::new();

        for arg in args {
            // Determine if this argument should be subject to IFS splitting
            // Only unquoted variables and command substitutions should be split
            let should_split_ifs = matches!(
                arg,
                Argument::Variable(_)
                    | Argument::BracedVariable(_)
                    | Argument::CommandSubstitution(_)
            );

            // Determine if this argument should have glob expansion
            // Glob patterns from the lexer (Argument::Glob) and unquoted variables should expand
            // Quoted strings (Argument::Literal from quoted tokens) should NOT expand
            // Path is included because paths like /tmp/*.txt are tokenized as Path by the lexer
            let should_expand = matches!(
                arg,
                Argument::Glob(_)
                    | Argument::Path(_)
                    | Argument::Variable(_)
                    | Argument::BracedVariable(_)
                    | Argument::CommandSubstitution(_)
            );

            // First resolve the argument (e.g., variable substitution)
            let resolved = self.resolve_argument(arg)?;

            if should_split_ifs {
                // Apply IFS splitting first
                let fields = self.runtime.split_by_ifs(&resolved);

                // Then check each field for glob patterns
                for field in fields {
                    if glob_expansion::should_expand_glob(field) {
                        match glob_expansion::expand_globs(field, self.runtime.get_cwd()) {
                            Ok(matches) => {
                                expanded_args.extend(matches);
                            }
                            Err(_) => {
                                // No matches - return literal (POSIX behavior)
                                expanded_args.push(field.to_string());
                            }
                        }
                    } else {
                        // Not a glob pattern, just add the field
                        expanded_args.push(field.to_string());
                    }
                }
            } else if should_expand {
                // Unquoted glob or path pattern - expand it
                if glob_expansion::should_expand_glob(&resolved) {
                    match glob_expansion::expand_globs(&resolved, self.runtime.get_cwd()) {
                        Ok(matches) => {
                            expanded_args.extend(matches);
                        }
                        Err(_) => {
                            // No matches - return literal (POSIX behavior)
                            expanded_args.push(resolved);
                        }
                    }
                } else {
                    expanded_args.push(resolved);
                }
            } else {
                // Quoted literal or flag - no glob expansion
                expanded_args.push(resolved);
            }
        }

        Ok(expanded_args)
    }

    /// Execute a command substitution and return its stdout, trimmed
    fn execute_command_substitution(&self, cmd_str: &str) -> Result<String> {
        // Check for arithmetic expansion: $((expr))
        if cmd_str.starts_with("$((") && cmd_str.ends_with("))") {
            let expr = &cmd_str[3..cmd_str.len() - 2];
            let result = arithmetic::evaluate(expr, &self.runtime)?;
            return Ok(result.to_string());
        }

        use crate::lexer::Lexer;
        use crate::parser::Parser;

        // Extract command from $(...) or `...`
        let command = if cmd_str.starts_with("$(") && cmd_str.ends_with(')') {
            &cmd_str[2..cmd_str.len() - 1]
        } else if cmd_str.starts_with('`') && cmd_str.ends_with('`') {
            &cmd_str[1..cmd_str.len() - 1]
        } else {
            cmd_str
        };

        // Parse and execute the command
        let tokens = Lexer::tokenize(command)
            .map_err(|e| anyhow!("Failed to tokenize command substitution: {}", e))?;
        let mut parser = Parser::new(tokens);
        let statements = parser
            .parse()
            .map_err(|e| anyhow!("Failed to parse command substitution: {}", e))?;

        // Create a new executor with the same runtime (but cloned to avoid borrow issues)
        let mut sub_executor = Executor {
            runtime: self.runtime.clone(),
            builtins: self.builtins.clone(),
            corrector: self.corrector.clone(),
            suggestion_engine: self.suggestion_engine.clone(),
            signal_handler: None,
            show_progress: false, // Don't show progress for substitutions
            terminal_control: self.terminal_control.clone(),
            call_stack: CallStack::new(),
            profile_data: None,
            enable_profiling: false,
        };

        // Execute the command and capture output
        let result = sub_executor.execute(statements)?;
        let mut output = result.stdout();
        let max = max_substitution_output();
        if output.len() > max {
            output.truncate(max);
            while !output.is_char_boundary(output.len()) {
                output.pop();
            }
            eprintln!(
                "rush: warning: command substitution output truncated at {} bytes",
                max
            );
        }

        // Return stdout with trailing newlines trimmed (bash behavior)
        Ok(output.trim_end().to_string())
    }

    /// Expand all command substitution sequences ($(...) and `...`) within a string.
    /// Handles nested substitutions by delegating to execute_command_substitution.
    fn expand_command_substitutions_in_string(&self, input: &str) -> Result<String> {
        let mut result = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if i + 1 < len && bytes[i] == b'$' && bytes[i + 1] == b'(' {
                // Found $( -- find the matching closing paren, respecting nesting
                let start = i;
                let mut depth: i32 = 1;
                let mut j = i + 2;

                while j < len && depth > 0 {
                    match bytes[j] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b'\'' => {
                            j += 1;
                            while j < len && bytes[j] != b'\'' {
                                j += 1;
                            }
                        }
                        b'"' => {
                            j += 1;
                            while j < len {
                                if bytes[j] == b'"' {
                                    break;
                                }
                                if bytes[j] == b'\\' {
                                    j += 1;
                                }
                                j += 1;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if depth == 0 {
                    let substitution = &input[start..j];
                    let output = self
                        .execute_command_substitution(substitution)
                        .unwrap_or_default();
                    result.push_str(&output);
                    i = j;
                } else {
                    result.push(bytes[i] as char);
                    i += 1;
                }
            } else if bytes[i] == b'`' {
                // Backtick substitution -- find matching closing backtick
                let start = i;
                let mut j = i + 1;

                while j < len {
                    if bytes[j] == b'`' {
                        j += 1;
                        break;
                    } else if bytes[j] == b'\\' && j + 1 < len {
                        j += 2;
                    } else {
                        j += 1;
                    }
                }

                if j <= len && j > start + 1 && bytes[j - 1] == b'`' {
                    let substitution = &input[start..j];
                    let output = self
                        .execute_command_substitution(substitution)
                        .unwrap_or_default();
                    result.push_str(&output);
                    i = j;
                } else {
                    result.push(bytes[i] as char);
                    i += 1;
                }
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        Ok(result)
    }

    fn literal_to_string(&self, lit: Literal) -> String {
        match lit {
            Literal::String(s) => s,
            Literal::Integer(n) => n.to_string(),
            Literal::Float(f) => f.to_string(),
            Literal::Boolean(b) => b.to_string(),
        }
    }

    fn is_truthy(&self, value: &str) -> bool {
        !value.is_empty() && value != "0" && value != "false"
    }

    fn pattern_matches(&self, pattern: &Pattern, value: &str) -> bool {
        match pattern {
            Pattern::Identifier(id) => id == value,
            Pattern::Literal(lit) => self.literal_to_string(lit.clone()) == value,
            Pattern::Wildcard => true,
        }
    }

    /// Reset executor state between command executions.
    /// Clears runtime state (variables, scopes, call stack, etc.)
    /// while preserving long-lived resources (history, job_manager, builtins, corrector).
    pub fn reset(&mut self) -> Result<()> {
        self.runtime.reset()
    }

    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Execute a trap handler for the given signal
    /// Returns Ok(()) if trap was executed successfully or if no trap is set
    /// Returns Err if trap execution failed
    pub fn execute_trap(&mut self, signal: crate::builtins::trap::TrapSignal) -> Result<()> {
        // Get the trap command for this signal
        let trap_command = match self.runtime.get_trap(signal) {
            Some(cmd) => cmd.clone(),
            None => return Ok(()), // No trap set, nothing to do
        };

        // Empty command means ignore the signal
        if trap_command.is_empty() {
            return Ok(());
        }

        // Execute the trap command
        use crate::lexer::Lexer;
        use crate::parser::Parser;

        let tokens = Lexer::tokenize(&trap_command)
            .map_err(|e| anyhow!("Failed to tokenize trap command: {}", e))?;
        let mut parser = Parser::new(tokens);
        let statements = parser
            .parse()
            .map_err(|e| anyhow!("Failed to parse trap command: {}", e))?;

        // Execute the trap (errors are logged but don't stop execution)
        match self.execute(statements) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Print error but don't fail - traps should be resilient
                eprintln!(
                    "trap: error executing {} handler: {}",
                    signal.to_string(),
                    e
                );
                Ok(())
            }
        }
    }

    /// Execute the EXIT trap if one is set
    /// This should be called before the shell exits
    pub fn execute_exit_trap(&mut self) {
        let _ = self.execute_trap(crate::builtins::trap::TrapSignal::Exit);
    }

    /// Source a file by executing its contents line by line
    /// Used for .rushrc and .rush_profile files
    pub fn source_file(&mut self, path: &std::path::Path) -> Result<()> {
        use std::fs;
        use std::io::{BufRead, BufReader};

        // Check if file exists
        if !path.exists() {
            return Ok(()); // Silently ignore missing config files
        }

        // Read file
        let file = fs::File::open(path)
            .map_err(|e| anyhow!("Failed to open '{}': {}", path.display(), e))?;
        let reader = BufReader::new(file);

        // Execute each line
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Execute the line
            match self.execute_line_internal(line) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}:{}: {}", path.display(), line_num + 1, e);
                    // Continue executing other lines even if one fails
                }
            }
        }

        Ok(())
    }

    /// Internal helper to execute a single line
    fn execute_line_internal(&mut self, line: &str) -> Result<ExecutionResult> {
        use crate::lexer::Lexer;
        use crate::parser::Parser;

        let tokens = Lexer::tokenize(line)?;
        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;
        self.execute(statements)
    }
}

/// Expand tilde (`~`) at the start of a path to the user's home directory.
///
/// - `~` expands to `$HOME`
/// - `~/path` expands to `$HOME/path`
/// - `~user` expands to that user's home directory (via passwd lookup)
/// - Paths not starting with `~` are returned unchanged
pub fn expand_tilde(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_string();
    }

    // Standalone ~ or ~/path
    if path == "~" || path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            if path == "~" {
                return home;
            }
            // ~/path -> $HOME/path
            return format!("{}{}", home, &path[1..]);
        }
        return path.to_string();
    }

    // ~user or ~user/path
    let rest = &path[1..];
    let (username, suffix) = match rest.find('/') {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, ""),
    };

    // Look up user's home directory via libc getpwnam
    use std::ffi::CString;
    if let Ok(c_username) = CString::new(username) {
        // SAFETY: getpwnam is a standard POSIX function
        let pw = unsafe { libc::getpwnam(c_username.as_ptr()) };
        if !pw.is_null() {
            let home_dir = unsafe { std::ffi::CStr::from_ptr((*pw).pw_dir) };
            if let Ok(home) = home_dir.to_str() {
                return format!("{}{}", home, suffix);
            }
        }
    }

    // If user lookup fails, return unchanged
    path.to_string()
}

fn resolve_argument_static(
    arg: &Argument,
    runtime: &Runtime,
    builtins: &Builtins,
    corrector: &Corrector,
) -> String {
    match arg {
        Argument::Literal(s) => {
            if s.contains("$(") || s.contains('`') {
                expand_command_substitutions_in_string_static(s, runtime, builtins, corrector)
            } else {
                s.clone()
            }
        }
        Argument::Variable(var) => {
            let var_name = var.trim_start_matches('$');
            runtime
                .get_variable(var_name)
                .unwrap_or_else(|| var.clone())
        }
        Argument::BracedVariable(var) => {
            // Strip ${ and } from variable name
            let var_name = var.trim_start_matches("${").trim_end_matches('}');
            runtime
                .get_variable(var_name)
                .unwrap_or_else(|| var.clone())
        }
        Argument::CommandSubstitution(cmd) => {
            // Check for arithmetic expansion: $((expr))
            if cmd.starts_with("$((") && cmd.ends_with("))") {
                let expr = &cmd[3..cmd.len() - 2];
                return arithmetic::evaluate(expr, runtime)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| "0".to_string());
            }
            // For parallel execution, we need to execute command substitution
            // Create a minimal executor for this

            let command = if cmd.starts_with("$(") && cmd.ends_with(')') {
                &cmd[2..cmd.len() - 1]
            } else if cmd.starts_with('`') && cmd.ends_with('`') {
                &cmd[1..cmd.len() - 1]
            } else {
                cmd.as_str()
            };

            // Try to execute the command substitution
            if let Ok(tokens) = crate::lexer::Lexer::tokenize(command) {
                let mut parser = crate::parser::Parser::new(tokens);
                if let Ok(statements) = parser.parse() {
                    let mut sub_executor = Executor {
                        runtime: runtime.clone(),
                        builtins: builtins.clone(),
                        corrector: corrector.clone(),
                        suggestion_engine: SuggestionEngine::new(),
                        signal_handler: None,
                        show_progress: false,
                        terminal_control: TerminalControl::new(),
                        call_stack: CallStack::new(),
                        profile_data: None,
                        enable_profiling: false,
                    };
                    if let Ok(exec_result) = sub_executor.execute(statements) {
                        let mut out = exec_result.stdout();
                        let max = max_substitution_output();
                        if out.len() > max {
                            out.truncate(max);
                            while !out.is_char_boundary(out.len()) {
                                out.pop();
                            }
                            eprintln!(
                                "rush: warning: command substitution output truncated at {} bytes",
                                max
                            );
                        }
                        return out.trim_end().to_string();
                    }
                }
            }

            // If execution failed, return empty string
            String::new()
        }
        Argument::Flag(f) => f.clone(),
        Argument::Path(p) => expand_tilde(p),
        Argument::Glob(g) => g.clone(),
        Argument::SingleQuoted(s) => s.clone(),
        Argument::DoubleQuoted(parts) => {
            let mut result = String::new();
            for part in parts {
                match part {
                    ArgumentPart::Literal(s) => result.push_str(s),
                    ArgumentPart::Variable(v) => {
                        result.push_str(&resolve_argument_static(
                            &Argument::Variable(v.clone()),
                            runtime,
                            builtins,
                            corrector,
                        ));
                    }
                    ArgumentPart::BracedVariable(v) => {
                        result.push_str(&resolve_argument_static(
                            &Argument::BracedVariable(v.clone()),
                            runtime,
                            builtins,
                            corrector,
                        ));
                    }
                    ArgumentPart::CommandSubstitution(c) => {
                        result.push_str(&resolve_argument_static(
                            &Argument::CommandSubstitution(c.clone()),
                            runtime,
                            builtins,
                            corrector,
                        ));
                    }
                }
            }
            result
        }
    }
}

// Helper function for parallel execution with glob expansion
fn expand_and_resolve_arguments_static(
    args: &[Argument],
    runtime: &Runtime,
    builtins: &Builtins,
    corrector: &Corrector,
) -> Result<Vec<String>> {
    let mut expanded_args = Vec::new();

    for arg in args {
        // Only expand globs for Argument::Glob, Path, and variable types (not quoted Literals)
        // Path is included because paths like /tmp/*.txt are tokenized as Path by the lexer
        let should_expand = matches!(
            arg,
            Argument::Glob(_)
                | Argument::Path(_)
                | Argument::Variable(_)
                | Argument::BracedVariable(_)
                | Argument::CommandSubstitution(_)
        );

        let resolved = resolve_argument_static(arg, runtime, builtins, corrector);

        if should_expand && glob_expansion::should_expand_glob(&resolved) {
            match glob_expansion::expand_globs(&resolved, runtime.get_cwd()) {
                Ok(matches) => {
                    expanded_args.extend(matches);
                }
                Err(_) => {
                    // No matches - return literal (POSIX behavior)
                    expanded_args.push(resolved);
                }
            }
        } else {
            expanded_args.push(resolved);
        }
    }

    Ok(expanded_args)
}

/// Static version of command substitution expansion for use outside &mut self methods.
pub(crate) fn expand_command_substitutions_in_string_static(
    input: &str,
    runtime: &Runtime,
    builtins: &Builtins,
    corrector: &Corrector,
) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && bytes[i] == b'$' && bytes[i + 1] == b'(' {
            let start = i;
            let mut depth: i32 = 1;
            let mut j = i + 2;

            while j < len && depth > 0 {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    b'\'' => {
                        j += 1;
                        while j < len && bytes[j] != b'\'' {
                            j += 1;
                        }
                    }
                    b'"' => {
                        j += 1;
                        while j < len {
                            if bytes[j] == b'"' {
                                break;
                            }
                            if bytes[j] == b'\\' {
                                j += 1;
                            }
                            j += 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }

            if depth == 0 {
                let substitution = &input[start..j];

                // Check for arithmetic expansion: $((expr))
                if substitution.starts_with("$((") && substitution.ends_with("))") {
                    let expr = &substitution[3..substitution.len() - 2];
                    if let Ok(value) = arithmetic::evaluate(expr, runtime) {
                        result.push_str(&value.to_string());
                        i = j;
                        continue;
                    }
                }

                let command = &substitution[2..substitution.len() - 1];
                if let Ok(tokens) = crate::lexer::Lexer::tokenize(command) {
                    let mut parser = crate::parser::Parser::new(tokens);
                    if let Ok(statements) = parser.parse() {
                        let mut sub_executor = Executor {
                            runtime: runtime.clone(),
                            builtins: builtins.clone(),
                            corrector: corrector.clone(),
                            suggestion_engine: SuggestionEngine::new(),
                            signal_handler: None,
                            show_progress: false,
                            terminal_control: TerminalControl::new(),
                            call_stack: CallStack::new(),
                            profile_data: None,
                            enable_profiling: false,
                        };
                        if let Ok(exec_result) = sub_executor.execute(statements) {
                            let mut out = exec_result.stdout();
                            let max = max_substitution_output();
                            if out.len() > max {
                                out.truncate(max);
                                while !out.is_char_boundary(out.len()) {
                                    out.pop();
                                }
                                eprintln!("rush: warning: command substitution output truncated at {} bytes", max);
                            }
                            result.push_str(out.trim_end());
                            i = j;
                            continue;
                        }
                    }
                }
                result.push(bytes[i] as char);
                i += 1;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        } else if bytes[i] == b'`' {
            let start = i;
            let mut j = i + 1;
            while j < len {
                if bytes[j] == b'`' {
                    j += 1;
                    break;
                } else if bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                } else {
                    j += 1;
                }
            }
            if j <= len && j > start + 1 && bytes[j - 1] == b'`' {
                let command = &input[start + 1..j - 1];
                if let Ok(tokens) = crate::lexer::Lexer::tokenize(command) {
                    let mut parser = crate::parser::Parser::new(tokens);
                    if let Ok(statements) = parser.parse() {
                        let mut sub_executor = Executor {
                            runtime: runtime.clone(),
                            builtins: builtins.clone(),
                            corrector: corrector.clone(),
                            suggestion_engine: SuggestionEngine::new(),
                            signal_handler: None,
                            show_progress: false,
                            terminal_control: TerminalControl::new(),
                            call_stack: CallStack::new(),
                            profile_data: None,
                            enable_profiling: false,
                        };
                        if let Ok(exec_result) = sub_executor.execute(statements) {
                            let mut out = exec_result.stdout();
                            let max = max_substitution_output();
                            if out.len() > max {
                                out.truncate(max);
                                while !out.is_char_boundary(out.len()) {
                                    out.pop();
                                }
                                eprintln!("rush: warning: command substitution output truncated at {} bytes", max);
                            }
                            result.push_str(out.trim_end());
                            i = j;
                            continue;
                        }
                    }
                }
                result.push(bytes[i] as char);
                i += 1;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub output: Output,
    pub stderr: String,
    pub exit_code: i32,
    /// Optional typed error information
    pub error: Option<String>,
}

/// Output can be either traditional text or structured data
#[derive(Debug, Clone)]
pub enum Output {
    Text(String),
    Structured(serde_json::Value),
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            output: Output::Text(String::new()),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        }
    }
}

impl Output {
    /// Get the text representation of this output
    pub fn as_text(&self) -> String {
        match self {
            Output::Text(s) => s.clone(),
            Output::Structured(v) => {
                // Convert JSON value to pretty-printed string
                serde_json::to_string_pretty(v).unwrap_or_else(|_| String::new())
            }
        }
    }
}

impl ExecutionResult {
    pub fn success(text: String) -> Self {
        Self {
            output: Output::Text(text),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        }
    }

    pub fn error(stderr: String) -> Self {
        Self {
            output: Output::Text(String::new()),
            stderr,
            exit_code: 1,
            error: None,
        }
    }

    // /// Create an error result from a typed RushError
    // pub fn error_typed(error: crate::error::RushError) -> Self {
    //     let stderr = if crate::error::should_output_json_errors() {
    //         error.to_json()
    //     } else {
    //         error.to_text()
    //     };

    //     Self {
    //         output: Output::Text(String::new()),
    //         stderr,
    //         exit_code: error.exit_code,
    //         error: Some(error),
    //     }
    // }

    pub fn stdout(&self) -> String {
        self.output.as_text()
    }

    /// Get mutable reference to stdout text (only works for Text output)
    pub fn stdout_mut(&mut self) -> Option<&mut String> {
        match &mut self.output {
            Output::Text(s) => Some(s),
            Output::Structured(_) => None,
        }
    }

    /// Clear stdout content (only works for Text output)
    pub fn clear_stdout(&mut self) {
        if let Output::Text(s) = &mut self.output {
            s.clear();
        }
    }

    /// Append to stdout (only works for Text output)
    pub fn push_stdout(&mut self, text: &str) {
        if let Output::Text(s) = &mut self.output {
            s.push_str(text);
        }
    }
}

/// Render an `Output` value for display at the terminal boundary.
///
/// - `Output::Structured` containing an array of objects → pretty table via `TableRenderer`
/// - `Output::Structured` with any other shape → pretty-printed JSON
/// - `Output::Text` → returned as-is
///
/// When `compact_json` is true (e.g. agent mode), structured output is emitted as
/// compact single-line JSON rather than a human-readable table.
pub fn render_output(output: &Output, compact_json: bool) -> String {
    match output {
        Output::Text(s) => s.clone(),
        Output::Structured(v) => render_json_value(v, compact_json),
    }
}

/// Convert `Output::Structured` to newline-separated text lines suitable for piping
/// to external commands (grep, awk, sed, etc.).
///
/// Arrays of objects → TSV (header row + one data row per entry)
/// Arrays of scalars → one value per line
/// Objects → pretty-printed JSON
/// Scalars → their string representation
pub fn structured_to_text_lines(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Array(items) => {
            if items.is_empty() {
                return String::new();
            }
            // Detect if all items are objects → emit TSV
            let all_objects = items.iter().all(|i| i.is_object());
            if all_objects {
                // Collect union of all keys preserving first-seen order
                let mut columns: Vec<String> = Vec::new();
                for item in items {
                    if let serde_json::Value::Object(map) = item {
                        for key in map.keys() {
                            if !columns.contains(key) {
                                columns.push(key.clone());
                            }
                        }
                    }
                }
                let mut out = columns.join("\t");
                out.push('\n');
                for item in items {
                    if let serde_json::Value::Object(map) = item {
                        let row: Vec<String> = columns
                            .iter()
                            .map(|col| {
                                json_scalar_to_string(
                                    map.get(col).unwrap_or(&serde_json::Value::Null),
                                )
                            })
                            .collect();
                        out.push_str(&row.join("\t"));
                        out.push('\n');
                    }
                }
                out
            } else {
                // Array of scalars → one per line
                items
                    .iter()
                    .map(json_scalar_to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// Render a `serde_json::Value` for terminal display.
/// Arrays of objects are rendered as a pretty table; everything else as JSON.
fn render_json_value(v: &serde_json::Value, compact_json: bool) -> String {
    if compact_json {
        return serde_json::to_string(v).unwrap_or_default();
    }

    match v {
        serde_json::Value::Array(items)
            if !items.is_empty() && items.iter().all(|i| i.is_object()) =>
        {
            // Convert array-of-objects to a Table and render
            use crate::executor::value::render::TableRenderer;
            use crate::executor::value::{Table, Value as RushValue};
            use std::collections::HashMap;

            // Collect columns preserving first-seen order
            let mut columns: Vec<String> = Vec::new();
            for item in items {
                if let serde_json::Value::Object(map) = item {
                    for key in map.keys() {
                        if !columns.contains(key) {
                            columns.push(key.clone());
                        }
                    }
                }
            }

            let mut table = Table::new(columns.clone());
            for item in items {
                if let serde_json::Value::Object(map) = item {
                    let mut row: HashMap<String, RushValue> = HashMap::new();
                    for col in &columns {
                        let val = match map.get(col).unwrap_or(&serde_json::Value::Null) {
                            serde_json::Value::String(s) => RushValue::String(s.clone()),
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    RushValue::Int(i)
                                } else {
                                    RushValue::Float(n.as_f64().unwrap_or(0.0))
                                }
                            }
                            serde_json::Value::Bool(b) => RushValue::Bool(*b),
                            serde_json::Value::Null => RushValue::Null,
                            other => RushValue::String(other.to_string()),
                        };
                        row.insert(col.clone(), val);
                    }
                    table.push_row(row);
                }
            }

            TableRenderer::new().render(&table)
        }
        serde_json::Value::Array(items) => {
            // Flat array of scalars → one per line
            items
                .iter()
                .map(json_scalar_to_string)
                .collect::<Vec<_>>()
                .join("\n")
        }
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// Convert a scalar JSON value to a plain string (no quotes for strings).
fn json_scalar_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Remove the shortest suffix matching the pattern from the value.
/// Pattern supports * (match any) and ? (match single char).
fn remove_shortest_suffix(value: &str, pattern: &str) -> String {
    if pattern.is_empty() {
        return value.to_string();
    }
    // Try removing increasingly longer suffixes
    for i in (0..=value.len()).rev() {
        let suffix = &value[i..];
        if pattern_matches(pattern, suffix) {
            return value[..i].to_string();
        }
    }
    value.to_string()
}

/// Remove the longest suffix matching the pattern from the value.
fn remove_longest_suffix(value: &str, pattern: &str) -> String {
    if pattern.is_empty() {
        return value.to_string();
    }
    // Try removing increasingly shorter suffixes (longest match first)
    for i in 0..=value.len() {
        let suffix = &value[i..];
        if pattern_matches(pattern, suffix) {
            return value[..i].to_string();
        }
    }
    value.to_string()
}

/// Remove the shortest prefix matching the pattern from the value.
fn remove_shortest_prefix(value: &str, pattern: &str) -> String {
    if pattern.is_empty() {
        return value.to_string();
    }
    // Try removing increasingly longer prefixes
    for i in 0..=value.len() {
        let prefix = &value[..i];
        if pattern_matches(pattern, prefix) {
            return value[i..].to_string();
        }
    }
    value.to_string()
}

/// Remove the longest prefix matching the pattern from the value.
fn remove_longest_prefix(value: &str, pattern: &str) -> String {
    if pattern.is_empty() {
        return value.to_string();
    }
    // Try removing increasingly shorter prefixes (longest match first)
    for i in (0..=value.len()).rev() {
        let prefix = &value[..i];
        if pattern_matches(pattern, prefix) {
            return value[i..].to_string();
        }
    }
    value.to_string()
}

/// Match a shell pattern against a string.
/// Supports * (match any sequence) and ? (match single char).
fn pattern_matches(pattern: &str, text: &str) -> bool {
    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    pattern_matches_helper(&pat_chars, &text_chars)
}

fn pattern_matches_helper(pattern: &[char], text: &[char]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }

    match pattern[0] {
        '*' => {
            // * matches zero or more characters
            // Try matching zero chars, then one, then two, etc.
            for i in 0..=text.len() {
                if pattern_matches_helper(&pattern[1..], &text[i..]) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // ? matches exactly one character
            if text.is_empty() {
                false
            } else {
                pattern_matches_helper(&pattern[1..], &text[1..])
            }
        }
        c => {
            // Literal character must match
            if text.is_empty() || text[0] != c {
                false
            } else {
                pattern_matches_helper(&pattern[1..], &text[1..])
            }
        }
    }
}

/// Expand variables and command substitutions in a redirect target path.
///
/// Handles `$VAR`, `${VAR}`, and `$(...)` within strings like
/// `/tmp/log-$name.txt`. Takes an immutable runtime reference so it can be
/// called from both `&self` and free-function contexts without borrowing issues.
pub(crate) fn expand_redirect_target(input: &str, runtime: &Runtime) -> String {
    // Fast path: no expansion needed
    if !input.contains('$') && !input.contains('`') {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len() * 2);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '$' if i + 1 < chars.len() => {
                match chars[i + 1] {
                    '(' => {
                        // Command substitution $(...) — depth-track to find matching ')'
                        let start = i + 2;
                        let mut depth = 1i32;
                        let mut j = start;
                        while j < chars.len() {
                            match chars[j] {
                                '(' => depth += 1,
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        if depth == 0 && j < chars.len() {
                            let cmd_str: String = chars[start..j].iter().collect();
                            // Check for arithmetic $((expr))
                            if cmd_str.starts_with('(') && cmd_str.ends_with(')') {
                                let expr = &cmd_str[1..cmd_str.len() - 1];
                                if let Ok(val) = crate::arithmetic::evaluate(expr, runtime) {
                                    result.push_str(&val.to_string());
                                    i = j + 1;
                                    continue;
                                }
                            }
                            // Execute command substitution via a cloned sub-executor
                            if let Ok(tokens) = crate::lexer::Lexer::tokenize(&cmd_str) {
                                let mut parser = crate::parser::Parser::new(tokens);
                                if let Ok(statements) = parser.parse() {
                                    let mut sub_executor = Executor {
                                        runtime: runtime.clone(),
                                        builtins: Builtins::new(),
                                        corrector: Corrector::new(),
                                        suggestion_engine: SuggestionEngine::new(),
                                        signal_handler: None,
                                        terminal_control: TerminalControl::new(),
                                        show_progress: false,
                                        call_stack: CallStack::new(),
                                        profile_data: None,
                                        enable_profiling: false,
                                    };
                                    if let Ok(exec_result) = sub_executor.execute(statements) {
                                        let mut out = exec_result.stdout();
                                        let max = max_substitution_output();
                                        if out.len() > max {
                                            out.truncate(max);
                                            while !out.is_char_boundary(out.len()) {
                                                out.pop();
                                            }
                                            eprintln!("rush: warning: command substitution output truncated at {} bytes", max);
                                        }
                                        result.push_str(out.trim_end());
                                        i = j + 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        result.push('$');
                        i += 1;
                    }
                    '{' => {
                        // Braced variable ${VAR}
                        let start = i + 2;
                        let mut depth = 1i32;
                        let mut j = start;
                        while j < chars.len() {
                            match chars[j] {
                                '{' => depth += 1,
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        if depth == 0 && j < chars.len() {
                            let var_name: String = chars[start..j].iter().collect();
                            let value = runtime.get_variable(&var_name).unwrap_or_default();
                            result.push_str(&value);
                            i = j + 1;
                        } else {
                            result.push('$');
                            i += 1;
                        }
                    }
                    c if c.is_ascii_digit() || c.is_ascii_alphabetic() || c == '_' => {
                        // Simple variable $VAR
                        let mut j = i + 1;
                        while j < chars.len()
                            && (chars[j].is_ascii_alphanumeric() || chars[j] == '_')
                        {
                            j += 1;
                        }
                        let var_name: String = chars[i + 1..j].iter().collect();
                        let value = runtime.get_variable(&var_name).unwrap_or_default();
                        result.push_str(&value);
                        i = j;
                    }
                    '?' => {
                        result.push_str(&runtime.get_last_exit_code().to_string());
                        i += 2;
                    }
                    '$' => {
                        result.push_str(&std::process::id().to_string());
                        i += 2;
                    }
                    _ => {
                        result.push('$');
                        i += 1;
                    }
                }
            }
            c => {
                result.push(c);
                i += 1;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_reset_clears_runtime_state() {
        let mut executor = Executor::new_embedded();

        // Set some runtime state
        executor
            .runtime_mut()
            .set_variable("TEST_VAR".to_string(), "value".to_string());
        executor.runtime_mut().set_last_exit_code(42);
        executor
            .runtime_mut()
            .set_alias("ll".to_string(), "ls -la".to_string());

        // Verify state is set
        assert_eq!(
            executor.runtime_mut().get_variable("TEST_VAR"),
            Some("value".to_string())
        );
        assert_eq!(executor.runtime_mut().get_last_exit_code(), 42);

        // Reset
        executor.reset().unwrap();

        // Verify state is cleared
        assert_eq!(executor.runtime_mut().get_variable("TEST_VAR"), None);
        assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
        assert!(executor.runtime_mut().get_alias("ll").is_none());
    }

    #[test]
    fn test_executor_reuse_no_state_leakage() {
        let mut executor = Executor::new_embedded();

        // Simulate first command: set a variable via assignment
        executor
            .runtime_mut()
            .set_variable("LEAKED".to_string(), "secret".to_string());
        executor.runtime_mut().set_last_exit_code(1);

        // Reset between commands
        executor.reset().unwrap();

        // After reset, the variable should not exist
        assert_eq!(executor.runtime_mut().get_variable("LEAKED"), None);
        assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
    }

    #[test]
    fn test_executor_reset_preserves_functionality() {
        let mut executor = Executor::new_embedded();

        // Execute a command, then reset, then execute again
        executor
            .runtime_mut()
            .set_variable("X".to_string(), "1".to_string());
        executor.reset().unwrap();

        // After reset, executor should still be usable
        executor
            .runtime_mut()
            .set_variable("Y".to_string(), "2".to_string());
        assert_eq!(
            executor.runtime_mut().get_variable("Y"),
            Some("2".to_string())
        );
        assert_eq!(executor.runtime_mut().get_variable("X"), None);
    }

    #[test]
    fn test_executor_reset_multiple_cycles() {
        let mut executor = Executor::new_embedded();

        // Simulate multiple request/reset cycles
        for i in 0..5 {
            let key = format!("VAR_{}", i);
            executor
                .runtime_mut()
                .set_variable(key.clone(), i.to_string());
            assert_eq!(
                executor.runtime_mut().get_variable(&key),
                Some(i.to_string())
            );

            executor.reset().unwrap();

            // After reset, variable from this cycle should be gone
            assert_eq!(executor.runtime_mut().get_variable(&key), None);
            // IFS and $? should be re-initialized
            assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
            assert_eq!(executor.runtime_mut().get_ifs(), " \t\n");
        }
    }

    /// Helper: parse and execute a single line, returning the result
    fn run_line(executor: &mut Executor, line: &str) -> ExecutionResult {
        use crate::lexer::Lexer;
        use crate::parser::Parser;

        let tokens = Lexer::tokenize(line).expect("tokenize failed");
        let mut parser = Parser::new(tokens);
        let statements = parser.parse().expect("parse failed");
        executor.execute(statements).expect("execute failed")
    }

    #[test]
    fn test_if_true_then_echo() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "if true; then echo yes; fi");
        assert_eq!(result.stdout().trim(), "yes");
    }

    #[test]
    fn test_if_false_else() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "if false; then echo yes; else echo no; fi");
        assert_eq!(result.stdout().trim(), "no");
    }

    #[test]
    fn test_if_elif() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "if false; then echo 1; elif true; then echo 2; fi",
        );
        assert_eq!(result.stdout().trim(), "2");
    }

    #[test]
    fn test_nested_if() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "if true; then if true; then echo nested; fi; fi",
        );
        assert_eq!(result.stdout().trim(), "nested");
    }

    #[test]
    fn test_for_loop_basic() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "for x in a b c; do echo $x; done");
        assert_eq!(result.stdout(), "a\nb\nc\n");
    }

    #[test]
    fn test_for_loop_nested() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "for i in 1 2; do for j in a b; do echo $i$j; done; done",
        );
        // echo $i$j currently produces "1 a" due to word splitting
        // TODO: Fix variable concatenation to produce "1a" (POSIX behavior)
        assert_eq!(result.stdout(), "1 a\n1 b\n2 a\n2 b\n");
    }

    #[test]
    fn test_for_loop_break() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "for x in a b c; do echo $x; break; done");
        assert_eq!(result.stdout(), "a\n");
    }

    #[test]
    fn test_for_loop_continue() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "for x in a b c; do echo $x; continue; echo NOPE; done",
        );
        assert_eq!(result.stdout(), "a\nb\nc\n");
    }

    #[test]
    fn test_for_loop_variable_expansion() {
        let mut executor = Executor::new_embedded();
        // Set a variable, then iterate with it
        run_line(&mut executor, "ITEMS=\"hello world\"");
        let result = run_line(&mut executor, "for x in $ITEMS; do echo $x; done");
        assert_eq!(result.stdout(), "hello\nworld\n");
    }

    #[test]
    fn test_for_loop_single_word() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "for x in only; do echo $x; done");
        assert_eq!(result.stdout(), "only\n");
    }

    // --- Function definition tests ---

    #[test]
    fn test_function_def_posix_basic() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "foo() { echo hello; }");
        let result = run_line(&mut executor, "foo");
        assert_eq!(result.stdout().trim(), "hello");
    }

    #[test]
    fn test_function_def_posix_inline() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "foo() { echo hello; }; foo");
        assert_eq!(result.stdout().trim(), "hello");
    }

    #[test]
    fn test_function_positional_params() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "greet() { echo hi $1; }");
        let result = run_line(&mut executor, "greet world");
        assert_eq!(result.stdout().trim(), "hi world");
    }

    #[test]
    fn test_function_return_exit_code() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "ret5() { return 5; }; ret5; echo $?");
        assert_eq!(result.stdout().trim(), "5");
    }

    #[test]
    fn test_function_local_variables() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "f() { local x=inner; echo $x; }; x=outer; f; echo $x",
        );
        assert_eq!(result.stdout(), "inner\nouter\n");
    }

    #[test]
    fn test_function_bash_keyword() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "function bar { echo bar-works; }; bar");
        assert_eq!(result.stdout().trim(), "bar-works");
    }

    #[test]
    fn test_function_bash_keyword_with_parens() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "function baz() { echo baz-works; }; baz");
        assert_eq!(result.stdout().trim(), "baz-works");
    }

    #[test]
    fn test_function_multiple_args() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "add() { echo $1 $2 $3; }");
        let result = run_line(&mut executor, "add a b c");
        assert_eq!(result.stdout().trim(), "a b c");
    }

    #[test]
    fn test_function_calls_function() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "inner() { echo inner; }");
        run_line(&mut executor, "outer() { inner; }");
        let result = run_line(&mut executor, "outer");
        assert_eq!(result.stdout().trim(), "inner");
    }

    #[test]
    fn test_function_multiple_body_statements() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "f() { echo one; echo two; }; f");
        assert_eq!(result.stdout(), "one\ntwo\n");
    }

    #[test]
    fn test_case_basic_match() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "x=foo; case $x in foo) echo matched;; esac");
        assert_eq!(result.stdout().trim(), "matched");
    }

    #[test]
    fn test_case_wildcard_default() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "x=c; case $x in a|b) echo ab;; *) echo other;; esac",
        );
        assert_eq!(result.stdout().trim(), "other");
    }

    #[test]
    fn test_case_multiple_patterns() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "x=b; case $x in a|b) echo ab;; *) echo other;; esac",
        );
        assert_eq!(result.stdout().trim(), "ab");
    }

    #[test]
    fn test_case_no_match() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "x=z; case $x in a) echo a;; b) echo b;; esac",
        );
        assert_eq!(result.stdout().trim(), "");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_case_nested() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "x=foo; case $x in foo) y=bar; case $y in bar) echo nested;; esac;; esac",
        );
        assert_eq!(result.stdout().trim(), "nested");
    }

    // --- While loop tests ---

    #[test]
    fn test_while_true_break() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "while true; do echo once; break; done");
        assert_eq!(result.stdout(), "once\n");
    }

    #[test]
    fn test_while_counter() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "count=0");
        let result = run_line(
            &mut executor,
            "while test $count -lt 3; do echo $count; count=$((count+1)); done",
        );
        assert_eq!(result.stdout(), "0\n1\n2\n");
    }

    #[test]
    fn test_while_loop_continue() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "count=0; while test $count -lt 3; do count=$((count+1)); if test $count -eq 2; then continue; fi; echo $count; done",
        );
        assert_eq!(result.stdout(), "1\n3\n");
    }

    #[test]
    fn test_while_nested() {
        let mut executor = Executor::new_embedded();
        let result = run_line(
            &mut executor,
            "i=0; while test $i -lt 2; do j=0; while test $j -lt 2; do echo $i$j; j=$((j+1)); done; i=$((i+1)); done",
        );
        // TODO: Fix variable concatenation to produce "00" (POSIX behavior)
        assert_eq!(result.stdout(), "0 0\n0 1\n1 0\n1 1\n");
    }

    // --- Until loop tests ---

    #[test]
    fn test_until_basic() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "i=0");
        let result = run_line(
            &mut executor,
            "until test $i -ge 3; do echo $i; i=$((i+1)); done",
        );
        assert_eq!(result.stdout(), "0\n1\n2\n");
    }

    #[test]
    fn test_until_with_break() {
        let mut executor = Executor::new_embedded();
        let result = run_line(&mut executor, "until false; do echo once; break; done");
        assert_eq!(result.stdout(), "once\n");
    }

    #[test]
    fn test_until_countdown() {
        let mut executor = Executor::new_embedded();
        run_line(&mut executor, "i=3");
        let result = run_line(
            &mut executor,
            "until test $i -le 0; do echo $i; i=$((i-1)); done",
        );
        assert_eq!(result.stdout(), "3\n2\n1\n");
    }

    // --- Configuration file tests ---

    #[test]
    fn test_source_file_basic() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a simple config file with environment variable setting
        fs::write(&config_file, "TEST_VAR=hello\necho $TEST_VAR\n").unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify the variable was set
        assert_eq!(
            executor.runtime_mut().get_variable("TEST_VAR"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_source_file_with_alias() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with alias definition
        fs::write(&config_file, "alias ll='ls -la'\n").unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify the alias was set
        assert!(executor.runtime_mut().get_alias("ll").is_some());
    }

    #[test]
    fn test_source_file_with_function() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with function definition
        fs::write(&config_file, "my_func() { echo 'function called'; }\n").unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify the function was defined
        assert!(executor.runtime_mut().get_function("my_func").is_some());
    }

    #[test]
    fn test_source_file_nonexistent() {
        use std::path::PathBuf;

        let config_file = PathBuf::from("/nonexistent/path/config");
        let mut executor = Executor::new_embedded();

        // Should not error on nonexistent file (silently ignore)
        let result = executor.source_file(&config_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_file_with_comments_and_blank_lines() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with comments and blank lines
        let content = r#"
# This is a comment
TEST_VAR=value1

# Another comment
ANOTHER_VAR=value2
"#;
        fs::write(&config_file, content).unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify variables were set
        assert_eq!(
            executor.runtime_mut().get_variable("TEST_VAR"),
            Some("value1".to_string())
        );
        assert_eq!(
            executor.runtime_mut().get_variable("ANOTHER_VAR"),
            Some("value2".to_string())
        );
    }

    #[test]
    fn test_source_file_with_multiple_variables() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with multiple variables
        let content = r#"
VAR1=one
VAR2=two
VAR3=three
"#;
        fs::write(&config_file, content).unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify all variables were set
        assert_eq!(
            executor.runtime_mut().get_variable("VAR1"),
            Some("one".to_string())
        );
        assert_eq!(
            executor.runtime_mut().get_variable("VAR2"),
            Some("two".to_string())
        );
        assert_eq!(
            executor.runtime_mut().get_variable("VAR3"),
            Some("three".to_string())
        );
    }

    #[test]
    fn test_source_file_error_handling() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with invalid syntax
        fs::write(&config_file, "invalid $$$ syntax\n").unwrap();

        let mut executor = Executor::new_embedded();

        // Should not panic, errors should be handled gracefully
        let result = executor.source_file(&config_file);
        // The error may be reported but execution should continue
        let _ = result;
    }

    #[test]
    fn test_source_file_with_shell_options() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with set command for shell options
        fs::write(&config_file, "set -e\nTEST_OPT=enabled\n").unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify variable was set even with set command
        assert_eq!(
            executor.runtime_mut().get_variable("TEST_OPT"),
            Some("enabled".to_string())
        );
    }

    #[test]
    fn test_source_file_execution_continues_on_error() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config");

        // Create a config file with one invalid line and one valid line
        let content = r#"
invalid $$$ syntax
VAR_AFTER_ERROR=should_be_set
"#;
        fs::write(&config_file, content).unwrap();

        let mut executor = Executor::new_embedded();
        executor.source_file(&config_file).unwrap();

        // Verify that the variable after the error was still set
        // (execution should continue despite error)
        assert_eq!(
            executor.runtime_mut().get_variable("VAR_AFTER_ERROR"),
            Some("should_be_set".to_string())
        );
    }
}

#[cfg(test)]
mod substitution_cap_tests {
    use super::*;

    #[test]
    fn test_command_substitution_output_cap() {
        // Verify the constant exists and has a reasonable default
        assert!(DEFAULT_MAX_SUBSTITUTION_OUTPUT >= 1024 * 1024); // at least 1MB
        assert!(DEFAULT_MAX_SUBSTITUTION_OUTPUT <= 100 * 1024 * 1024); // at most 100MB

        // Verify the function respects the env var
        std::env::set_var("RUSH_MAX_SUBST_OUTPUT", "1KB");
        assert_eq!(max_substitution_output(), 1024);
        std::env::remove_var("RUSH_MAX_SUBST_OUTPUT");

        // After removing, should return the default
        assert_eq!(max_substitution_output(), DEFAULT_MAX_SUBSTITUTION_OUTPUT);
    }
}
