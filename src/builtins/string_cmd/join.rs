use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

pub fn run_join(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>, null_sep: bool) -> Result<ExecutionResult> {
    // join: string join SEP [STRING...]
    // join0: string join0 [STRING...]  (joins with NUL, no sep arg)
    let (separator, strings_args) = if null_sep {
        ("\0".to_string(), args.to_vec())
    } else {
        if args.is_empty() {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: "string join: missing separator\n".to_string(),
                exit_code: 1,
                error: None,
            });
        }
        (args[0].clone(), args[1..].to_vec())
    };

    let inputs: Vec<String> = if strings_args.is_empty() {
        // Read from stdin
        let data = stdin.map(|d| d.to_vec()).unwrap_or_else(|| {
            let mut buf = Vec::new();
            use std::io::Read;
            std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
            buf
        });
        let text = String::from_utf8_lossy(&data);
        text.lines().map(|l| l.to_string()).collect()
    } else {
        strings_args
    };

    let joined = inputs.join(&separator);

    // join0 output ends with NUL; join output ends with newline
    let mut output = joined;
    if null_sep {
        output.push('\0');
    } else {
        output.push('\n');
    }

    let exit_code = if !inputs.is_empty() { 0 } else { 1 };

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_string_join_basic() {
        let mut rt = make_runtime();
        let result = run_join(&args(&[",", "a", "b", "c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a,b,c\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_join_single() {
        let mut rt = make_runtime();
        let result = run_join(&args(&["-", "only"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "only\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_join_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"hello\nworld";
        let result = run_join(&args(&[" "]), &mut rt, Some(stdin_data), false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello world\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_join0() {
        let mut rt = make_runtime();
        let result = run_join(&args(&["a", "b", "c"]), &mut rt, None, true).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\0b\0c\0");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_join_newline_sep() {
        let mut rt = make_runtime();
        let result = run_join(&args(&["\n", "a", "b", "c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb\nc\n");
        } else {
            panic!("expected text output");
        }
    }
}
