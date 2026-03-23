use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Special error type for continuing to next loop iteration
/// This is used to signal skipping to next iteration of for/while/until loops
#[derive(Debug)]
pub struct ContinueSignal {
    pub levels: usize,
    pub accumulated_stdout: String,
    pub accumulated_stderr: String,
}

impl std::fmt::Display for ContinueSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "continue {}", self.levels)
    }
}

impl std::error::Error for ContinueSignal {}

/// Implementation of the `continue` builtin
///
/// Usage:
///   continue [N]
///
/// Skip to next iteration of N enclosing for/while/until loops (default 1).
/// Can only be used inside loops.
///
/// Examples:
///   continue        # Skip to next iteration of innermost loop
///   continue 2      # Skip to next iteration of 2nd enclosing loop
pub fn builtin_continue(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Check if we're in a loop context
    if runtime.get_loop_depth() == 0 {
        return Err(anyhow!(
            "continue: only meaningful in a `for', `while', or `until' loop"
        ));
    }

    // Parse the number of levels to continue
    let levels = if args.is_empty() {
        1
    } else if args.len() == 1 {
        match args[0].parse::<usize>() {
            Ok(0) => {
                return Err(anyhow!("continue: loop count out of range"));
            }
            Ok(n) => n,
            Err(_) => {
                return Err(anyhow!("continue: {}: numeric argument required", args[0]));
            }
        }
    } else {
        return Err(anyhow!("continue: too many arguments"));
    };

    // Check if the number of levels is valid
    if levels > runtime.get_loop_depth() {
        return Err(anyhow!("continue: loop count out of range"));
    }

    // Throw a special error that will be caught by the loop executor
    Err(anyhow::Error::new(ContinueSignal {
        levels,
        accumulated_stdout: String::new(),
        accumulated_stderr: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continue_outside_loop() {
        let mut runtime = Runtime::new();
        let result = builtin_continue(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("only meaningful in a"));
    }

    #[test]
    fn test_continue_with_no_args() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_continue(&[], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<ContinueSignal>().is_some());
        assert_eq!(err.downcast_ref::<ContinueSignal>().unwrap().levels, 1);
        runtime.exit_loop();
    }

    #[test]
    fn test_continue_with_level() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        runtime.enter_loop();
        let result = builtin_continue(&["2".to_string()], &mut runtime);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<ContinueSignal>().is_some());
        assert_eq!(err.downcast_ref::<ContinueSignal>().unwrap().levels, 2);
        runtime.exit_loop();
        runtime.exit_loop();
    }

    #[test]
    fn test_continue_with_zero() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_continue(&["0".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loop count out of range"));
        runtime.exit_loop();
    }

    #[test]
    fn test_continue_with_invalid_number() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_continue(&["not_a_number".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("numeric argument required"));
        runtime.exit_loop();
    }

    #[test]
    fn test_continue_too_many_args() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_continue(&["1".to_string(), "2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("too many arguments"));
        runtime.exit_loop();
    }

    #[test]
    fn test_continue_exceeds_loop_depth() {
        let mut runtime = Runtime::new();
        runtime.enter_loop();
        let result = builtin_continue(&["2".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("loop count out of range"));
        runtime.exit_loop();
    }
}
