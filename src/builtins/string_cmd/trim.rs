use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct TrimOptions {
    left: bool,
    right: bool,
    chars: Option<String>,
    strings: Vec<String>,
}

impl TrimOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut left = false;
        let mut right = false;
        let mut chars: Option<String> = None;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "-l" | "--left" => left = true,
                "-r" | "--right" => right = true,
                "-c" | "--chars" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string trim: -c requires an argument")
                    })?;
                    chars = Some(val.clone());
                }
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--chars=") => {
                    chars = Some(a["--chars=".len()..].to_string());
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string trim: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        // Default: trim both sides if neither -l nor -r given
        if !left && !right {
            left = true;
            right = true;
        }

        let strings = args[i..].to_vec();

        Ok(TrimOptions {
            left,
            right,
            chars,
            strings,
        })
    }
}

fn trim_string(s: &str, opts: &TrimOptions) -> String {
    match &opts.chars {
        Some(chars) => {
            let char_set: Vec<char> = chars.chars().collect();
            let trimmed = if opts.left && opts.right {
                s.trim_matches(|c| char_set.contains(&c))
            } else if opts.left {
                s.trim_start_matches(|c| char_set.contains(&c))
            } else {
                s.trim_end_matches(|c| char_set.contains(&c))
            };
            trimmed.to_string()
        }
        None => {
            if opts.left && opts.right {
                s.trim().to_string()
            } else if opts.left {
                s.trim_start().to_string()
            } else {
                s.trim_end().to_string()
            }
        }
    }
}

pub fn run_trim(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match TrimOptions::parse(args) {
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
        let trimmed = trim_string(input, &opts);
        output.push_str(&trimmed);
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
    fn test_string_trim_basic() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["  hello  "]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_trim_left_only() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["-l", "  hello  "]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello  \n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_trim_right_only() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["-r", "  hello  "]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "  hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_trim_chars() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["-c", "x", "xxxhelloxxx"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_trim_chars_multiple() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["-c", "abc", "aaabbbccc-hello-aaabbbccc"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "-hello-\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_trim_multiple_strings() {
        let mut rt = make_runtime();
        let result = run_trim(&args(&["  a  ", "  b  "]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "a\nb\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_trim_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"  hello  \n  world  ";
        let result = run_trim(&args(&[]), &mut rt, Some(stdin_data)).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\nworld\n");
        } else {
            panic!("expected text output");
        }
    }
}
