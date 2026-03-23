use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Execute a builtin command, bypassing any function with the same name
///
/// This is useful when a user has defined a function with the same name as a builtin.
/// For example, if there's a function named `cd`, `builtin cd /tmp` will call the
/// actual cd builtin, not the function.
///
/// Returns an error if the specified command is not a builtin.
pub fn builtin_builtin(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("builtin: usage: builtin command [args...]"));
    }

    let builtin_name = &args[0];
    let builtin_args = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        vec![]
    };

    // Get the Builtins instance from runtime to check if the command is a builtin
    // Since we don't have direct access to the Builtins instance here,
    // we need to check if the command is a known builtin
    if !is_builtin(builtin_name) {
        return Err(anyhow!("{}: not a shell builtin", builtin_name));
    }

    // Execute the builtin by calling the appropriate function
    // We need to dispatch to the correct builtin manually
    match builtin_name.as_str() {
        "cd" => super::builtin_cd(&builtin_args, runtime),
        "pwd" => super::builtin_pwd(&builtin_args, runtime),
        "echo" => super::builtin_echo(&builtin_args, runtime),
        "exit" => super::exit_builtin::builtin_exit(&builtin_args, runtime),
        "export" => super::builtin_export(&builtin_args, runtime),
        "source" => super::builtin_source(&builtin_args, runtime),
        "cat" => super::cat::builtin_cat(&builtin_args, runtime),
        "find" => super::find::builtin_find(&builtin_args, runtime),
        "ls" => super::ls::builtin_ls(&builtin_args, runtime),
        "mkdir" => super::mkdir::builtin_mkdir(&builtin_args, runtime),
        "git" => super::builtin_git_external(&builtin_args, runtime),
        "grep" => super::grep::builtin_grep(&builtin_args, runtime),
        "undo" => super::undo::builtin_undo(&builtin_args, runtime),
        "jobs" => super::jobs::builtin_jobs(&builtin_args, runtime),
        "set" => super::set::builtin_set(&builtin_args, runtime),
        "alias" => super::alias::builtin_alias(&builtin_args, runtime),
        "unalias" => super::alias::builtin_unalias(&builtin_args, runtime),
        "test" => super::test::builtin_test(&builtin_args, runtime),
        "[" => super::test::builtin_bracket(&builtin_args, runtime),
        "help" => super::help::builtin_help(&builtin_args, runtime),
        "type" => super::type_builtin::builtin_type(&builtin_args, runtime),
        "true" => super::builtin_true(&builtin_args, runtime),
        "false" => super::builtin_false(&builtin_args, runtime),
        "unset" => super::unset::builtin_unset(&builtin_args, runtime),
        "printf" => super::printf::builtin_printf(&builtin_args, runtime),
        _ => Err(anyhow!("{}: not a shell builtin", builtin_name)),
    }
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
            | "git"
            | "grep"
            | "undo"
            | "jobs"
            | "set"
            | "alias"
            | "unalias"
            | "test"
            | "["
            | "help"
            | "type"
            | "true"
            | "false"
            | "unset"
            | "printf"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::FunctionDef;

    #[test]
    fn test_builtin_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_builtin(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_builtin_echo() {
        let mut runtime = Runtime::new();
        let args = vec!["echo".to_string(), "hello".to_string(), "world".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "hello world\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_pwd() {
        let mut runtime = Runtime::new();
        let args = vec!["pwd".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert!(!result.stdout().is_empty());
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_nonexistent() {
        let mut runtime = Runtime::new();
        let args = vec!["nonexistent".to_string()];
        let result = builtin_builtin(&args, &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a shell builtin"));
    }

    #[test]
    fn test_builtin_bypasses_function() {
        let mut runtime = Runtime::new();

        // Define a function named "echo" that would normally override the builtin
        let func = FunctionDef {
            name: "echo".to_string(),
            params: vec![],
            body: vec![],
        };
        runtime.define_function(func);

        // Verify the function exists
        assert!(runtime.get_function("echo").is_some());

        // Call builtin echo - should still execute the builtin, not the function
        let args = vec!["echo".to_string(), "test".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "test\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_cd() {
        let mut runtime = Runtime::new();
        let original_cwd = runtime.get_cwd().clone();

        // Try to cd to /tmp
        let args = vec!["cd".to_string(), "/tmp".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Verify the directory changed
        let new_cwd = runtime.get_cwd();
        assert_ne!(new_cwd, &original_cwd);
        assert!(new_cwd.to_string_lossy().contains("tmp"));
    }

    #[test]
    fn test_builtin_true() {
        let mut runtime = Runtime::new();
        let args = vec!["true".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_false() {
        let mut runtime = Runtime::new();
        let args = vec!["false".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_builtin_type() {
        let mut runtime = Runtime::new();
        let args = vec!["type".to_string(), "cd".to_string()];
        let result = builtin_builtin(&args, &mut runtime).unwrap();
        assert!(result.stdout().contains("cd is a shell builtin"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_with_cd_function_override() {
        let mut runtime = Runtime::new();

        // Define a function named "cd"
        let func = FunctionDef {
            name: "cd".to_string(),
            params: vec![],
            body: vec![],
        };
        runtime.define_function(func);

        // Even with a cd function defined, builtin cd should work
        let args = vec!["cd".to_string(), "/tmp".to_string()];
        let result = builtin_builtin(&args, &mut runtime);
        assert!(result.is_ok());
    }
}
