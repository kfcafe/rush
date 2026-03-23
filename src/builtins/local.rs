use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `local` builtin command
///
/// Usage:
/// - `local var=value` - Create a function-scoped variable with assignment
/// - `local var` - Declare a function-scoped variable without assignment
/// - `local a=1 b=2 c=3` - Multiple declarations
///
/// The local builtin creates variables that are scoped to the current function.
/// These variables shadow global variables with the same name and are
/// automatically cleaned up when the function exits.
///
/// Error conditions:
/// - Called outside a function (not in a function scope)
/// - Invalid variable name or syntax
pub fn builtin_local(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Check if we're inside a function
    if !runtime.in_function_context() {
        return Err(anyhow!("local: can only be used in a function"));
    }

    // If no arguments, that's an error (unlike export which can list variables)
    if args.is_empty() {
        return Err(anyhow!("local: usage: local var[=value] ..."));
    }

    // Process each argument
    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            // Variable assignment: local var=value
            validate_variable_name(name)?;
            runtime.set_variable(name.to_string(), value.to_string());
        } else {
            // Variable declaration without assignment: local var
            validate_variable_name(arg)?;
            // Declare with empty value
            runtime.set_variable(arg.to_string(), String::new());
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// Validate that a variable name is valid
/// Valid names: start with letter or underscore, contain only alphanumeric and underscore
fn validate_variable_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("local: invalid variable name: empty string"));
    }

    // First character must be letter or underscore
    let first = name.chars().next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return Err(anyhow!(
            "local: invalid variable name: '{}' (must start with letter or underscore)",
            name
        ));
    }

    // Rest must be alphanumeric or underscore
    for ch in name.chars() {
        if !ch.is_alphanumeric() && ch != '_' {
            return Err(anyhow!(
                "local: invalid variable name: '{}' (contains invalid character '{}')",
                name,
                ch
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_local_requires_function_scope() {
        let mut runtime = Runtime::new();

        // Calling local outside a function should fail
        let result = builtin_local(&["x=1".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("can only be used in a function"));
    }

    #[test]
    fn test_local_with_assignment() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context(); // Simulate being in a function
        runtime.push_scope();

        // Create a local variable
        let result = builtin_local(&["x=hello".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Variable should exist in current scope
        assert_eq!(runtime.get_variable("x"), Some("hello".to_string()));

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_without_assignment() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        // Declare a local variable without value
        let result = builtin_local(&["x".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Variable should exist but be empty
        assert_eq!(runtime.get_variable("x"), Some(String::new()));

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_multiple_declarations() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        // Multiple declarations at once
        let result = builtin_local(
            &["a=1".to_string(), "b=2".to_string(), "c=3".to_string()],
            &mut runtime,
        );
        assert!(result.is_ok());

        assert_eq!(runtime.get_variable("a"), Some("1".to_string()));
        assert_eq!(runtime.get_variable("b"), Some("2".to_string()));
        assert_eq!(runtime.get_variable("c"), Some("3".to_string()));

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_shadows_global() {
        let mut runtime = Runtime::new();

        // Set a global variable
        runtime.set_variable("x".to_string(), "global".to_string());
        assert_eq!(runtime.get_variable("x"), Some("global".to_string()));

        // Enter function scope
        runtime.enter_function_context();
        runtime.push_scope();

        // Create local variable with same name
        let result = builtin_local(&["x=local".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Should see local value, not global
        assert_eq!(runtime.get_variable("x"), Some("local".to_string()));

        // Exit function scope
        runtime.pop_scope();
        runtime.exit_function_context();

        // Should see global value again
        assert_eq!(runtime.get_variable("x"), Some("global".to_string()));
    }

    #[test]
    fn test_local_cleanup_on_scope_exit() {
        let mut runtime = Runtime::new();

        runtime.enter_function_context();
        runtime.push_scope();

        // Create local variables
        builtin_local(&["a=1".to_string(), "b=2".to_string()], &mut runtime).unwrap();

        assert_eq!(runtime.get_variable("a"), Some("1".to_string()));
        assert_eq!(runtime.get_variable("b"), Some("2".to_string()));

        runtime.pop_scope();
        runtime.exit_function_context();

        // Variables should not exist anymore
        assert_eq!(runtime.get_variable("a"), None);
        assert_eq!(runtime.get_variable("b"), None);
    }

    #[test]
    fn test_local_no_args_error() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        let result = builtin_local(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_invalid_variable_names() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        // Name starting with number
        let result = builtin_local(&["1var=value".to_string()], &mut runtime);
        assert!(result.is_err());

        // Name with invalid characters
        let result = builtin_local(&["var-name=value".to_string()], &mut runtime);
        assert!(result.is_err());

        // Name with space
        let result = builtin_local(&["var name=value".to_string()], &mut runtime);
        assert!(result.is_err());

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_valid_variable_names() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        // Valid names
        let valid_names = vec!["x=1", "var=2", "_private=3", "MY_VAR=4", "var123=5", "_=6"];

        for name in valid_names {
            let result = builtin_local(&[name.to_string()], &mut runtime);
            assert!(
                result.is_ok(),
                "Expected '{}' to be valid, but got error: {:?}",
                name,
                result
            );
        }

        runtime.pop_scope();
        runtime.exit_function_context();
    }

    #[test]
    fn test_local_nested_scopes() {
        let mut runtime = Runtime::new();

        // Outer function
        runtime.enter_function_context();
        runtime.push_scope();
        builtin_local(&["x=outer".to_string()], &mut runtime).unwrap();
        assert_eq!(runtime.get_variable("x"), Some("outer".to_string()));

        // Inner function
        runtime.enter_function_context();
        runtime.push_scope();
        builtin_local(&["x=inner".to_string()], &mut runtime).unwrap();
        assert_eq!(runtime.get_variable("x"), Some("inner".to_string()));

        // Exit inner function
        runtime.pop_scope();
        runtime.exit_function_context();
        assert_eq!(runtime.get_variable("x"), Some("outer".to_string()));

        // Exit outer function
        runtime.pop_scope();
        runtime.exit_function_context();
        assert_eq!(runtime.get_variable("x"), None);
    }

    #[test]
    fn test_local_mixed_declarations() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        runtime.push_scope();

        // Mix of assigned and unassigned
        let result = builtin_local(
            &[
                "a=1".to_string(),
                "b".to_string(),
                "c=3".to_string(),
                "d".to_string(),
            ],
            &mut runtime,
        );
        assert!(result.is_ok());

        assert_eq!(runtime.get_variable("a"), Some("1".to_string()));
        assert_eq!(runtime.get_variable("b"), Some(String::new()));
        assert_eq!(runtime.get_variable("c"), Some("3".to_string()));
        assert_eq!(runtime.get_variable("d"), Some(String::new()));

        runtime.pop_scope();
        runtime.exit_function_context();
    }
}
