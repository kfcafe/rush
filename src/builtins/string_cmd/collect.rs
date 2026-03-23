use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

/// `string collect` — collect strings into a single output without word splitting.
/// Each argument (or stdin line) is emitted as-is with a trailing newline.
/// The purpose is to prevent the shell from word-splitting the result when captured.
pub fn run_collect(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let inputs: Vec<String> = if args.is_empty() {
        let data = stdin.map(|d| d.to_vec()).unwrap_or_else(|| {
            let mut buf = Vec::new();
            use std::io::Read;
            std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
            buf
        });
        let text = String::from_utf8_lossy(&data);
        text.lines().map(|l| l.to_string()).collect()
    } else {
        args.to_vec()
    };

    let mut output = String::new();
    for s in &inputs {
        output.push_str(s);
        output.push('\n');
    }

    let exit_code = if inputs.is_empty() { 1 } else { 0 };

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
    fn test_string_collect_basic() {
        let mut rt = make_runtime();
        let result = run_collect(&args(&["hello", "world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\nworld\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_collect_single() {
        let mut rt = make_runtime();
        let result = run_collect(&args(&["only one"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "only one\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_collect_stdin() {
        let mut rt = make_runtime();
        let result = run_collect(&args(&[]), &mut rt, Some(b"foo\nbar\nbaz")).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo\nbar\nbaz\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_collect_empty_no_args() {
        let mut rt = make_runtime();
        let result = run_collect(&args(&[]), &mut rt, Some(b"")).unwrap();
        assert_eq!(result.exit_code, 1);
    }
}
