use crate::correction::Corrector;
use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

mod cat;
mod find;
mod getopts;
#[cfg(feature = "git-builtins")]
mod git_log;
#[cfg(feature = "git-builtins")]
mod git_status;
// mod git_diff;  // Temporarily disabled due to compilation errors
mod alias;
pub mod break_builtin; // Public so executor can access BreakSignal
mod builtin;
mod command;
pub mod continue_builtin; // Public so executor can access ContinueSignal
mod eval;
mod exec;
pub mod exit_builtin; // Public so executor/main can access ExitSignal
mod fetch;
mod grep;
mod help;
mod jobs;
mod json;
mod kill;
mod local;
mod ls;
mod mkdir;
mod printf;
mod profile;
mod read;
mod readonly;
pub mod return_builtin; // Public so executor can access ReturnSignal
mod rm;
mod set;
mod shift;
mod test;
pub mod time; // Public so executor can access timing functions
pub mod trap; // Public so runtime and executor can access TrapSignal
mod type_builtin;
mod undo;
mod unset;
mod wait;

type BuiltinFn = fn(&[String], &mut Runtime) -> Result<ExecutionResult>;

/// Process-global builtin table. Initialized once on first access via LazyLock.
/// Uses &'static str keys to avoid per-Executor String allocations.
static BUILTIN_MAP: LazyLock<HashMap<&'static str, BuiltinFn>> = LazyLock::new(|| {
    let mut m: HashMap<&'static str, BuiltinFn> = HashMap::with_capacity(50);
    m.insert("cd", builtin_cd as BuiltinFn);
    m.insert("pwd", builtin_pwd);
    m.insert("echo", builtin_echo);
    m.insert("exit", exit_builtin::builtin_exit);
    m.insert("export", builtin_export);
    m.insert("source", builtin_source);
    m.insert(".", builtin_source);
    m.insert("cat", cat::builtin_cat);
    m.insert("find", find::builtin_find);
    m.insert("ls", ls::builtin_ls);
    m.insert("mkdir", mkdir::builtin_mkdir);
    #[cfg(feature = "git-builtins")]
    m.insert("git", builtin_git);
    #[cfg(not(feature = "git-builtins"))]
    m.insert("git", builtin_git_external);
    m.insert("grep", grep::builtin_grep);
    m.insert("undo", undo::builtin_undo);
    m.insert("jobs", jobs::builtin_jobs);
    m.insert("fg", jobs::builtin_fg);
    m.insert("bg", jobs::builtin_bg);
    m.insert("set", set::builtin_set);
    m.insert("alias", alias::builtin_alias);
    m.insert("unalias", alias::builtin_unalias);
    m.insert("test", test::builtin_test);
    m.insert("[", test::builtin_bracket);
    m.insert("help", help::builtin_help);
    m.insert("type", type_builtin::builtin_type);
    m.insert("shift", shift::builtin_shift);
    m.insert("local", local::builtin_local);
    m.insert("true", builtin_true);
    m.insert("false", builtin_false);
    m.insert("return", return_builtin::builtin_return);
    m.insert("trap", trap::builtin_trap);
    m.insert("unset", unset::builtin_unset);
    m.insert("printf", printf::builtin_printf);
    m.insert("read", read::builtin_read);
    m.insert("eval", eval::builtin_eval);
    m.insert("exec", exec::builtin_exec);
    m.insert("builtin", builtin::builtin_builtin);
    m.insert("kill", kill::builtin_kill);
    m.insert("break", break_builtin::builtin_break);
    m.insert("continue", continue_builtin::builtin_continue);
    m.insert(":", builtin_colon);
    m.insert("command", command::builtin_command);
    m.insert("json_get", json::builtin_json_get);
    m.insert("json_set", json::builtin_json_set);
    m.insert("json_query", json::builtin_json_query);
    m.insert("fetch", fetch::builtin_fetch);
    m.insert("readonly", readonly::builtin_readonly);
    m.insert("rm", rm::builtin_rm);
    m.insert("wait", wait::builtin_wait);
    m.insert("profile", profile::builtin_profile);
    m.insert("time", time::builtin_time);
    m.insert("getopts", getopts::builtin_getopts);
    m
});

/// Zero-cost wrapper around the process-global builtin table.
/// Creating a Builtins instance does no allocation — the static map
/// is initialized once on first use.
#[derive(Clone)]
pub struct Builtins;

impl Default for Builtins {
    fn default() -> Self {
        Self::new()
    }
}

impl Builtins {
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn is_builtin(&self, name: &str) -> bool {
        BUILTIN_MAP.contains_key(name)
    }

    pub fn builtin_names(&self) -> Vec<String> {
        BUILTIN_MAP.keys().map(|k| k.to_string()).collect()
    }

    #[inline]
    pub fn execute(
        &self,
        name: &str,
        args: Vec<String>,
        runtime: &mut Runtime,
    ) -> Result<ExecutionResult> {
        if let Some(func) = BUILTIN_MAP.get(name) {
            func(&args, runtime)
        } else {
            Err(anyhow!("Builtin '{}' not found", name))
        }
    }

    /// Execute a builtin with optional stdin data
    pub fn execute_with_stdin(
        &self,
        name: &str,
        args: Vec<String>,
        runtime: &mut Runtime,
        stdin: Option<&[u8]>,
    ) -> Result<ExecutionResult> {
        // Special handling for cat with stdin
        if name == "cat" {
            if let Some(stdin_data) = stdin {
                return cat::builtin_cat_with_stdin(&args, runtime, stdin_data);
            }
        }

        // Special handling for grep with stdin
        if name == "grep" {
            if let Some(stdin_data) = stdin {
                return grep::builtin_grep_with_stdin(&args, runtime, stdin_data);
            }
        }

        // Special handling for read with stdin
        if name == "read" {
            if let Some(stdin_data) = stdin {
                return read::builtin_read_with_stdin(&args, runtime, stdin_data);
            }
        }

        // Special handling for JSON builtins with stdin
        if name == "json_get" {
            if let Some(stdin_data) = stdin {
                return json::builtin_json_get_with_stdin(&args, runtime, stdin_data);
            }
        }

        if name == "json_set" {
            if let Some(stdin_data) = stdin {
                return json::builtin_json_set_with_stdin(&args, runtime, stdin_data);
            }
        }

        if name == "json_query" {
            if let Some(stdin_data) = stdin {
                return json::builtin_json_query_with_stdin(&args, runtime, stdin_data);
            }
        }

        // For other builtins, use regular execute
        self.execute(name, args, runtime)
    }
}

pub(crate) fn builtin_cd(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let target = if args.is_empty() {
        dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?
    } else {
        let path = &args[0];
        if path == "-" {
            // cd - goes to OLDPWD
            if let Some(oldpwd) = runtime.get_variable("OLDPWD") {
                PathBuf::from(oldpwd)
            } else {
                return Err(anyhow!("cd: OLDPWD not set"));
            }
        } else if path.starts_with('~') {
            let home =
                dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
            home.join(path.trim_start_matches("~/"))
        } else {
            PathBuf::from(path)
        }
    };

    // Keep original argument for CDPATH check
    let original_arg = args.first().map(|s| s.as_str()).unwrap_or("");

    let absolute = if target.is_absolute() {
        target
    } else {
        runtime.get_cwd().join(target)
    };

    // CDPATH: if the path doesn't exist and the argument is a bare name (not starting
    // with /, ./, or ../), search CDPATH directories for a match.
    let (absolute, used_cdpath) = if !absolute.exists()
        && !args.is_empty()
        && !original_arg.starts_with('/')
        && !original_arg.starts_with("./")
        && !original_arg.starts_with("../")
        && original_arg != "-"
        && !original_arg.starts_with('~')
    {
        let mut found = None;
        if let Some(cdpath) = runtime.get_variable("CDPATH") {
            for dir in cdpath.split(':') {
                if dir.is_empty() {
                    continue;
                }
                let candidate = PathBuf::from(dir).join(original_arg);
                if candidate.is_dir() {
                    found = Some(candidate);
                    break;
                }
            }
        }
        match found {
            Some(path) => (path, true),
            None => (absolute, false),
        }
    } else {
        (absolute, false)
    };

    if !absolute.exists() {
        // Provide path suggestions
        let corrector = Corrector::new();
        let suggestions = corrector.suggest_path(&absolute, runtime.get_cwd());

        let mut error_msg = format!("cd: no such file or directory: {:?}", absolute);

        if !suggestions.is_empty() {
            error_msg.push_str("\n\nDid you mean?");
            for suggestion in suggestions.iter().take(3) {
                let similarity = Corrector::similarity_percent(suggestion.score, &suggestion.text);
                error_msg.push_str(&format!(
                    "\n  {} ({}%, {})",
                    suggestion.text,
                    similarity,
                    suggestion.kind.label()
                ));
            }
        }

        return Err(anyhow!(error_msg));
    }

    if !absolute.is_dir() {
        return Err(anyhow!("cd: not a directory: {:?}", absolute));
    }

    // Save current PWD to OLDPWD before changing
    let current_pwd = runtime.get_cwd().to_string_lossy().to_string();
    runtime.set_variable("OLDPWD".to_string(), current_pwd.clone());

    // Update runtime's cwd
    runtime.set_cwd(absolute.clone());

    // Also update the process's actual current directory so other parts can see it
    env::set_current_dir(&absolute)?;

    // Update PWD variable to new directory
    let new_pwd = absolute.to_string_lossy().to_string();
    runtime.set_variable("PWD".to_string(), new_pwd.clone());

    // Print the resolved path when cd - or CDPATH was used (POSIX requirement)
    if (!args.is_empty() && args[0] == "-") || used_cdpath {
        return Ok(ExecutionResult::success(new_pwd + "\n"));
    }

    Ok(ExecutionResult::success(String::new()))
}

pub(crate) fn builtin_pwd(_args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();
    Ok(ExecutionResult::success(
        cwd.to_string_lossy().to_string() + "\n",
    ))
}

pub(crate) fn builtin_echo(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let output = args.join(" ") + "\n";
    Ok(ExecutionResult::success(output))
}

// builtin_exit is now in exit_builtin module (uses ExitSignal for subshell support)

pub(crate) fn builtin_export(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("export: usage: export VAR=value"));
    }

    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            runtime.set_env(key, value);
            runtime.set_variable(key.to_string(), value.to_string());
        } else {
            return Err(anyhow!("export: invalid syntax: {}", arg));
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

#[cfg(feature = "git-builtins")]
pub(crate) fn builtin_git(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        // No subcommand provided - let external git handle it
        return Err(anyhow!("git: missing subcommand"));
    }

    match args[0].as_str() {
        "status" => {
            // Call the optimized git status builtin
            git_status::builtin_git_status(&args[1..], runtime)
        }
        "log" => {
            // Call the optimized git log builtin
            git_log::builtin_git_log(&args[1..], runtime)
        }
        _ => {
            // For other git subcommands, spawn external git
            builtin_git_external(args, runtime)
        }
    }
}

/// Fallback: always shell out to external git (used when git-builtins feature is disabled)
pub(crate) fn builtin_git_external(
    args: &[String],
    runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    use std::process::Command;

    let output = Command::new("git")
        .args(args)
        .current_dir(runtime.get_cwd())
        .output()
        .map_err(|e| anyhow!("Failed to execute git: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(1);

    Ok(ExecutionResult {
        output: Output::Text(stdout),
        stderr,
        exit_code,
        error: None,
    })
}

pub(crate) fn builtin_true(_args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

pub(crate) fn builtin_false(_args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: 1,
        error: None,
    })
}

pub(crate) fn builtin_colon(_args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

// TODO: Implement builtin_source properly with executor access
#[allow(dead_code)]
pub(crate) fn builtin_source(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("source: usage: source <file>"));
    }

    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use std::fs;
    use std::io::{BufRead, BufReader};

    let file_path = &args[0];
    let path = if file_path.starts_with('~') {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        home.join(file_path.trim_start_matches("~/"))
    } else {
        PathBuf::from(file_path)
    };

    // Make path absolute if relative
    let path = if path.is_absolute() {
        path
    } else {
        runtime.get_cwd().join(path)
    };

    if !path.exists() {
        return Err(anyhow!("source: {}: No such file or directory", file_path));
    }

    // Read and execute file
    let file = fs::File::open(&path)
        .map_err(|e| anyhow!("source: Failed to open '{}': {}", path.display(), e))?;
    let reader = BufReader::new(file);

    // Enter function context for sourced scripts (allows return)
    runtime.enter_function_context();

    // We need an executor to run the commands, but we can't access it from here
    // So we'll return the file contents as a special marker that main.rs can handle
    // For now, execute line by line in a basic way
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse and execute - we need to do this carefully
        // since we don't have access to executor here
        match Lexer::tokenize(line) {
            Ok(tokens) => {
                let mut parser = Parser::new(tokens);
                match parser.parse() {
                    Ok(statements) => {
                        // Create temporary executor with current runtime
                        let mut executor = Executor::new();
                        // Copy runtime state (this is not ideal but works for source)
                        *executor.runtime_mut() = runtime.clone();

                        match executor.execute(statements) {
                            Ok(result) => {
                                // Copy back runtime state to preserve variable changes
                                *runtime = executor.runtime_mut().clone();
                                // Print any output
                                if !result.stdout().is_empty() {
                                    print!("{}", result.stdout());
                                }
                                if !result.stderr.is_empty() {
                                    eprint!("{}", result.stderr);
                                }
                            }
                            Err(e) => {
                                // Check if this is a return signal from sourced script
                                if let Some(return_signal) =
                                    e.downcast_ref::<return_builtin::ReturnSignal>()
                                {
                                    // Early return from sourced script
                                    runtime.exit_function_context();
                                    return Ok(ExecutionResult {
                                        output: Output::Text(String::new()),
                                        stderr: String::new(),
                                        exit_code: return_signal.exit_code,
                                        error: None,
                                    });
                                }
                                eprintln!("{}:{}: {}", path.display(), line_num + 1, e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}:{}: Parse error: {}", path.display(), line_num + 1, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}:{}: Tokenize error: {}", path.display(), line_num + 1, e);
            }
        }
    }

    // Exit function context after sourced script completes
    runtime.exit_function_context();

    Ok(ExecutionResult::success(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let mut runtime = Runtime::new();
        let result =
            builtin_echo(&["hello".to_string(), "world".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "hello world\n");
    }

    #[test]
    fn test_pwd() {
        let mut runtime = Runtime::new();
        let result = builtin_pwd(&[], &mut runtime).unwrap();
        assert!(!result.stdout().is_empty());
    }

    #[test]
    fn test_true_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_true(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr, "");
    }

    #[test]
    fn test_false_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_false(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr, "");
    }

    #[test]
    fn test_true_ignores_arguments() {
        let mut runtime = Runtime::new();
        let args = vec!["arg1".to_string(), "arg2".to_string(), "--flag".to_string()];
        let result = builtin_true(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_false_ignores_arguments() {
        let mut runtime = Runtime::new();
        let args = vec!["arg1".to_string(), "arg2".to_string(), "--flag".to_string()];
        let result = builtin_false(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_colon_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_colon(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr, "");
    }

    #[test]
    fn test_colon_ignores_arguments() {
        let mut runtime = Runtime::new();
        let args = vec!["arg1".to_string(), "arg2".to_string(), "--flag".to_string()];
        let result = builtin_colon(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr, "");
    }

    #[test]
    fn test_colon_with_many_arguments() {
        let mut runtime = Runtime::new();
        let args = vec![
            "foo".to_string(),
            "bar".to_string(),
            "baz".to_string(),
            "qux".to_string(),
        ];
        let result = builtin_colon(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }
}
