use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct LengthOptions {
    quiet: bool,
    strings: Vec<String>,
}

impl LengthOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut quiet = false;
        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "-q" | "--quiet" => quiet = true,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string length: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        Ok(LengthOptions {
            quiet,
            strings: args[i..].to_vec(),
        })
    }
}

pub fn run_length(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let opts = match LengthOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    let inputs: Vec<String> = if opts.strings.is_empty() {
        let data = stdin.map(|d| d.to_vec()).unwrap_or_else(|| {
            let mut buf = Vec::new();
            use std::io::Read;
            std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
            buf
        });
        let text = String::from_utf8_lossy(&data);
        text.lines().map(|l| l.to_string()).collect()
    } else {
        opts.strings.clone()
    };

    // Exit code: 0 if any string is non-empty, 1 if all are empty
    let any_nonempty = inputs.iter().any(|s| !s.is_empty());

    let mut output = String::new();
    if !opts.quiet {
        for s in &inputs {
            // char count (not byte count) to handle Unicode correctly
            output.push_str(&s.chars().count().to_string());
            output.push('\n');
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: if any_nonempty { 0 } else { 1 },
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
    fn test_string_length_basic() {
        let mut rt = make_runtime();
        let result = run_length(&args(&["hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "5\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_length_multiple() {
        let mut rt = make_runtime();
        let result = run_length(&args(&["hi", "world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "2\n5\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_length_empty() {
        let mut rt = make_runtime();
        let result = run_length(&args(&[""]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "0\n");
        } else {
            panic!("expected text output");
        }
        // All empty → exit code 1
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_length_quiet() {
        let mut rt = make_runtime();
        let result = run_length(&args(&["-q", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_length_quiet_empty() {
        let mut rt = make_runtime();
        let result = run_length(&args(&["-q", ""]), &mut rt, None).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_length_unicode() {
        let mut rt = make_runtime();
        // "café" has 4 chars, 5 bytes (é is 2-byte UTF-8)
        let result = run_length(&args(&["café"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "4\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_length_stdin() {
        let mut rt = make_runtime();
        let result = run_length(&args(&[]), &mut rt, Some(b"hello\nworld")).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "5\n5\n");
        } else {
            panic!("expected text output");
        }
    }
}
