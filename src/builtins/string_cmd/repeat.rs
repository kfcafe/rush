use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct RepeatOptions {
    count: usize,
    max: Option<usize>,
    strings: Vec<String>,
}

impl RepeatOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut count: Option<usize> = None;
        let mut max: Option<usize> = None;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "-n" | "--count" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string repeat: -n requires an argument")
                    })?;
                    count = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string repeat: invalid count '{}'", val)
                    })?);
                }
                "-m" | "--max" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string repeat: -m requires an argument")
                    })?;
                    max = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string repeat: invalid max value '{}'", val)
                    })?);
                }
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--count=") => {
                    let val = &a["--count=".len()..];
                    count = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string repeat: invalid count '{}'", val)
                    })?);
                }
                a if a.starts_with("--max=") => {
                    let val = &a["--max=".len()..];
                    max = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string repeat: invalid max value '{}'", val)
                    })?);
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string repeat: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let count =
            count.ok_or_else(|| anyhow::anyhow!("string repeat: -n/--count is required"))?;
        let strings = args[i..].to_vec();

        Ok(RepeatOptions {
            count,
            max,
            strings,
        })
    }
}

fn repeat_string(s: &str, opts: &RepeatOptions) -> String {
    if opts.count == 0 {
        return String::new();
    }
    let repeated = s.repeat(opts.count);
    match opts.max {
        Some(max) => {
            // Truncate to max chars (by char count, not bytes)
            repeated.chars().take(max).collect()
        }
        None => repeated,
    }
}

pub fn run_repeat(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match RepeatOptions::parse(args) {
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

    let mut output = String::new();
    let mut any_output = false;

    for input in &inputs {
        let repeated = repeat_string(input, &opts);
        output.push_str(&repeated);
        output.push('\n');
        any_output = true;
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
    fn test_string_repeat_basic() {
        let mut rt = make_runtime();
        let result = run_repeat(&args(&["-n", "3", "ab"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "ababab\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_repeat_zero() {
        let mut rt = make_runtime();
        let result = run_repeat(&args(&["-n", "0", "ab"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_repeat_max() {
        let mut rt = make_runtime();
        let result = run_repeat(&args(&["-n", "5", "-m", "7", "ab"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "abababa\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_repeat_multiple_strings() {
        let mut rt = make_runtime();
        let result = run_repeat(&args(&["-n", "2", "x", "y"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "xx\nyy\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_repeat_missing_count() {
        let mut rt = make_runtime();
        let result = run_repeat(&args(&["hello"]), &mut rt, None).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_repeat_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"hi";
        let result = run_repeat(&args(&["-n", "3"]), &mut rt, Some(stdin_data)).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hihihi\n");
        } else {
            panic!("expected text output");
        }
    }
}
