use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `unset` builtin command
///
/// Usage:
/// - `unset var` - Remove variable
/// - `unset -v var` - Remove variable (explicit)
/// - `unset -f func` - Remove function
/// - `unset a b c` - Remove multiple variables
pub fn builtin_unset(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("unset: usage: unset [-v | -f] name [name ...]"));
    }

    let mut remove_functions = false;
    let mut start_idx = 0;

    // Parse flags
    if let Some(first) = args.first() {
        if first == "-f" {
            remove_functions = true;
            start_idx = 1;
        } else if first == "-v" {
            // Explicit variable removal (default behavior)
            start_idx = 1;
        }
    }

    // Check if there are any names to unset after parsing flags
    if start_idx >= args.len() {
        return Err(anyhow!("unset: usage: unset [-v | -f] name [name ...]"));
    }

    // Process each name
    for name in &args[start_idx..] {
        // Don't allow unsetting special variables like ? (readonly-like behavior)
        // This is future-proofing for readonly support
        if name == "?" {
            return Err(anyhow!("unset: {}: cannot unset", name));
        }

        // Check if variable is readonly
        if !remove_functions && runtime.is_readonly(name) {
            return Err(anyhow!("unset: {}: cannot unset: readonly variable", name));
        }

        if remove_functions {
            // Remove function - no error if it doesn't exist
            runtime.remove_function(name);
        } else {
            // Remove variable - no error if it doesn't exist
            runtime.remove_variable(name);
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::FunctionDef;
    use crate::parser::ast::{Argument, Command, Statement};
    use crate::runtime::Runtime;

    #[test]
    fn test_unset_variable() {
        let mut runtime = Runtime::new();
        runtime.set_variable("TEST_VAR".to_string(), "value".to_string());

        assert_eq!(runtime.get_variable("TEST_VAR"), Some("value".to_string()));

        let result = builtin_unset(&["TEST_VAR".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("TEST_VAR"), None);
    }

    #[test]
    fn test_unset_variable_explicit() {
        let mut runtime = Runtime::new();
        runtime.set_variable("TEST_VAR".to_string(), "value".to_string());

        let result = builtin_unset(&["-v".to_string(), "TEST_VAR".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("TEST_VAR"), None);
    }

    #[test]
    fn test_unset_multiple_variables() {
        let mut runtime = Runtime::new();
        runtime.set_variable("A".to_string(), "1".to_string());
        runtime.set_variable("B".to_string(), "2".to_string());
        runtime.set_variable("C".to_string(), "3".to_string());

        let result = builtin_unset(
            &["A".to_string(), "B".to_string(), "C".to_string()],
            &mut runtime,
        );
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("A"), None);
        assert_eq!(runtime.get_variable("B"), None);
        assert_eq!(runtime.get_variable("C"), None);
    }

    #[test]
    fn test_unset_nonexistent_variable() {
        let mut runtime = Runtime::new();

        // Should not error if variable doesn't exist
        let result = builtin_unset(&["NONEXISTENT".to_string()], &mut runtime);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unset_function() {
        let mut runtime = Runtime::new();

        // Define a function
        let func = FunctionDef {
            name: "myfunc".to_string(),
            params: vec![],
            body: vec![Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("hello".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            })],
        };
        runtime.define_function(func);

        assert!(runtime.get_function("myfunc").is_some());

        let result = builtin_unset(&["-f".to_string(), "myfunc".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(runtime.get_function("myfunc").is_none());
    }

    #[test]
    fn test_unset_nonexistent_function() {
        let mut runtime = Runtime::new();

        // Should not error if function doesn't exist
        let result = builtin_unset(&["-f".to_string(), "nonexistent".to_string()], &mut runtime);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unset_no_args() {
        let mut runtime = Runtime::new();

        let result = builtin_unset(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_unset_flag_only() {
        let mut runtime = Runtime::new();

        // -f flag without name should error
        let result = builtin_unset(&["-f".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));

        // -v flag without name should error
        let result = builtin_unset(&["-v".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_unset_special_variable_protected() {
        let mut runtime = Runtime::new();
        runtime.set_last_exit_code(42);

        // Should not be able to unset $?
        let result = builtin_unset(&["?".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot unset"));

        // $? should still be set
        assert_eq!(runtime.get_last_exit_code(), 42);
    }

    #[test]
    fn test_unset_in_function_scope() {
        let mut runtime = Runtime::new();

        // Set a global variable
        runtime.set_variable("GLOBAL".to_string(), "global_value".to_string());

        // Enter a function scope
        runtime.push_scope();
        runtime.set_variable("LOCAL".to_string(), "local_value".to_string());

        // Local variable should exist
        assert_eq!(
            runtime.get_variable("LOCAL"),
            Some("local_value".to_string())
        );

        // Unset local variable
        let result = builtin_unset(&["LOCAL".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("LOCAL"), None);

        // Global variable should still exist
        assert_eq!(
            runtime.get_variable("GLOBAL"),
            Some("global_value".to_string())
        );

        // Exit function scope
        runtime.pop_scope();

        // Global variable should still be accessible
        assert_eq!(
            runtime.get_variable("GLOBAL"),
            Some("global_value".to_string())
        );
    }

    #[test]
    fn test_unset_multiple_functions() {
        let mut runtime = Runtime::new();

        // Define multiple functions
        for name in &["func1", "func2", "func3"] {
            let func = FunctionDef {
                name: name.to_string(),
                params: vec![],
                body: vec![Statement::Command(Command {
                    name: "echo".to_string(),
                    args: vec![Argument::Literal(name.to_string())],
                    redirects: vec![],
                    prefix_env: vec![],
                })],
            };
            runtime.define_function(func);
        }

        // Unset all at once
        let result = builtin_unset(
            &[
                "-f".to_string(),
                "func1".to_string(),
                "func2".to_string(),
                "func3".to_string(),
            ],
            &mut runtime,
        );
        assert!(result.is_ok());

        // All should be removed
        assert!(runtime.get_function("func1").is_none());
        assert!(runtime.get_function("func2").is_none());
        assert!(runtime.get_function("func3").is_none());
    }
}
