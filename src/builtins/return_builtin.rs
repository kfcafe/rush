use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Special error type for return from functions
/// This is used to signal early return from a function with a specific exit code
#[derive(Debug)]
pub struct ReturnSignal {
    pub exit_code: i32,
}

impl std::fmt::Display for ReturnSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "return {}", self.exit_code)
    }
}

impl std::error::Error for ReturnSignal {}

/// Implementation of the `return` builtin
///
/// Usage:
///   return [N]
///
/// Exit from a function with exit code N (default 0).
/// Can only be used inside functions or sourced scripts.
///
/// Examples:
///   return        # Exit with code 0
///   return 1      # Exit with code 1
///   return $?     # Exit with last command's exit code
pub fn builtin_return(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Check if we're in a function context
    if !runtime.in_function_context() {
        return Err(anyhow!(
            "return: can only 'return' from a function or sourced script"
        ));
    }

    // Parse exit code argument
    let exit_code = if args.is_empty() {
        0
    } else if args.len() == 1 {
        args[0].parse::<i32>().unwrap_or_else(|_| {
            eprintln!("return: {}: numeric argument required", args[0]);
            2 // bash returns 2 for invalid numeric argument
        })
    } else {
        return Err(anyhow!("return: too many arguments"));
    };

    // Throw a special error that will be caught by the function executor
    Err(anyhow::Error::new(ReturnSignal { exit_code }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_return_outside_function() {
        let mut runtime = Runtime::new();
        let result = builtin_return(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("can only 'return' from a function"));
    }

    #[test]
    fn test_return_with_no_args() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        let result = builtin_return(&[], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<ReturnSignal>().is_some());
        assert_eq!(err.downcast_ref::<ReturnSignal>().unwrap().exit_code, 0);
    }

    #[test]
    fn test_return_with_exit_code() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        let result = builtin_return(&["42".to_string()], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<ReturnSignal>().is_some());
        assert_eq!(err.downcast_ref::<ReturnSignal>().unwrap().exit_code, 42);
    }

    #[test]
    fn test_return_with_invalid_exit_code() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        let result = builtin_return(&["not_a_number".to_string()], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<ReturnSignal>().is_some());
        assert_eq!(err.downcast_ref::<ReturnSignal>().unwrap().exit_code, 2);
    }

    #[test]
    fn test_return_too_many_args() {
        let mut runtime = Runtime::new();
        runtime.enter_function_context();
        let result = builtin_return(&["1".to_string(), "2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too many arguments"));
    }
}
