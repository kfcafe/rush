use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::Result;

/// Implement the `count` builtin (fish-style).
///
/// Usage:
///   count ARGS...   — print the number of arguments
///   count $array    — print the number of elements (pass expanded array words)
///
/// Exit status:
///   0 if count > 0
///   1 if count == 0
pub fn builtin_count(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let n = args.len();
    let exit_code = if n > 0 { 0 } else { 1 };
    Ok(ExecutionResult {
        output: crate::executor::Output::Text(format!("{}\n", n)),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_count_zero() {
        let mut rt = Runtime::new();
        let result = builtin_count(&[], &mut rt).unwrap();
        assert_eq!(result.stdout(), "0\n");
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_count_one() {
        let mut rt = Runtime::new();
        let result = builtin_count(&args(&["hello"]), &mut rt).unwrap();
        assert_eq!(result.stdout(), "1\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_count_many() {
        let mut rt = Runtime::new();
        let result = builtin_count(&args(&["a", "b", "c", "d", "e"]), &mut rt).unwrap();
        assert_eq!(result.stdout(), "5\n");
        assert_eq!(result.exit_code, 0);
    }
}
