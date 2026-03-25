use crate::builtins::trap::{TrapHandlers, TrapSignal};
use crate::history::History;
use crate::jobs::JobManager;
use crate::parser::ast::FunctionDef;
use crate::parser::ast::{VarExpansion, VarExpansionOp};
use crate::undo::UndoManager;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

/// Shell options that control execution behavior
#[derive(Clone, Default)]
pub struct ShellOptions {
    pub errexit: bool,   // Exit on error (set -e)
    pub pipefail: bool,  // Pipeline fails if any command fails (set -o pipefail)
    pub nounset: bool,   // Error on undefined variable (set -u)
    pub xtrace: bool,    // Print commands before executing (set -x)
    pub noclobber: bool, // Prevent overwriting files (set -C)
    pub verbose: bool,   // Print input lines as they are read (set -v)
}

/// Runtime environment for the shell
#[derive(Clone)]
pub struct Runtime {
    variables: HashMap<String, String>,
    readonly_vars: HashSet<String>, // Track readonly variables
    functions: Arc<HashMap<String, FunctionDef>>,
    aliases: HashMap<String, String>,
    cwd: PathBuf,
    scopes: Vec<HashMap<String, String>>,
    call_stack: Vec<String>,
    max_call_depth: usize,
    history: Option<Arc<History>>,     // Lazy initialization
    undo_manager: Option<UndoManager>, // Lazy initialization
    job_manager: JobManager,
    pub options: ShellOptions,
    positional_params: Vec<String>, // Track $1, $2, etc. for shift builtin
    positional_stack: Vec<Vec<String>>, // Stack for function scopes
    function_depth: usize,          // Track function call depth for return builtin
    loop_depth: usize,              // Track loop nesting depth for break/continue builtins
    trap_handlers: TrapHandlers,    // Signal trap handlers
    // Permanent file descriptor redirections (set by exec builtin)
    permanent_stdout: Option<i32>,
    permanent_stderr: Option<i32>,
    permanent_stdin: Option<i32>,
    // Special variables tracking
    last_bg_pid: Option<u32>, // Track PID of last background job ($!)
    last_arg: String,         // Track last argument of previous command ($_)
    // Directory stack for pushd/popd/dirs builtins
    dir_stack: Vec<PathBuf>,
    // Piped stdin data for compound commands in pipelines
    piped_stdin: Option<Vec<u8>>,
    // Agent mode: emit JSON from built-ins and strip ANSI from all output
    agent_mode: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let mut runtime = Self {
            variables: HashMap::new(),
            readonly_vars: HashSet::new(),
            functions: Arc::new(HashMap::new()),
            aliases: HashMap::new(),
            cwd,
            scopes: Vec::new(),
            call_stack: Vec::new(),
            max_call_depth: 100,
            history: None,      // Lazy initialization
            undo_manager: None, // Lazy initialization
            job_manager: JobManager::new(),
            options: ShellOptions::default(),
            positional_params: Vec::new(),
            positional_stack: Vec::new(),
            function_depth: 0,
            loop_depth: 0,
            trap_handlers: TrapHandlers::new(),
            permanent_stdout: None,
            permanent_stderr: None,
            permanent_stdin: None,
            last_bg_pid: None,
            last_arg: String::new(),
            dir_stack: Vec::new(),
            piped_stdin: None,
            agent_mode: std::env::var("RUSH_AGENT_MODE").as_deref() == Ok("1"),
        };

        // Initialize $? to 0
        runtime.set_last_exit_code(0);

        // Initialize IFS to default value (space, tab, newline)
        runtime.set_variable("IFS".to_string(), " \t\n".to_string());

        runtime
    }

    /// Get the IFS (Internal Field Separator) variable value
    /// Defaults to space, tab, and newline if not set
    pub fn get_ifs(&self) -> String {
        self.get_variable("IFS")
            .unwrap_or_else(|| " \t\n".to_string())
    }

    /// Split a string by IFS characters
    /// Returns a vector of fields after splitting
    ///
    /// If IFS is empty, no splitting occurs.
    /// Leading/trailing IFS whitespace characters are removed.
    /// Consecutive IFS whitespace characters are treated as a single separator.
    pub fn split_by_ifs<'a>(&self, s: &'a str) -> Vec<&'a str> {
        let ifs = self.get_ifs();

        if ifs.is_empty() {
            // Empty IFS means no splitting
            return vec![s];
        }

        // Split by any character in IFS
        let mut fields = Vec::new();
        let mut current_field_start = 0;
        let mut in_field = false;

        for (i, ch) in s.char_indices() {
            if ifs.contains(ch) {
                if in_field {
                    fields.push(&s[current_field_start..i]);
                    in_field = false;
                }
            } else {
                if !in_field {
                    current_field_start = i;
                    in_field = true;
                }
            }
        }

        // Add the last field if we ended in one
        if in_field {
            fields.push(&s[current_field_start..]);
        }

        fields
    }

    pub fn set_variable(&mut self, name: String, value: String) {
        // If we're in a function scope, set the variable in the current scope
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        } else {
            // Otherwise set in global scope
            self.variables.insert(name, value);
        }
    }

    /// Set a variable with readonly check
    /// Returns an error if the variable is readonly
    pub fn set_variable_checked(&mut self, name: String, value: String) -> Result<()> {
        if self.is_readonly(&name) {
            return Err(anyhow!("{}: readonly variable", name));
        }
        self.set_variable(name, value);
        Ok(())
    }

    pub fn get_variable(&self, name: &str) -> Option<String> {
        // Check scopes from most recent to oldest
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value.clone());
            }
        }
        // Fall back to global variables
        self.variables.get(name).cloned()
    }

    /// Remove a variable from the current scope or global scope
    /// Returns true if the variable was found and removed
    pub fn remove_variable(&mut self, name: &str) -> bool {
        // If we're in a function scope, try to remove from the current scope first
        if let Some(scope) = self.scopes.last_mut() {
            if scope.remove(name).is_some() {
                return true;
            }
        }
        // Otherwise remove from global scope
        self.variables.remove(name).is_some()
    }

    /// Get variable with nounset option check
    pub fn get_variable_checked(&self, name: &str) -> Result<String> {
        match self.get_variable(name) {
            Some(value) => Ok(value),
            None => {
                if self.options.nounset {
                    Err(anyhow!("{}: unbound variable", name))
                } else {
                    Ok(String::new())
                }
            }
        }
    }

    /// Set the last exit code (stored in $? variable)
    pub fn set_last_exit_code(&mut self, code: i32) {
        self.variables.insert("?".to_string(), code.to_string());
    }

    /// Get the last exit code (from $? variable)
    pub fn get_last_exit_code(&self) -> i32 {
        self.variables
            .get("?")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// Set the PIPESTATUS array (exit codes of each pipeline command)
    pub fn set_pipestatus(&mut self, codes: Vec<i32>) {
        // Store as space-separated string for POSIX compatibility
        let status_str = codes
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        self.variables.insert("PIPESTATUS".to_string(), status_str);
    }

    /// Get the PIPESTATUS array as a vector of exit codes
    pub fn get_pipestatus(&self) -> Vec<i32> {
        self.variables
            .get("PIPESTATUS")
            .map(|s| {
                s.split_whitespace()
                    .filter_map(|c| c.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn define_function(&mut self, func: FunctionDef) {
        Arc::make_mut(&mut self.functions).insert(func.name.clone(), func);
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// Get all user-defined function names
    pub fn get_function_names(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    /// Remove a function definition
    /// Returns true if the function was found and removed
    pub fn remove_function(&mut self, name: &str) -> bool {
        Arc::make_mut(&mut self.functions).remove(name).is_some()
    }

    // Alias management
    pub fn set_alias(&mut self, name: String, value: String) {
        self.aliases.insert(name, value);
    }

    pub fn get_alias(&self, name: &str) -> Option<&String> {
        self.aliases.get(name)
    }

    pub fn remove_alias(&mut self, name: &str) -> bool {
        self.aliases.remove(name).is_some()
    }

    pub fn get_all_aliases(&self) -> &HashMap<String, String> {
        &self.aliases
    }

    pub fn set_cwd(&mut self, path: PathBuf) {
        self.cwd = path;
    }

    pub fn get_cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Refresh the stored cwd from the OS.
    /// Handles cases where the current directory was renamed or moved externally
    /// (e.g. while a child process was running). The OS tracks cwd by inode so
    /// the process stays in the directory, but the stored path becomes stale.
    pub fn refresh_cwd(&mut self) {
        if let Ok(actual) = env::current_dir() {
            if self.cwd != actual {
                self.cwd = actual.clone();
                self.variables
                    .insert("PWD".to_string(), actual.to_string_lossy().to_string());
            }
        }
    }

    // Shell options management
    pub fn set_option(&mut self, option: &str, value: bool) -> Result<()> {
        match option {
            "e" | "errexit" => self.options.errexit = value,
            "u" | "nounset" => self.options.nounset = value,
            "x" | "xtrace" => self.options.xtrace = value,
            "pipefail" => self.options.pipefail = value,
            _ => return Err(anyhow!("Unknown option: {}", option)),
        }
        Ok(())
    }

    pub fn get_option(&self, option: &str) -> Result<bool> {
        match option {
            "e" | "errexit" => Ok(self.options.errexit),
            "u" | "nounset" => Ok(self.options.nounset),
            "x" | "xtrace" => Ok(self.options.xtrace),
            "pipefail" => Ok(self.options.pipefail),
            _ => Err(anyhow!("Unknown option: {}", option)),
        }
    }

    pub fn get_env(&self) -> HashMap<String, String> {
        env::vars().collect()
    }

    pub fn set_env(&self, key: &str, value: &str) {
        env::set_var(key, value);
    }

    // Scope management for function calls
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
        // Note: local_variables field doesn't exist yet - commented out for now
        // if let Some(scope) = self.scopes.pop() {
        //     // Clear local variables that were in this scope
        //     for key in scope.keys() {
        //         self.local_variables.remove(key);
        //     }
        // }
    }

    // Call stack management
    pub fn push_call(&mut self, name: String) -> Result<(), String> {
        if self.call_stack.len() >= self.max_call_depth {
            return Err(format!(
                "Maximum recursion depth exceeded ({})",
                self.max_call_depth
            ));
        }
        self.call_stack.push(name);
        Ok(())
    }

    pub fn pop_call(&mut self) {
        self.call_stack.pop();
    }

    /// Get the current function call stack (for error reporting)
    pub fn get_call_stack(&self) -> Vec<String> {
        self.call_stack.clone()
    }

    // Function context tracking for return builtin
    pub fn enter_function_context(&mut self) {
        self.function_depth += 1;
    }

    pub fn exit_function_context(&mut self) {
        if self.function_depth > 0 {
            self.function_depth -= 1;
        }
    }

    pub fn in_function_context(&self) -> bool {
        self.function_depth > 0
    }

    /// Alias for in_function_context (for backward compatibility with local builtin)
    pub fn in_function(&self) -> bool {
        self.in_function_context()
    }

    // Loop context tracking (for break/continue builtins)
    pub fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    pub fn exit_loop(&mut self) {
        if self.loop_depth > 0 {
            self.loop_depth -= 1;
        }
    }

    pub fn get_loop_depth(&self) -> usize {
        self.loop_depth
    }

    /// Set a local variable in the current function scope
    /// Returns an error if not in a function scope
    pub fn set_local_variable(&mut self, name: String, value: String) -> Result<()> {
        if !self.in_function_context() {
            return Err(anyhow!("Cannot set local variable outside of function"));
        }
        // Set in the current scope (which should be a function scope)
        self.set_variable(name, value);
        Ok(())
    }

    // History management
    pub fn history(&mut self) -> &History {
        if self.history.is_none() {
            self.history = Some(Arc::new(History::default()));
        }
        self.history.as_deref().unwrap()
    }

    pub fn history_mut(&mut self) -> &mut History {
        let arc = self
            .history
            .get_or_insert_with(|| Arc::new(History::default()));
        Arc::make_mut(arc)
    }

    pub fn load_history(&mut self) -> Result<(), String> {
        let arc = self
            .history
            .get_or_insert_with(|| Arc::new(History::default()));
        Arc::make_mut(arc).load().map_err(|e| e.to_string())
    }

    pub fn add_to_history(&mut self, command: String) -> Result<(), String> {
        self.history_mut().add(command).map_err(|e| e.to_string())
    }

    // Undo manager access
    pub fn undo_manager(&self) -> Option<&UndoManager> {
        self.undo_manager.as_ref()
    }

    pub fn undo_manager_mut(&mut self) -> &mut UndoManager {
        if self.undo_manager.is_none() {
            // Initialize on first use, falling back to a temp directory if the
            // default location (home dir) is unavailable.
            let manager = UndoManager::new().unwrap_or_else(|e| {
                eprintln!(
                    "Warning: Failed to initialize undo manager: {}. Using temp directory.",
                    e
                );
                let tmp_dir = std::env::temp_dir().join("rush-undo");
                UndoManager::with_undo_dir(tmp_dir).unwrap_or_else(|e2| {
                    eprintln!(
                        "Warning: Undo support disabled (temp fallback also failed: {})",
                        e2
                    );
                    // Last resort: in-memory-only undo with a path that won't be used
                    UndoManager::new_disabled()
                })
            });
            self.undo_manager = Some(manager);
        }
        self.undo_manager.as_mut().unwrap()
    }

    // Job manager access
    pub fn job_manager(&self) -> &JobManager {
        &self.job_manager
    }

    pub fn job_manager_mut(&mut self) -> &mut JobManager {
        &mut self.job_manager
    }

    /// Expand a variable with operators like ${VAR:-default}
    pub fn expand_variable(&mut self, expansion: &VarExpansion) -> Result<String> {
        let var_value = self.get_variable(&expansion.name);

        match &expansion.operator {
            VarExpansionOp::Simple => Ok(var_value.unwrap_or_default()),
            VarExpansionOp::StringLength => Ok(var_value
                .map(|v| v.len().to_string())
                .unwrap_or_else(|| "0".to_string())),
            VarExpansionOp::UseDefault(default) => Ok(var_value.unwrap_or_else(|| default.clone())),
            VarExpansionOp::AssignDefault(default) => {
                if let Some(value) = var_value {
                    Ok(value)
                } else {
                    self.set_variable(expansion.name.clone(), default.clone());
                    Ok(default.clone())
                }
            }
            VarExpansionOp::ErrorIfUnset(error_msg) => match &var_value {
                Some(v) if !v.is_empty() => Ok(v.clone()),
                _ => Err(anyhow!("{}: {}", expansion.name, error_msg)),
            },
            VarExpansionOp::UseAlternate(alternate) => {
                if var_value.as_ref().map_or(false, |v| !v.is_empty()) {
                    Ok(alternate.clone())
                } else {
                    Ok(String::new())
                }
            }
            VarExpansionOp::RemoveShortestPrefix(pattern) => {
                let value = var_value.unwrap_or_default();
                Ok(Self::remove_prefix(&value, pattern, false))
            }
            VarExpansionOp::RemoveLongestPrefix(pattern) => {
                let value = var_value.unwrap_or_default();
                Ok(Self::remove_prefix(&value, pattern, true))
            }
            VarExpansionOp::RemoveShortestSuffix(pattern) => {
                let value = var_value.unwrap_or_default();
                Ok(Self::remove_suffix(&value, pattern, false))
            }
            VarExpansionOp::RemoveLongestSuffix(pattern) => {
                let value = var_value.unwrap_or_default();
                Ok(Self::remove_suffix(&value, pattern, true))
            }
        }
    }

    /// Remove prefix pattern from value
    fn remove_prefix(value: &str, pattern: &str, longest: bool) -> String {
        // Simple glob pattern matching
        if pattern.contains('*') {
            // Extract the literal part after the *
            let literal_part = if pattern.ends_with('*') {
                pattern.trim_end_matches('*')
            } else {
                // Pattern like "*foo/" - the part after * is the literal to match
                pattern.split('*').next_back().unwrap_or("")
            };

            if literal_part.is_empty() {
                return value.to_string();
            }

            if longest {
                // ## - Remove the longest match (find last occurrence)
                if let Some(pos) = value.rfind(literal_part) {
                    return value[pos + literal_part.len()..].to_string();
                }
            } else {
                // # - Remove the shortest match (find first occurrence)
                if let Some(pos) = value.find(literal_part) {
                    return value[pos + literal_part.len()..].to_string();
                }
            }
        } else {
            // Literal prefix match
            if let Some(stripped) = value.strip_prefix(pattern) {
                return stripped.to_string();
            }
        }
        value.to_string()
    }

    /// Remove suffix pattern from value
    fn remove_suffix(value: &str, pattern: &str, longest: bool) -> String {
        // Simple glob pattern matching
        if pattern.contains('*') {
            // Extract the literal part before the *
            let literal_part = if pattern.starts_with('*') {
                pattern.trim_start_matches('*')
            } else {
                // Pattern like ".tar*" - the part before * is the literal to match
                pattern.split('*').next().unwrap_or("")
            };

            if literal_part.is_empty() {
                return value.to_string();
            }

            if longest {
                // %% - Remove the longest match (find first occurrence)
                if let Some(pos) = value.find(literal_part) {
                    return value[..pos].to_string();
                }
            } else {
                // % - Remove the shortest match (find last occurrence)
                if let Some(pos) = value.rfind(literal_part) {
                    return value[..pos].to_string();
                }
            }
        } else {
            // Literal suffix match
            if let Some(stripped) = value.strip_suffix(pattern) {
                return stripped.to_string();
            }
        }
        value.to_string()
    }

    // Positional parameter management

    /// Set all positional parameters ($1, $2, etc.)
    pub fn set_positional_params(&mut self, params: Vec<String>) {
        self.positional_params = params;
        self.update_positional_variables();
    }

    /// Get a specific positional parameter by index (1-based)
    pub fn get_positional_param(&self, index: usize) -> Option<String> {
        if index == 0 {
            // $0 is handled separately
            None
        } else {
            self.positional_params.get(index - 1).cloned()
        }
    }

    /// Get all positional parameters
    pub fn get_positional_params(&self) -> &[String] {
        &self.positional_params
    }

    /// Shift positional parameters by n positions
    pub fn shift_params(&mut self, n: usize) -> Result<()> {
        if n > self.positional_params.len() {
            return Err(anyhow!(
                "shift: shift count ({}) exceeds number of positional parameters ({})",
                n,
                self.positional_params.len()
            ));
        }

        // Remove first n parameters
        self.positional_params.drain(0..n);

        // Update $1, $2, $#, $@, $* variables
        self.update_positional_variables();

        Ok(())
    }

    /// Push positional parameters onto stack (for function calls)
    pub fn push_positional_scope(&mut self, params: Vec<String>) {
        self.positional_stack.push(self.positional_params.clone());
        self.positional_params = params;
        self.update_positional_variables();
    }

    /// Pop positional parameters from stack (after function returns)
    pub fn pop_positional_scope(&mut self) {
        if let Some(params) = self.positional_stack.pop() {
            self.positional_params = params;
            self.update_positional_variables();
        }
    }

    /// Get the count of positional parameters (for $#)
    pub fn param_count(&self) -> usize {
        self.positional_params.len()
    }

    /// Update $1, $2, $#, $@, $* variables based on current positional params
    fn update_positional_variables(&mut self) {
        // Get old count BEFORE updating $#
        let old_count = self
            .variables
            .get("#")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Update $# (parameter count)
        self.variables
            .insert("#".to_string(), self.positional_params.len().to_string());

        // Update $@ and $* (all parameters as space-separated string)
        let all_params = self.positional_params.join(" ");
        self.variables.insert("@".to_string(), all_params.clone());
        self.variables.insert("*".to_string(), all_params);

        // Clear old numbered parameters that are no longer in use
        for i in 1..=old_count.max(self.positional_params.len()) {
            self.variables.remove(&i.to_string());
        }

        // Set new numbered parameters
        for (i, param) in self.positional_params.iter().enumerate() {
            self.variables.insert((i + 1).to_string(), param.clone());
        }
    }

    // Permanent file descriptor redirection management (for exec builtin)

    /// Set permanent stdout redirection file descriptor
    pub fn set_permanent_stdout(&mut self, fd: Option<i32>) {
        self.permanent_stdout = fd;
    }

    /// Get permanent stdout redirection file descriptor
    pub fn get_permanent_stdout(&self) -> Option<i32> {
        self.permanent_stdout
    }

    /// Set permanent stderr redirection file descriptor
    pub fn set_permanent_stderr(&mut self, fd: Option<i32>) {
        self.permanent_stderr = fd;
    }

    /// Get permanent stderr redirection file descriptor
    pub fn get_permanent_stderr(&self) -> Option<i32> {
        self.permanent_stderr
    }

    /// Set permanent stdin redirection file descriptor
    pub fn set_permanent_stdin(&mut self, fd: Option<i32>) {
        self.permanent_stdin = fd;
    }

    /// Get permanent stdin redirection file descriptor
    pub fn get_permanent_stdin(&self) -> Option<i32> {
        self.permanent_stdin
    }

    // Trap handler management

    /// Set a trap handler for a signal
    pub fn set_trap(&mut self, signal: TrapSignal, command: String) {
        self.trap_handlers.set(signal, command);
    }

    /// Remove a trap handler for a signal
    pub fn remove_trap(&mut self, signal: TrapSignal) {
        self.trap_handlers.remove(signal);
    }

    /// Get the trap handler for a signal
    pub fn get_trap(&self, signal: TrapSignal) -> Option<&String> {
        self.trap_handlers.get(signal)
    }

    /// Get all trap handlers
    pub fn get_all_traps(&self) -> &HashMap<TrapSignal, String> {
        self.trap_handlers.all()
    }

    /// Check if a signal has a trap handler
    pub fn has_trap(&self, signal: TrapSignal) -> bool {
        self.trap_handlers.has_handler(signal)
    }

    // Readonly variable management

    /// Mark a variable as readonly
    pub fn mark_readonly(&mut self, name: String) {
        self.readonly_vars.insert(name);
    }

    /// Check if a variable is readonly
    pub fn is_readonly(&self, name: &str) -> bool {
        self.readonly_vars.contains(name)
    }

    /// Get all readonly variable names
    pub fn get_readonly_vars(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.readonly_vars.iter().cloned().collect();
        vars.sort();
        vars
    }

    // Special variable management

    /// Set the PID of the last background job ($!)
    pub fn set_last_bg_pid(&mut self, pid: u32) {
        self.last_bg_pid = Some(pid);
    }

    /// Get the PID of the last background job ($!)
    pub fn get_last_bg_pid(&self) -> Option<u32> {
        self.last_bg_pid
    }

    /// Set the last argument of the previous command ($_)
    pub fn set_last_arg(&mut self, arg: String) {
        self.last_arg = arg;
    }

    /// Get the last argument of the previous command ($_)
    pub fn get_last_arg(&self) -> &str {
        &self.last_arg
    }

    /// Set piped stdin data for compound commands in pipelines
    pub fn set_piped_stdin(&mut self, data: Vec<u8>) {
        self.piped_stdin = Some(data);
    }

    /// Get piped stdin data (if any)
    pub fn get_piped_stdin(&self) -> Option<&[u8]> {
        self.piped_stdin.as_deref()
    }

    /// Take piped stdin data, removing it from the runtime
    pub fn take_piped_stdin(&mut self) -> Option<Vec<u8>> {
        self.piped_stdin.take()
    }

    /// Get current shell options as a flag string (for $-)
    /// Returns flags like "eux" for enabled options
    pub fn get_option_flags(&self) -> String {
        let mut flags = String::new();
        if self.options.errexit {
            flags.push('e');
        }
        if self.options.nounset {
            flags.push('u');
        }
        if self.options.xtrace {
            flags.push('x');
        }
        if self.options.verbose {
            flags.push('v');
        }
        if self.options.noclobber {
            flags.push('C');
        }
        // Note: pipefail is a long option and doesn't have a short flag
        flags
    }

    // Directory stack management (for pushd/popd/dirs builtins)

    /// Push a directory onto the stack
    pub fn push_dir(&mut self, dir: PathBuf) {
        self.dir_stack.push(dir);
    }

    /// Pop a directory from the stack
    pub fn pop_dir(&mut self) -> Option<PathBuf> {
        self.dir_stack.pop()
    }

    /// Get the directory stack (returns a reference)
    pub fn get_dir_stack(&self) -> &[PathBuf] {
        &self.dir_stack
    }

    /// Whether agent mode is active (JSON output + ANSI stripping for all built-ins)
    pub fn agent_mode(&self) -> bool {
        self.agent_mode
    }

    /// Enable or disable agent mode programmatically (used by the run() API)
    pub fn set_agent_mode(&mut self, enabled: bool) {
        self.agent_mode = enabled;
    }

    /// Clear the directory stack
    pub fn clear_dir_stack(&mut self) {
        self.dir_stack.clear();
    }

    /// Reset runtime state between command executions
    /// Clears variables, scopes, call stack, positional params, etc.
    /// Preserves history, undo_manager, and job_manager for reuse
    pub fn reset(&mut self) -> Result<()> {
        // Clear variables (except special ones we want to preserve)
        self.variables.clear();
        self.readonly_vars.clear();

        // Clear functions and aliases
        Arc::make_mut(&mut self.functions).clear();
        self.aliases.clear();

        // Reset scopes and call stack
        self.scopes.clear();
        self.call_stack.clear();

        // Reset positional parameters
        self.positional_params.clear();
        self.positional_stack.clear();

        // Reset depths
        self.function_depth = 0;
        self.loop_depth = 0;

        // Clear trap handlers
        self.trap_handlers = TrapHandlers::new();

        // Clear permanent redirections
        self.permanent_stdout = None;
        self.permanent_stderr = None;
        self.permanent_stdin = None;

        // Clear special variables
        self.last_bg_pid = None;
        self.last_arg = String::new();

        // Clear directory stack
        self.dir_stack.clear();

        // Reset cwd to current directory
        self.cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        // Reset shell options to defaults
        self.options = ShellOptions::default();

        // Reinitialize required variables
        self.set_last_exit_code(0);
        self.set_variable("IFS".to_string(), " \t\n".to_string());

        // Keep history, undo_manager, and job_manager (they persist across commands)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_reset_clears_variables() {
        let mut rt = Runtime::new();
        rt.set_variable("FOO".to_string(), "bar".to_string());
        rt.set_variable("BAZ".to_string(), "qux".to_string());
        assert_eq!(rt.get_variable("FOO"), Some("bar".to_string()));

        rt.reset().unwrap();

        assert_eq!(rt.get_variable("FOO"), None);
        assert_eq!(rt.get_variable("BAZ"), None);
    }

    #[test]
    fn test_runtime_reset_clears_functions() {
        let mut rt = Runtime::new();
        let func = FunctionDef {
            name: "myfunc".to_string(),
            params: vec![],
            body: vec![],
        };
        rt.define_function(func);
        assert!(rt.get_function("myfunc").is_some());

        rt.reset().unwrap();

        assert!(rt.get_function("myfunc").is_none());
    }

    #[test]
    fn test_runtime_reset_clears_aliases() {
        let mut rt = Runtime::new();
        rt.set_alias("ll".to_string(), "ls -la".to_string());
        assert!(rt.get_alias("ll").is_some());

        rt.reset().unwrap();

        assert!(rt.get_alias("ll").is_none());
    }

    #[test]
    fn test_runtime_reset_clears_scopes_and_call_stack() {
        let mut rt = Runtime::new();
        rt.push_scope();
        rt.set_variable("LOCAL".to_string(), "val".to_string());
        rt.push_call("func1".to_string()).unwrap();

        rt.reset().unwrap();

        // After reset, setting a variable should go to global scope (no scopes)
        rt.set_variable("X".to_string(), "1".to_string());
        assert_eq!(rt.get_variable("X"), Some("1".to_string()));
    }

    #[test]
    fn test_runtime_reset_clears_positional_params() {
        let mut rt = Runtime::new();
        rt.set_positional_params(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(rt.param_count(), 3);

        rt.reset().unwrap();

        assert_eq!(rt.param_count(), 0);
        assert!(rt.get_positional_params().is_empty());
    }

    #[test]
    fn test_runtime_reset_resets_depths() {
        let mut rt = Runtime::new();
        rt.enter_function_context();
        rt.enter_loop();
        rt.enter_loop();
        assert!(rt.in_function_context());
        assert_eq!(rt.get_loop_depth(), 2);

        rt.reset().unwrap();

        assert!(!rt.in_function_context());
        assert_eq!(rt.get_loop_depth(), 0);
    }

    #[test]
    fn test_runtime_reset_clears_shell_options() {
        let mut rt = Runtime::new();
        rt.set_option("e", true).unwrap();
        rt.set_option("x", true).unwrap();
        assert!(rt.options.errexit);
        assert!(rt.options.xtrace);

        rt.reset().unwrap();

        assert!(!rt.options.errexit);
        assert!(!rt.options.xtrace);
    }

    #[test]
    fn test_runtime_reset_reinitializes_defaults() {
        let mut rt = Runtime::new();
        rt.set_last_exit_code(42);
        rt.set_variable("IFS".to_string(), ",".to_string());

        rt.reset().unwrap();

        // $? should be 0 after reset
        assert_eq!(rt.get_last_exit_code(), 0);
        // IFS should be restored to default
        assert_eq!(rt.get_ifs(), " \t\n");
    }

    #[test]
    fn test_runtime_reset_clears_readonly() {
        let mut rt = Runtime::new();
        rt.set_variable("CONST".to_string(), "value".to_string());
        rt.mark_readonly("CONST".to_string());
        assert!(rt.is_readonly("CONST"));

        rt.reset().unwrap();

        assert!(!rt.is_readonly("CONST"));
    }

    #[test]
    fn test_runtime_reset_clears_redirections() {
        let mut rt = Runtime::new();
        rt.set_permanent_stdout(Some(3));
        rt.set_permanent_stderr(Some(4));
        rt.set_permanent_stdin(Some(5));

        rt.reset().unwrap();

        assert_eq!(rt.get_permanent_stdout(), None);
        assert_eq!(rt.get_permanent_stderr(), None);
        assert_eq!(rt.get_permanent_stdin(), None);
    }

    #[test]
    fn test_runtime_reset_clears_special_vars() {
        let mut rt = Runtime::new();
        rt.set_last_bg_pid(12345);
        rt.set_last_arg("hello".to_string());

        rt.reset().unwrap();

        assert_eq!(rt.get_last_bg_pid(), None);
        assert_eq!(rt.get_last_arg(), "");
    }

    #[test]
    fn test_runtime_reset_clears_dir_stack() {
        let mut rt = Runtime::new();
        rt.push_dir(PathBuf::from("/tmp"));
        rt.push_dir(PathBuf::from("/home"));
        assert_eq!(rt.get_dir_stack().len(), 2);

        rt.reset().unwrap();

        assert!(rt.get_dir_stack().is_empty());
    }

    #[test]
    fn test_runtime_reset_no_state_leakage() {
        let mut rt = Runtime::new();

        // Simulate first command execution
        rt.set_variable("SECRET".to_string(), "password123".to_string());
        rt.set_last_exit_code(1);
        rt.set_option("e", true).unwrap();
        rt.set_alias("x".to_string(), "exit".to_string());
        rt.enter_function_context();
        rt.push_scope();
        rt.set_variable("LOCAL".to_string(), "val".to_string());

        // Reset between commands
        rt.reset().unwrap();

        // Verify no state leaked
        assert_eq!(rt.get_variable("SECRET"), None);
        assert_eq!(rt.get_variable("LOCAL"), None);
        assert_eq!(rt.get_last_exit_code(), 0);
        assert!(!rt.options.errexit);
        assert!(rt.get_alias("x").is_none());
        assert!(!rt.in_function_context());
    }
}
