use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct SplitOptions {
    separator: String,
    max: Option<usize>,
    right: bool,
    no_empty: bool,
    strings: Vec<String>,
    null_sep: bool,
}

impl SplitOptions {
    fn parse(args: &[String], null_sep: bool) -> Result<Self> {
        let mut max: Option<usize> = None;
        let mut right = false;
        let mut no_empty = false;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "-m" | "--max" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string split: -m requires an argument")
                    })?;
                    max = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string split: invalid max value '{}'", val)
                    })?);
                }
                "-r" | "--right" => right = true,
                "-n" | "--no-empty" => no_empty = true,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--max=") => {
                    let val = &a["--max=".len()..];
                    max = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string split: invalid max value '{}'", val)
                    })?);
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string split: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        // For split0, no separator argument
        let separator = if null_sep {
            "\0".to_string()
        } else {
            let sep = args.get(i).ok_or_else(|| {
                anyhow::anyhow!("string split: missing separator")
            })?;
            i += 1;
            sep.clone()
        };

        let strings = args[i..].to_vec();

        Ok(SplitOptions {
            separator,
            max,
            right,
            no_empty,
            strings,
            null_sep,
        })
    }
}

fn split_string(s: &str, opts: &SplitOptions) -> Vec<String> {
    let sep = &opts.separator;

    let parts: Vec<String> = if opts.right {
        // Split from right: rsplitn gives parts in reverse order
        let max_n = opts.max.map(|m| m + 1).unwrap_or(usize::MAX);
        let mut parts: Vec<String> = s.rsplitn(max_n, sep.as_str()).map(|p| p.to_string()).collect();
        parts.reverse();
        parts
    } else {
        let max_n = opts.max.map(|m| m + 1).unwrap_or(usize::MAX);
        s.splitn(max_n, sep.as_str()).map(|p| p.to_string()).collect()
    };

    if opts.no_empty {
        parts.into_iter().filter(|p| !p.is_empty()).collect()
    } else {
        parts
    }
}

pub fn run_split(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>, null_sep: bool) -> Result<ExecutionResult> {
    let opts = match SplitOptions::parse(args, null_sep) {
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
        // Read from stdin
        let data = stdin.map(|d| d.to_vec()).unwrap_or_else(|| {
            let mut buf = Vec::new();
            use std::io::Read;
            std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
            buf
        });
        // Split stdin lines into inputs
        let text = String::from_utf8_lossy(&data);
        // Each newline-separated line is treated as a separate input string
        text.lines().map(|l| l.to_string()).collect()
    } else {
        opts.strings.clone()
    };

    let mut output = String::new();
    let mut any_output = false;

    for input in &inputs {
        let parts = split_string(input, &opts);
        for part in parts {
            if opts.null_sep {
                output.push_str(&part);
                output.push('\0');
            } else {
                output.push_str(&part);
                output.push('\n');
            }
            any_output = true;
        }
    }

    let exit_code = if any_output { 0 } else { 1 };

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
    fn test_string_split_basic() {
        let mut rt = make_runtime();
        let result = run_split(&args(&[",", "a,b,c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb\nc\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_split_max() {
        let mut rt = make_runtime();
        let result = run_split(&args(&["-m", "1", ",", "a,b,c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb,c\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_split_right() {
        let mut rt = make_runtime();
        let result = run_split(&args(&["-r", "-m", "1", ",", "a,b,c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a,b\nc\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_split_no_empty() {
        let mut rt = make_runtime();
        let result = run_split(&args(&["-n", ",", "a,,b,,c"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb\nc\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_split_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"hello world";
        let result = run_split(&args(&[" "]), &mut rt, Some(stdin_data), false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\nworld\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_split_multiple_strings() {
        let mut rt = make_runtime();
        let result = run_split(&args(&[",", "a,b", "c,d"]), &mut rt, None, false).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb\nc\nd\n");
        } else {
            panic!("expected text output");
        }
    }
}
