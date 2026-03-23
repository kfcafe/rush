use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `contains` builtin (fish-style).
///
/// Usage:
///   contains KEY VALUES...          — exit 0 if KEY is in VALUES, 1 otherwise
///   contains -i KEY VALUES...       — also print the 1-based index of the match
///
/// Examples:
///   if contains "$x" a b c; then echo found; fi
///   contains -i needle foo bar needle baz   # prints "3"
pub fn builtin_contains(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("contains: missing key argument"));
    }

    // Check for -i flag
    let (print_index, rest) = if args[0] == "-i" {
        if args.len() < 2 {
            return Err(anyhow!("contains: -i requires a key argument"));
        }
        (true, &args[1..])
    } else {
        (false, &args[..])
    };

    // rest[0] is KEY, rest[1..] are VALUES
    let key = &rest[0];
    let values = &rest[1..];

    for (i, val) in values.iter().enumerate() {
        if val == key {
            let output = if print_index {
                format!("{}\n", i + 1) // 1-based index
            } else {
                String::new()
            };
            return Ok(ExecutionResult::success(output));
        }
    }

    // Key not found
    Ok(ExecutionResult {
        output: crate::executor::Output::Text(String::new()),
        stderr: String::new(),
        exit_code: 1,
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
    fn test_contains_found() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&args(&["needle", "foo", "needle", "bar"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_contains_not_found() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&args(&["missing", "foo", "bar"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_contains_empty_values() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&args(&["x"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_contains_index_flag() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&args(&["-i", "c", "a", "b", "c", "d"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "3\n"); // 1-based
    }

    #[test]
    fn test_contains_index_not_found() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&args(&["-i", "z", "a", "b", "c"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_contains_no_args_error() {
        let mut rt = Runtime::new();
        let result = builtin_contains(&[], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn test_contains_exact_match() {
        let mut rt = Runtime::new();
        // Should not match substrings — only exact values
        let result = builtin_contains(&args(&["fo", "foo", "bar"]), &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }
}
