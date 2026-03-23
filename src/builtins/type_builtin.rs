use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::env;
use std::path::Path;

pub fn builtin_type(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("type: usage: type name [name ...]"));
    }

    let mut output = String::new();
    let mut exit_code = 0;

    for arg in args {
        match get_command_type(arg, runtime) {
            Some(CommandType::Builtin) => {
                output.push_str(&format!("{} is a shell builtin\n", arg));
            }
            Some(CommandType::Function) => {
                output.push_str(&format!("{} is a function\n", arg));
            }
            Some(CommandType::Alias(value)) => {
                output.push_str(&format!("{} is aliased to '{}'\n", arg, value));
            }
            Some(CommandType::External(path)) => {
                output.push_str(&format!("{} is {}\n", arg, path));
            }
            None => {
                output.push_str(&format!("{}: not found\n", arg));
                exit_code = 1;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

enum CommandType {
    Builtin,
    Function,
    Alias(String),
    External(String),
}

fn get_command_type(name: &str, runtime: &Runtime) -> Option<CommandType> {
    // Check if it's a builtin (highest priority)
    if is_builtin(name) {
        return Some(CommandType::Builtin);
    }

    // Check if it's an alias
    if let Some(value) = runtime.get_alias(name) {
        return Some(CommandType::Alias(value.clone()));
    }

    // Check if it's a user-defined function
    if runtime.get_function(name).is_some() {
        return Some(CommandType::Function);
    }

    // Search PATH for external command
    if let Some(path) = find_in_path(name) {
        return Some(CommandType::External(path));
    }

    None
}

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
            | "git-status"
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
            | "read"
            | "trap"
            | "unset"
            | "printf"
            | "eval"
            | "exec"
            | "kill"
    )
}

fn find_in_path(command: &str) -> Option<String> {
    // If the command contains a path separator, check if it exists directly
    if command.contains('/') {
        let path = Path::new(command);
        if path.exists() && path.is_file() {
            return Some(command.to_string());
        }
        return None;
    }

    // Search in PATH
    let path_env = env::var("PATH").ok()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::FunctionDef;
    use crate::runtime::Runtime;

    #[test]
    fn test_type_builtin() {
        let mut runtime = Runtime::new();

        let result = builtin_type(&["cd".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "cd is a shell builtin\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_alias() {
        let mut runtime = Runtime::new();

        // Create an alias
        runtime.set_alias("ll".to_string(), "ls -la".to_string());

        let result = builtin_type(&["ll".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "ll is aliased to 'ls -la'\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_external() {
        let mut runtime = Runtime::new();

        // Test with a common external command that should exist on most systems
        let result = builtin_type(&["sh".to_string()], &mut runtime).unwrap();
        // sh should be found as an external command
        assert!(result.stdout().contains("sh is /"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_function() {
        let mut runtime = Runtime::new();

        // Define a test function
        let func = FunctionDef {
            name: "myfunc".to_string(),
            params: vec![],
            body: vec![],
        };
        runtime.define_function(func);

        let result = builtin_type(&["myfunc".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "myfunc is a function\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_not_found() {
        let mut runtime = Runtime::new();

        let result = builtin_type(&["nonexistent_command_xyz".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "nonexistent_command_xyz: not found\n");
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_type_multiple_args() {
        let mut runtime = Runtime::new();

        let result = builtin_type(&["cd".to_string(), "pwd".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("cd is a shell builtin"));
        assert!(result.stdout().contains("pwd is a shell builtin"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_mixed_args() {
        let mut runtime = Runtime::new();

        // Set up different command types
        runtime.set_alias("ll".to_string(), "ls -la".to_string());
        let func = FunctionDef {
            name: "myfunc".to_string(),
            params: vec![],
            body: vec![],
        };
        runtime.define_function(func);

        let result = builtin_type(
            &["cd".to_string(), "ll".to_string(), "myfunc".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert!(result.stdout().contains("cd is a shell builtin"));
        assert!(result.stdout().contains("ll is aliased to 'ls -la'"));
        assert!(result.stdout().contains("myfunc is a function"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_priority_builtin_over_alias() {
        let mut runtime = Runtime::new();

        // Try to create an alias with the same name as a builtin
        runtime.set_alias("cd".to_string(), "echo fake cd".to_string());

        // type should report cd as a builtin (builtins have priority)
        let result = builtin_type(&["cd".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "cd is a shell builtin\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_type_no_args() {
        let mut runtime = Runtime::new();

        let result = builtin_type(&[], &mut runtime);
        assert!(result.is_err());
    }
}
