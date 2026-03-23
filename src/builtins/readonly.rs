use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `readonly` builtin command
///
/// Usage:
/// - `readonly VAR=value` - Create readonly variable with value
/// - `readonly VAR` - Mark existing variable as readonly
/// - `readonly -p` - Print all readonly variables
/// - `readonly var1 var2=value` - Mark multiple variables readonly
///
/// POSIX Specification:
/// - Mark variables as read-only, preventing modification or unsetting
/// - Can set value and mark readonly in one command
/// - Can mark existing variable readonly
/// - `-p` option prints readonly variables in a format that can be re-input
/// - Exit code 0 on success, >0 on error
pub fn builtin_readonly(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Handle -p flag (print readonly variables)
    if args.len() == 1 && args[0] == "-p" {
        return print_readonly_vars(runtime);
    }

    // If no arguments, print readonly variables (same as -p)
    if args.is_empty() {
        return print_readonly_vars(runtime);
    }

    // Process each argument
    for arg in args {
        // Skip -p flag if mixed with other arguments
        if arg == "-p" {
            continue;
        }

        // Check if it's an assignment (VAR=value)
        if let Some(eq_pos) = arg.find('=') {
            let name = &arg[..eq_pos];
            let value = &arg[eq_pos + 1..];

            // Validate variable name
            if !is_valid_var_name(name) {
                return Err(anyhow!("readonly: `{}': not a valid identifier", name));
            }

            // Check if already readonly
            if runtime.is_readonly(name) {
                return Err(anyhow!("{}: readonly variable", name));
            }

            // Set the variable value
            runtime.set_variable(name.to_string(), value.to_string());

            // Mark as readonly
            runtime.mark_readonly(name.to_string());
        } else {
            // Just marking existing variable as readonly
            let name = arg;

            // Validate variable name
            if !is_valid_var_name(name) {
                return Err(anyhow!("readonly: `{}': not a valid identifier", name));
            }

            // Variable doesn't need to exist to be marked readonly
            // If it doesn't exist, we still mark it readonly for future assignments
            runtime.mark_readonly(name.to_string());
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// Print all readonly variables in a format that can be re-input
fn print_readonly_vars(runtime: &Runtime) -> Result<ExecutionResult> {
    let readonly_vars = runtime.get_readonly_vars();
    let mut output = String::new();

    for var_name in readonly_vars {
        // Get the variable value if it exists
        if let Some(value) = runtime.get_variable(&var_name) {
            // Format: readonly VAR='value'
            // Quote the value to handle spaces and special characters
            output.push_str(&format!("readonly {}='{}'\n", var_name, value));
        } else {
            // Variable is marked readonly but has no value
            output.push_str(&format!("readonly {}\n", var_name));
        }
    }

    Ok(ExecutionResult::success(output))
}

/// Validate variable name (must start with letter or underscore, contain only alphanumeric and underscore)
fn is_valid_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();

    // First character must be letter or underscore
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }
    }

    // Remaining characters must be alphanumeric or underscore
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_readonly_create_with_value() {
        let mut runtime = Runtime::new();

        let result = builtin_readonly(&["VAR=value".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Variable should be set
        assert_eq!(runtime.get_variable("VAR"), Some("value".to_string()));

        // Variable should be readonly
        assert!(runtime.is_readonly("VAR"));
    }

    #[test]
    fn test_readonly_mark_existing() {
        let mut runtime = Runtime::new();
        runtime.set_variable("VAR".to_string(), "value".to_string());

        let result = builtin_readonly(&["VAR".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Variable should still have its value
        assert_eq!(runtime.get_variable("VAR"), Some("value".to_string()));

        // Variable should be readonly
        assert!(runtime.is_readonly("VAR"));
    }

    #[test]
    fn test_readonly_mark_nonexistent() {
        let mut runtime = Runtime::new();

        let result = builtin_readonly(&["VAR".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Variable is marked readonly even though it doesn't exist yet
        assert!(runtime.is_readonly("VAR"));
    }

    #[test]
    fn test_readonly_cannot_reassign() {
        let mut runtime = Runtime::new();

        // Create readonly variable
        let result = builtin_readonly(&["VAR=value".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Try to reassign - should error
        let result = builtin_readonly(&["VAR=newvalue".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("readonly variable"));

        // Value should not have changed
        assert_eq!(runtime.get_variable("VAR"), Some("value".to_string()));
    }

    #[test]
    fn test_readonly_multiple_vars() {
        let mut runtime = Runtime::new();

        let result = builtin_readonly(
            &["A=1".to_string(), "B".to_string(), "C=3".to_string()],
            &mut runtime,
        );
        assert!(result.is_ok());

        assert!(runtime.is_readonly("A"));
        assert!(runtime.is_readonly("B"));
        assert!(runtime.is_readonly("C"));
        assert_eq!(runtime.get_variable("A"), Some("1".to_string()));
        assert_eq!(runtime.get_variable("C"), Some("3".to_string()));
    }

    #[test]
    fn test_readonly_print_p_flag() {
        let mut runtime = Runtime::new();

        // Create some readonly variables
        builtin_readonly(&["VAR1=value1".to_string()], &mut runtime).unwrap();
        builtin_readonly(&["VAR2=value2".to_string()], &mut runtime).unwrap();
        builtin_readonly(&["VAR3".to_string()], &mut runtime).unwrap();

        // Print with -p flag
        let result = builtin_readonly(&["-p".to_string()], &mut runtime);
        assert!(result.is_ok());

        let output = result.unwrap().stdout();
        assert!(output.contains("readonly VAR1='value1'"));
        assert!(output.contains("readonly VAR2='value2'"));
        assert!(output.contains("readonly VAR3"));
    }

    #[test]
    fn test_readonly_print_no_args() {
        let mut runtime = Runtime::new();

        // Create a readonly variable
        builtin_readonly(&["VAR=value".to_string()], &mut runtime).unwrap();

        // Print without arguments (should work like -p)
        let result = builtin_readonly(&[], &mut runtime);
        assert!(result.is_ok());

        let output = result.unwrap().stdout();
        assert!(output.contains("readonly VAR='value'"));
    }

    #[test]
    fn test_readonly_invalid_name() {
        let mut runtime = Runtime::new();

        // Invalid names
        let invalid_names = vec![
            "123VAR",   // starts with number
            "VAR-NAME", // contains hyphen
            "VAR NAME", // contains space
            "",         // empty
        ];

        for name in invalid_names {
            let result = builtin_readonly(&[name.to_string()], &mut runtime);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("not a valid identifier"));
        }
    }

    #[test]
    fn test_valid_var_names() {
        assert!(is_valid_var_name("VAR"));
        assert!(is_valid_var_name("_VAR"));
        assert!(is_valid_var_name("var"));
        assert!(is_valid_var_name("VAR123"));
        assert!(is_valid_var_name("_123"));
        assert!(is_valid_var_name("VAR_NAME"));

        assert!(!is_valid_var_name("123VAR"));
        assert!(!is_valid_var_name("VAR-NAME"));
        assert!(!is_valid_var_name(""));
    }
}
