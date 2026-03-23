use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `shift` builtin command
///
/// Usage:
/// - `shift` - Shift positional parameters left by 1 (remove $1)
/// - `shift N` - Shift positional parameters left by N
///
/// The shift builtin removes the first N positional parameters,
/// shifting all remaining parameters down. Updates $#, $@, and $*.
pub fn builtin_shift(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Parse the shift count (default is 1)
    let shift_count = if args.is_empty() {
        1
    } else if args.len() == 1 {
        // Parse the argument as a number
        args[0]
            .parse::<usize>()
            .map_err(|_| anyhow!("shift: {}: numeric argument required", args[0]))?
    } else {
        return Err(anyhow!("shift: too many arguments"));
    };

    // Perform the shift operation
    runtime.shift_params(shift_count)?;

    Ok(ExecutionResult::success(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_shift_basic() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec![
            "arg1".to_string(),
            "arg2".to_string(),
            "arg3".to_string(),
        ]);

        // Verify initial state
        assert_eq!(runtime.get_variable("1"), Some("arg1".to_string()));
        assert_eq!(runtime.get_variable("2"), Some("arg2".to_string()));
        assert_eq!(runtime.get_variable("3"), Some("arg3".to_string()));
        assert_eq!(runtime.get_variable("#"), Some("3".to_string()));

        // Shift by 1 (default)
        let result = builtin_shift(&[], &mut runtime);
        assert!(result.is_ok());

        // Verify parameters shifted
        assert_eq!(runtime.get_variable("1"), Some("arg2".to_string()));
        assert_eq!(runtime.get_variable("2"), Some("arg3".to_string()));
        assert_eq!(runtime.get_variable("3"), None);
        assert_eq!(runtime.get_variable("#"), Some("2".to_string()));
        assert_eq!(runtime.get_variable("@"), Some("arg2 arg3".to_string()));
        assert_eq!(runtime.get_variable("*"), Some("arg2 arg3".to_string()));
    }

    #[test]
    fn test_shift_multiple() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ]);

        // Shift by 2
        let result = builtin_shift(&["2".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Verify parameters shifted by 2
        assert_eq!(runtime.get_variable("1"), Some("c".to_string()));
        assert_eq!(runtime.get_variable("2"), Some("d".to_string()));
        assert_eq!(runtime.get_variable("3"), Some("e".to_string()));
        assert_eq!(runtime.get_variable("4"), None);
        assert_eq!(runtime.get_variable("#"), Some("3".to_string()));
    }

    #[test]
    fn test_shift_all() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["one".to_string(), "two".to_string()]);

        // Shift all parameters
        let result = builtin_shift(&["2".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Verify all parameters gone
        assert_eq!(runtime.get_variable("1"), None);
        assert_eq!(runtime.get_variable("2"), None);
        assert_eq!(runtime.get_variable("#"), Some("0".to_string()));
        assert_eq!(runtime.get_variable("@"), Some("".to_string()));
        assert_eq!(runtime.get_variable("*"), Some("".to_string()));
    }

    #[test]
    fn test_shift_error_too_many() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["arg1".to_string(), "arg2".to_string()]);

        // Try to shift more than available
        let result = builtin_shift(&["3".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds"));

        // Verify parameters unchanged
        assert_eq!(runtime.get_variable("#"), Some("2".to_string()));
    }

    #[test]
    fn test_shift_error_invalid_count() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["arg1".to_string()]);

        // Try with non-numeric argument
        let result = builtin_shift(&["abc".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("numeric argument required"));
    }

    #[test]
    fn test_shift_error_too_many_args() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["arg1".to_string()]);

        // Try with too many arguments
        let result = builtin_shift(&["1".to_string(), "2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too many arguments"));
    }

    #[test]
    fn test_shift_zero() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["arg1".to_string(), "arg2".to_string()]);

        // Shift by 0 (no-op)
        let result = builtin_shift(&["0".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Verify parameters unchanged
        assert_eq!(runtime.get_variable("1"), Some("arg1".to_string()));
        assert_eq!(runtime.get_variable("2"), Some("arg2".to_string()));
        assert_eq!(runtime.get_variable("#"), Some("2".to_string()));
    }

    #[test]
    fn test_shift_empty_params() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec![]);

        // Shift with no parameters should error
        let result = builtin_shift(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds"));
    }

    #[test]
    fn test_shift_in_loop() {
        let mut runtime = Runtime::new();
        runtime.set_positional_params(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        // Simulate processing all args in a loop
        let mut processed = Vec::new();

        while runtime.param_count() > 0 {
            if let Some(arg) = runtime.get_variable("1") {
                processed.push(arg);
            }
            let _ = builtin_shift(&[], &mut runtime);
        }

        assert_eq!(processed, vec!["a", "b", "c"]);
        assert_eq!(runtime.get_variable("#"), Some("0".to_string()));
    }
}
