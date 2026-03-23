use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::env;
use std::path::Path;

/// Execute a command, bypassing function and alias lookup
///
/// The `command` builtin runs commands without function or alias lookup.
/// This is useful when you want to ensure you're running the actual builtin or external command.
///
/// Options:
/// - `-p`: Use a default PATH instead of the current PATH value
/// - `-v`: Print a description of command (like `type` builtin)
/// - `-V`: Print a verbose description of command
///
/// If no options are given, `command` executes the specified command with arguments,
/// bypassing function and alias lookup but still allowing builtins.
pub fn builtin_command(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("command: usage: command [-pVv] command [args...]"));
    }

    let mut use_default_path = false;
    let mut print_verbose = false;
    let mut print_description = false;
    let mut arg_idx = 0;

    // Parse flags
    while arg_idx < args.len() && args[arg_idx].starts_with('-') && args[arg_idx].len() > 1 {
        let flag = &args[arg_idx];

        // Stop at "--"
        if flag == "--" {
            arg_idx += 1;
            break;
        }

        // Parse combined flags like -pv
        for ch in flag.chars().skip(1) {
            match ch {
                'p' => use_default_path = true,
                'V' => print_verbose = true,
                'v' => print_description = true,
                _ => return Err(anyhow!("command: invalid option: -{}", ch)),
            }
        }

        arg_idx += 1;
    }

    // Must have at least a command name
    if arg_idx >= args.len() {
        return Err(anyhow!("command: usage: command [-pVv] command [args...]"));
    }

    let command_name = &args[arg_idx];
    let command_args = if arg_idx + 1 < args.len() {
        args[arg_idx + 1..].to_vec()
    } else {
        vec![]
    };

    // -v or -V: Just print command description
    if print_description || print_verbose {
        return print_command_info(command_name, print_verbose, use_default_path, runtime);
    }

    // Execute the command, bypassing functions and aliases
    execute_command_bypassing_lookup(command_name, command_args, use_default_path, runtime)
}

/// Print information about a command (for -v and -V flags)
fn print_command_info(
    name: &str,
    verbose: bool,
    use_default_path: bool,
    _runtime: &Runtime,
) -> Result<ExecutionResult> {
    // Check if it's a builtin (builtins are not bypassed by command)
    if is_builtin(name) {
        let output = if verbose {
            format!("{} is a shell builtin\n", name)
        } else {
            format!("{}\n", name)
        };
        return Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        });
    }

    // Search for external command (functions and aliases are bypassed)
    let path_to_use = if use_default_path {
        get_default_path()
    } else {
        env::var("PATH").ok()
    };

    if let Some(path) = find_in_path_custom(name, path_to_use) {
        let output = if verbose {
            format!("{} is {}\n", name, path)
        } else {
            format!("{}\n", path)
        };
        return Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        });
    }

    // Command not found
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: format!("command: {}: not found\n", name),
        exit_code: 1,
        error: None,
    })
}

/// Execute a command, bypassing function and alias lookup
fn execute_command_bypassing_lookup(
    name: &str,
    args: Vec<String>,
    use_default_path: bool,
    runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    // Check if it's a builtin - builtins are still executed
    if is_builtin(name) {
        return execute_builtin(name, args, runtime);
    }

    // Find external command (bypassing functions and aliases)
    let path_to_use = if use_default_path {
        get_default_path()
    } else {
        env::var("PATH").ok()
    };

    if let Some(cmd_path) = find_in_path_custom(name, path_to_use) {
        return execute_external_command(&cmd_path, args, runtime);
    }

    // Command not found
    Err(anyhow!("command: {}: not found", name))
}

/// Execute a builtin command
fn execute_builtin(
    name: &str,
    args: Vec<String>,
    runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    // Dispatch to the correct builtin
    match name {
        "cd" => super::builtin_cd(&args, runtime),
        "pwd" => super::builtin_pwd(&args, runtime),
        "echo" => super::builtin_echo(&args, runtime),
        "exit" => super::exit_builtin::builtin_exit(&args, runtime),
        "export" => super::builtin_export(&args, runtime),
        "source" => super::builtin_source(&args, runtime),
        "cat" => super::cat::builtin_cat(&args, runtime),
        "find" => super::find::builtin_find(&args, runtime),
        "ls" => super::ls::builtin_ls(&args, runtime),
        "mkdir" => super::mkdir::builtin_mkdir(&args, runtime),
        "git" => super::builtin_git_external(&args, runtime),
        "grep" => super::grep::builtin_grep(&args, runtime),
        "undo" => super::undo::builtin_undo(&args, runtime),
        "jobs" => super::jobs::builtin_jobs(&args, runtime),
        "fg" => super::jobs::builtin_fg(&args, runtime),
        "bg" => super::jobs::builtin_bg(&args, runtime),
        "set" => super::set::builtin_set(&args, runtime),
        "alias" => super::alias::builtin_alias(&args, runtime),
        "unalias" => super::alias::builtin_unalias(&args, runtime),
        "test" => super::test::builtin_test(&args, runtime),
        "[" => super::test::builtin_bracket(&args, runtime),
        "help" => super::help::builtin_help(&args, runtime),
        "type" => super::type_builtin::builtin_type(&args, runtime),
        "shift" => super::shift::builtin_shift(&args, runtime),
        "local" => super::local::builtin_local(&args, runtime),
        "true" => super::builtin_true(&args, runtime),
        "false" => super::builtin_false(&args, runtime),
        "return" => super::return_builtin::builtin_return(&args, runtime),
        "trap" => super::trap::builtin_trap(&args, runtime),
        "unset" => super::unset::builtin_unset(&args, runtime),
        "printf" => super::printf::builtin_printf(&args, runtime),
        "read" => super::read::builtin_read(&args, runtime),
        "eval" => super::eval::builtin_eval(&args, runtime),
        "exec" => super::exec::builtin_exec(&args, runtime),
        "builtin" => super::builtin::builtin_builtin(&args, runtime),
        "kill" => super::kill::builtin_kill(&args, runtime),
        "break" => super::break_builtin::builtin_break(&args, runtime),
        "continue" => super::continue_builtin::builtin_continue(&args, runtime),
        ":" => super::builtin_colon(&args, runtime),
        "json_get" => super::json::builtin_json_get(&args, runtime),
        "json_set" => super::json::builtin_json_set(&args, runtime),
        "json_query" => super::json::builtin_json_query(&args, runtime),
        _ => Err(anyhow!("command: {}: not a shell builtin", name)),
    }
}

/// Execute an external command
fn execute_external_command(
    cmd_path: &str,
    args: Vec<String>,
    runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    use std::process::Command;

    let output = Command::new(cmd_path)
        .args(&args)
        .current_dir(runtime.get_cwd())
        .output()
        .map_err(|e| anyhow!("Failed to execute {}: {}", cmd_path, e))?;

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

/// Check if a command is a builtin
fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "cd" | "pwd"
            | "echo"
            | "exit"
            | "export"
            | "source"
            | "cat"
            | "find"
            | "ls"
            | "mkdir"
            | "git"
            | "grep"
            | "undo"
            | "jobs"
            | "fg"
            | "bg"
            | "set"
            | "alias"
            | "unalias"
            | "test"
            | "["
            | "help"
            | "type"
            | "shift"
            | "local"
            | "true"
            | "false"
            | "return"
            | "trap"
            | "unset"
            | "printf"
            | "read"
            | "eval"
            | "exec"
            | "builtin"
            | "kill"
            | "break"
            | "continue"
            | ":"
            | "json_get"
            | "json_set"
            | "json_query"
    )
}

/// Find a command in PATH with custom PATH value
fn find_in_path_custom(command: &str, path_env: Option<String>) -> Option<String> {
    // If the command contains a path separator, check if it exists directly
    if command.contains('/') {
        let path = Path::new(command);
        if path.exists() && path.is_file() {
            return Some(command.to_string());
        }
        return None;
    }

    // Search in PATH
    let path_env = path_env?;

    for dir in path_env.split(':') {
        let full_path = Path::new(dir).join(command);
        if full_path.exists() && full_path.is_file() {
            // Check if the file is executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = full_path.metadata() {
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        return Some(full_path.to_string_lossy().to_string());
                    }
                }
            }

            #[cfg(not(unix))]
            {
                return Some(full_path.to_string_lossy().to_string());
            }
        }
    }

    None
}

/// Get a default PATH value for -p flag
/// POSIX specifies this should be a "default" path that is guaranteed to find standard utilities
fn get_default_path() -> Option<String> {
    // POSIX default path - these directories should contain standard utilities
    Some("/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::FunctionDef;

    #[test]
    fn test_command_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_command(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_command_echo_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["echo".to_string(), "hello".to_string(), "world".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "hello world\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_bypasses_function() {
        let mut runtime = Runtime::new();

        // Define a function named "echo"
        let func = FunctionDef {
            name: "echo".to_string(),
            params: vec![],
            body: vec![],
        };
        runtime.define_function(func);

        // Verify function exists
        assert!(runtime.get_function("echo").is_some());

        // command echo should still execute the builtin, not the function
        let args = vec!["echo".to_string(), "test".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "test\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_bypasses_alias() {
        let mut runtime = Runtime::new();

        // Define an alias for echo
        runtime.set_alias("echo".to_string(), "echo ALIASED".to_string());

        // Verify alias exists
        assert!(runtime.get_alias("echo").is_some());

        // command echo should execute the builtin, bypassing the alias
        let args = vec!["echo".to_string(), "test".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "test\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_v_flag_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["-v".to_string(), "echo".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "echo\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_V_flag_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["-V".to_string(), "pwd".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "pwd is a shell builtin\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_v_flag_external() {
        let mut runtime = Runtime::new();
        let args = vec!["-v".to_string(), "sh".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        // Should print the path to sh
        assert!(result.stdout().starts_with('/'));
        assert!(result.stdout().contains("sh"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_V_flag_external() {
        let mut runtime = Runtime::new();
        let args = vec!["-V".to_string(), "sh".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        // Should print "sh is /path/to/sh"
        assert!(result.stdout().contains("sh is /"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_v_flag_not_found() {
        let mut runtime = Runtime::new();
        let args = vec!["-v".to_string(), "nonexistent_cmd_xyz".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("not found"));
    }

    #[test]
    fn test_command_invalid_flag() {
        let mut runtime = Runtime::new();
        let args = vec!["-x".to_string(), "echo".to_string()];
        let result = builtin_command(&args, &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid option"));
    }

    #[test]
    fn test_command_combined_flags() {
        let mut runtime = Runtime::new();
        let args = vec!["-pv".to_string(), "sh".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        // Should use default path and print short description
        assert!(result.stdout().contains("sh"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_double_dash() {
        let mut runtime = Runtime::new();
        let args = vec!["--".to_string(), "echo".to_string(), "test".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "test\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_pwd_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["pwd".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert!(!result.stdout().is_empty());
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_true_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["true".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_command_false_builtin() {
        let mut runtime = Runtime::new();
        let args = vec!["false".to_string()];
        let result = builtin_command(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_command_nonexistent() {
        let mut runtime = Runtime::new();
        let args = vec!["nonexistent_cmd_xyz".to_string()];
        let result = builtin_command(&args, &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
