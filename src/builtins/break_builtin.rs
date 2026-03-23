use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Special error type for breaking out of loops
/// This is used to signal early exit from for/while/until loops
#[derive(Debug)]
pub struct BreakSignal {
    pub levels: usize,
    pub accumulated_stdout: String,
    pub accumulated_stderr: String,
}

impl std::fmt::Display for BreakSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "break {}", self.levels)
    }
}

impl std::error::Error for BreakSignal {}

/// Implementation of the `break` builtin
///
/// Usage:
///   break [N]
///
/// Exit from N enclosing for/while/until loops (default 1).
/// Can only be used inside loops.
///
/// Examples:
///   break        # Exit from innermost loop
///   break 2      # Exit from 2 nested loops
pub fn builtin_break(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Check if we're in a loop context
    if runtime.get_loop_depth() == 0 {
        return Err(anyhow!(
            "break: only meaningful in a `for', `while', or `until' loop"
        ));
    }

    // Parse the number of levels to break
    let levels = if args.is_empty() {
        1
    } else if args.len() == 1 {
        match args[0].parse::<usize>() {
            Ok(0) => {
                return Err(anyhow!("break: loop count out of range"));
            }
            Ok(n) => n,
            Err(_) => {
                return Err(anyhow!("break: {}: numeric argument required", args[0]));
            }
        }
    } else {
        return Err(anyhow!("break: too many arguments"));
    };

    // Check if the number of levels is valid
    if levels > runtime.get_loop_depth() {
        return Err(anyhow!("break: loop count out of range"));
    }

    // Throw a special error that will be caught by the loop executor
    Err(anyhow::Error::new(BreakSignal {
        levels,
        accumulated_stdout: String::new(),
        accumulated_stderr: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_break_outside_loop() {
        let mut runtime = Runtime::new();
        let result = builtin_break(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("only meaningful in a"));
    }

    #[test]
    fn test_break_with_no_args() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_break(&[], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<BreakSignal>().is_some());
        assert_eq!(err.downcast_ref::<BreakSignal>().unwrap().levels, 1);
        runtime.exit_loop();
    }

    #[test]
    fn test_break_with_level() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        runtime.enter_loop();
        let result = builtin_break(&["2".to_string()], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<BreakSignal>().is_some());
        assert_eq!(err.downcast_ref::<BreakSignal>().unwrap().levels, 2);
        runtime.exit_loop();
        runtime.exit_loop();
    }

    #[test]
    fn test_break_with_zero() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_break(&["0".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loop count out of range"));
        runtime.exit_loop();
    }

    #[test]
    fn test_break_with_invalid_number() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_break(&["not_a_number".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("numeric argument required"));
        runtime.exit_loop();
    }

    #[test]
    fn test_break_too_many_args() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_break(&["1".to_string(), "2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too many arguments"));
        runtime.exit_loop();
    }

    #[test]
    fn test_break_exceeds_loop_depth() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_break(&["2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loop count out of range"));
        runtime.exit_loop();
    }
}
