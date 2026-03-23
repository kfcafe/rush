//! Variable expansion and resolution for the Rush shell executor.
//!
//! This module handles all forms of shell variable expansion including:
//! - Simple variable expansion ($VAR, ${VAR})
//! - Braced variable operations (${VAR:-default}, ${VAR%%pattern}, etc.)
//! - Command substitution ($(...) and `...`)
//! - Heredoc body expansion
//! - Tilde expansion (~, ~/path, ~user)
//! - Argument resolution and glob expansion

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Counter for generating unique process substitution FIFO paths
static PROC_SUB_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Executor {
    /// Expand a string value that may contain variable references ($VAR, ${VAR}, etc.)
    pub(crate) fn expand_string_value(&self, value: &str) -> Result<String> {
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

    pub(crate) fn expand_variables_in_literal(&mut self, input: &str) -> Result<String> {
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
                            for ch in chars.by_ref() {
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
                        '#' | '@' | '*' | '?' | '!' | '$' | '-' | '_' => {
                            let special = chars.next().unwrap();
                            let name = String::from(special);
                            if let Some(val) = self.resolve_special_variable(&name) {
                                result.push_str(&val);
                            }
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

    /// Expand variables and command substitutions in a heredoc body.
    pub(crate) fn expand_heredoc_body(&mut self, body: &str) -> Result<String> {
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
                            let value = self.runtime
                                .get_variable(&var_name)
                                .unwrap_or_default();
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
                '\\' if i + 1 < chars.len() => {
                    match chars[i + 1] {
                        '$' => { result.push('$'); i += 2; }
                        '`' => { result.push('`'); i += 2; }
                        '\\' => { result.push('\\'); i += 2; }
                        'n' => { result.push('\n'); i += 2; }
                        't' => { result.push('\t'); i += 2; }
                        _ => { result.push('\\'); result.push(chars[i + 1]); i += 2; }
                    }
                }
                c => {
                    result.push(c);
                    i += 1;
                }
            }
        }

        Ok(result)
    }

    pub(crate) fn find_matching_paren_in_str(&self, chars: &[char], start: usize) -> Option<usize> {
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

    pub(crate) fn expand_braced_variable(&self, expr: &str) -> String {
        // String length: ${#var}
        if let Some(var_name) = expr.strip_prefix('#') {
            return self.runtime
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
            return self.runtime
                .get_variable(var_name)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default_val.to_string());
        }
        if let Some(pos) = expr.find(":=") {
            let var_name = &expr[..pos];
            let default_val = &expr[pos + 2..];
            let val = self.runtime.get_variable(var_name);
            if val.as_deref().is_none_or(str::is_empty) {
                return default_val.to_string();
            }
            return val.unwrap_or_default();
        }
        // Use alternate if set and non-empty
        if let Some(pos) = expr.find(":+") {
            let var_name = &expr[..pos];
            let alternate = &expr[pos + 2..];
            return self.runtime
                .get_variable(var_name)
                .filter(|v| !v.is_empty())
                .map(|_| alternate.to_string())
                .unwrap_or_default();
        }
        // Error if unset or empty
        if let Some(pos) = expr.find(":?") {
            let var_name = &expr[..pos];
            let message = &expr[pos + 2..];
            if self.runtime.get_variable(var_name).is_none_or(|v| v.is_empty()) {
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

    pub(crate) fn execute_command_substitution_str(&mut self, cmd_str: &str) -> Result<String> {
        use crate::lexer::Lexer;
        use crate::parser::Parser;

        let tokens = Lexer::tokenize(cmd_str)
            .map_err(|e| anyhow!("Heredoc command substitution lex error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse()?;
        let result = self.execute(stmts)?;
        Ok(result.stdout())
    }

    /// Resolve a special shell variable by name. Returns `Some(value)` for
    /// recognized names ($?, $$, $!, $#, $@, $*, $-, $_, $0) or `None`.
    pub(crate) fn resolve_special_variable(&self, name: &str) -> Option<String> {
        match name {
            "?" => Some(self.runtime.get_last_exit_code().to_string()),
            "$" => Some(std::process::id().to_string()),
            "!" => Some(
                self.runtime
                    .get_last_bg_pid()
                    .map(|pid| pid.to_string())
                    .unwrap_or_default(),
            ),
            "#" => Some(self.runtime.param_count().to_string()),
            "@" => Some(self.runtime.get_positional_params().join(" ")),
            "*" => Some(self.runtime.get_positional_params().join(" ")),
            "-" => Some(self.runtime.get_option_flags()),
            "_" => Some(self.runtime.get_last_arg().to_string()),
            "0" => Some(
                self.runtime
                    .get_variable("0")
                    .unwrap_or_else(|| "rush".to_string()),
            ),
            _ => None,
        }
    }

    pub(crate) fn resolve_argument(&mut self, arg: &Argument) -> Result<String> {
        match arg {
            Argument::Literal(s) => {
                // Expand variables and command substitutions in literal strings
                self.expand_variables_in_literal(s)
            }
            Argument::Variable(var) => {
                // Strip single $ from variable name (use strip_prefix to remove only one $)
                let var_name = var.strip_prefix('$').unwrap_or(var);

                // Handle special variables first
                if let Some(val) = self.resolve_special_variable(var_name) {
                    return Ok(val);
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
                if let Some(val) = self.resolve_special_variable(&expansion.name) {
                    return Ok(val);
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
                Ok(self.execute_command_substitution(cmd)
                    .unwrap_or_else(|_| String::new()))
            }
            Argument::Flag(f) => Ok(f.clone()),
            Argument::Path(p) => Ok(expand_tilde(p)),
            Argument::Glob(g) => Ok(g.clone()),
            Argument::ProcessSubIn(cmd) => {
                let fifo_path = self.setup_process_sub(cmd, true)?;
                self.process_sub_fifos.push(fifo_path.clone());
                Ok(fifo_path)
            }
            Argument::ProcessSubOut(cmd) => {
                let fifo_path = self.setup_process_sub(cmd, false)?;
                self.process_sub_fifos.push(fifo_path.clone());
                Ok(fifo_path)
            }
        }
    }

    /// Set up a process substitution by creating a FIFO and spawning a child process.
    ///
    /// For `<(cmd)` (is_input=true): the child writes `cmd` stdout to the FIFO.
    /// For `>(cmd)` (is_input=false): the child reads from the FIFO as `cmd` stdin.
    ///
    /// Returns the FIFO path which the parent command uses as a file argument.
    fn setup_process_sub(&mut self, token: &str, is_input: bool) -> Result<String> {
        // Extract command from <(command) or >(command)
        let command = &token[2..token.len() - 1];

        // Create a unique FIFO in /tmp
        let id = PROC_SUB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let fifo_path = format!("/tmp/rush_procsub_{}_{}", std::process::id(), id);
        let c_path = std::ffi::CString::new(fifo_path.as_str())
            .map_err(|_| anyhow!("Invalid FIFO path"))?;
        let ret = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        if ret != 0 {
            return Err(anyhow!(
                "Failed to create FIFO '{}': {}",
                fifo_path,
                std::io::Error::last_os_error()
            ));
        }

        if is_input {
            // <(cmd): child runs cmd, stdout goes to FIFO
            // Use sh to handle the redirect so the child process blocks on FIFO open
            // (not the parent). spawn() forks immediately — the child blocks, not rush.
            let shell_cmd = format!("{} > '{}'", command, fifo_path);
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&shell_cmd)
                .current_dir(self.runtime.get_cwd())
                .envs(self.runtime.get_env())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| anyhow!("Failed to spawn process substitution: {}", e))?;
        } else {
            // >(cmd): child runs cmd, stdin reads from FIFO
            let shell_cmd = format!("{} < '{}'", command, fifo_path);
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&shell_cmd)
                .current_dir(self.runtime.get_cwd())
                .envs(self.runtime.get_env())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| anyhow!("Failed to spawn process substitution: {}", e))?;
        }

        Ok(fifo_path)
    }

    pub(crate) fn parse_braced_var_expansion(&self, braced_var: &str) -> Result<VarExpansion> {
        // Remove ${ and } from the string
        let inner = braced_var.trim_start_matches("${").trim_end_matches('}');

        // String length: ${#var}
        if inner.starts_with('#') && !inner.contains(':') && !inner[1..].contains('#') && !inner[1..].contains('%') {
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
    pub(crate) fn expand_and_resolve_arguments(&mut self, args: &[Argument]) -> Result<Vec<String>> {
        let mut expanded_args = Vec::new();

        for arg in args {
            // Determine if this argument should be subject to IFS splitting
            // Only unquoted variables and command substitutions should be split
            let should_split_ifs = matches!(
                arg,
                Argument::Variable(_) | Argument::BracedVariable(_) | Argument::CommandSubstitution(_)
            );

            // Determine if this argument should have glob expansion
            // Glob patterns from the lexer (Argument::Glob) and unquoted variables should expand
            // Quoted strings (Argument::Literal from quoted tokens) should NOT expand
            // Path is included because paths like /tmp/*.txt are tokenized as Path by the lexer
            let should_expand = matches!(
                arg,
                Argument::Glob(_) | Argument::Path(_) | Argument::Variable(_) | Argument::BracedVariable(_) | Argument::CommandSubstitution(_)
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
    pub(crate) fn execute_command_substitution(&self, cmd_str: &str) -> Result<String> {
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
        let statements = parser.parse()
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
            process_sub_fifos: Vec::new(),
            hook_manager: Default::default(),
        };

        // Execute the command and capture output
        let result = sub_executor.execute(statements)?;

        // Return stdout with trailing newlines trimmed (bash behavior)
        Ok(result.stdout().trim_end().to_string())
    }

    /// Expand all command substitution sequences ($(...) and `...`) within a string.
    /// Handles nested substitutions by delegating to execute_command_substitution.
    pub(crate) fn expand_command_substitutions_in_string(&self, input: &str) -> Result<String> {
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
                            while j < len && bytes[j] != b'\'' { j += 1; }
                        }
                        b'"' => {
                            j += 1;
                            while j < len {
                                if bytes[j] == b'"' { break; }
                                if bytes[j] == b'\\' { j += 1; }
                                j += 1;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if depth == 0 {
                    let substitution = &input[start..j];
                    let output = self.execute_command_substitution(substitution)
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
                    if bytes[j] == b'`' { j += 1; break; }
                    else if bytes[j] == b'\\' && j + 1 < len { j += 2; }
                    else { j += 1; }
                }

                if j <= len && j > start + 1 && bytes[j - 1] == b'`' {
                    let substitution = &input[start..j];
                    let output = self.execute_command_substitution(substitution)
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

    pub(crate) fn literal_to_string(&self, lit: Literal) -> String {
        match lit {
            Literal::String(s) => s,
            Literal::Integer(n) => n.to_string(),
            Literal::Float(f) => f.to_string(),
            Literal::Boolean(b) => b.to_string(),
        }
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
        // SAFETY: getpwnam is a standard POSIX function; c_username is a valid CString
        let pw = unsafe { libc::getpwnam(c_username.as_ptr()) }; // SAFETY
        if !pw.is_null() {
            // SAFETY: pw is non-null (checked above); pw_dir is a valid C string per POSIX
            let home_dir = unsafe { std::ffi::CStr::from_ptr((*pw).pw_dir) }; // SAFETY
            if let Ok(home) = home_dir.to_str() {
                return format!("{}{}", home, suffix);
            }
        }
    }

    // If user lookup fails, return unchanged
    path.to_string()
}

pub(crate) fn resolve_argument_static(arg: &Argument, runtime: &Runtime) -> String {
    match arg {
        Argument::Literal(s) => {
            if s.contains("$(") || s.contains('`') {
                expand_command_substitutions_in_string_static(s, runtime)
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
                        builtins: Builtins::new(),
                        corrector: Corrector::new(),
                        suggestion_engine: SuggestionEngine::new(),
                        signal_handler: None,
                        show_progress: false,
                        terminal_control: TerminalControl::new(),
                        call_stack: CallStack::new(),
                        profile_data: None,
                        enable_profiling: false,
                        process_sub_fifos: Vec::new(),
            hook_manager: Default::default(),
                    };
                    if let Ok(exec_result) = sub_executor.execute(statements) {
                        return exec_result.stdout().trim_end().to_string();
                    }
                }
            }

            // If execution failed, return empty string
            String::new()
        }
        Argument::Flag(f) => f.clone(),
        Argument::Path(p) => expand_tilde(p),
        Argument::Glob(g) => g.clone(),
        // Process substitution should not appear in static resolution context
        Argument::ProcessSubIn(_) | Argument::ProcessSubOut(_) => String::new(),
    }
}

// Helper function for parallel execution with glob expansion
pub(crate) fn expand_and_resolve_arguments_static(args: &[Argument], runtime: &Runtime) -> Result<Vec<String>> {
    let mut expanded_args = Vec::new();

    for arg in args {
        // Only expand globs for Argument::Glob, Path, and variable types (not quoted Literals)
        // Path is included because paths like /tmp/*.txt are tokenized as Path by the lexer
        let should_expand = matches!(
            arg,
            Argument::Glob(_) | Argument::Path(_) | Argument::Variable(_) | Argument::BracedVariable(_) | Argument::CommandSubstitution(_)
        );

        let resolved = resolve_argument_static(arg, runtime);

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
pub(crate) fn expand_command_substitutions_in_string_static(input: &str, runtime: &Runtime) -> String {
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
                    b'\'' => { j += 1; while j < len && bytes[j] != b'\'' { j += 1; } }
                    b'"' => { j += 1; while j < len { if bytes[j] == b'"' { break; } else if bytes[j] == b'\\' { j += 1; } j += 1; } }
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
                            builtins: Builtins::new(),
                            corrector: Corrector::new(),
                            suggestion_engine: SuggestionEngine::new(),
                            signal_handler: None,
                            show_progress: false,
                            terminal_control: TerminalControl::new(),
                            call_stack: CallStack::new(),
                            profile_data: None,
                            enable_profiling: false,
                            process_sub_fifos: Vec::new(),
            hook_manager: Default::default(),
                        };
                        if let Ok(exec_result) = sub_executor.execute(statements) {
                            result.push_str(exec_result.stdout().trim_end());
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
                if bytes[j] == b'`' { j += 1; break; }
                else if bytes[j] == b'\\' && j + 1 < len { j += 2; }
                else { j += 1; }
            }
            if j <= len && j > start + 1 && bytes[j - 1] == b'`' {
                let command = &input[start + 1..j - 1];
                if let Ok(tokens) = crate::lexer::Lexer::tokenize(command) {
                    let mut parser = crate::parser::Parser::new(tokens);
                    if let Ok(statements) = parser.parse() {
                        let mut sub_executor = Executor {
                            runtime: runtime.clone(),
                            builtins: Builtins::new(),
                            corrector: Corrector::new(),
                            suggestion_engine: SuggestionEngine::new(),
                            signal_handler: None,
                            show_progress: false,
                            terminal_control: TerminalControl::new(),
                            call_stack: CallStack::new(),
                            profile_data: None,
                            enable_profiling: false,
                            process_sub_fifos: Vec::new(),
            hook_manager: Default::default(),
                        };
                        if let Ok(exec_result) = sub_executor.execute(statements) {
                            result.push_str(exec_result.stdout().trim_end());
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
