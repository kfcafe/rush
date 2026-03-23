use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

fn get_inputs(args: &[String], stdin: Option<&[u8]>) -> Vec<String> {
    if args.is_empty() {
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
    }
}

pub fn run_upper(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let inputs = get_inputs(args, stdin);
    let mut output = String::new();
    for s in &inputs {
        output.push_str(&s.to_uppercase());
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

pub fn run_lower(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let inputs = get_inputs(args, stdin);
    let mut output = String::new();
    for s in &inputs {
        output.push_str(&s.to_lowercase());
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
    fn test_string_upper_basic() {
        let mut rt = make_runtime();
        let result = run_upper(&args(&["hello", "world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "HELLO\nWORLD\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_upper_mixed_case() {
        let mut rt = make_runtime();
        let result = run_upper(&args(&["Hello World"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "HELLO WORLD\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_upper_stdin() {
        let mut rt = make_runtime();
        let result = run_upper(&args(&[]), &mut rt, Some(b"foo\nbar")).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "FOO\nBAR\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_lower_basic() {
        let mut rt = make_runtime();
        let result = run_lower(&args(&["HELLO", "WORLD"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\nworld\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_lower_mixed_case() {
        let mut rt = make_runtime();
        let result = run_lower(&args(&["Hello World"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello world\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_lower_stdin() {
        let mut rt = make_runtime();
        let result = run_lower(&args(&[]), &mut rt, Some(b"FOO\nBAR")).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo\nbar\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_upper_already_upper() {
        let mut rt = make_runtime();
        let result = run_upper(&args(&["ALREADY"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "ALREADY\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_lower_already_lower() {
        let mut rt = make_runtime();
        let result = run_lower(&args(&["already"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "already\n");
        } else {
            panic!("expected text output");
        }
    }
}
