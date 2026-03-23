use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use regex::Regex;

struct ReplaceOptions {
    regex_mode: bool,
    all: bool,
    case_insensitive: bool,
    pattern: String,
    replacement: String,
    strings: Vec<String>,
}

impl ReplaceOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut regex_mode = false;
        let mut all = false;
        let mut case_insensitive = false;
        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "-r" | "--regex" => regex_mode = true,
                "-a" | "--all" => all = true,
                "-i" | "--ignore-case" => case_insensitive = true,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string replace: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let pattern = args
            .get(i)
            .ok_or_else(|| anyhow::anyhow!("string replace: missing pattern"))?
            .clone();
        i += 1;

        let replacement = args
            .get(i)
            .ok_or_else(|| anyhow::anyhow!("string replace: missing replacement"))?
            .clone();
        i += 1;

        let strings = args[i..].to_vec();

        Ok(ReplaceOptions {
            regex_mode,
            all,
            case_insensitive,
            pattern,
            replacement,
            strings,
        })
    }
}

fn literal_replace(s: &str, pattern: &str, replacement: &str, all: bool, case_insensitive: bool) -> String {
    if case_insensitive {
        let s_lower = s.to_lowercase();
        let p_lower = pattern.to_lowercase();
        if all {
            let mut result = String::new();
            let mut remaining = s;
            let mut remaining_lower = s_lower.as_str();
            while let Some(pos) = remaining_lower.find(p_lower.as_str()) {
                result.push_str(&remaining[..pos]);
                result.push_str(replacement);
                remaining = &remaining[pos + pattern.len()..];
                remaining_lower = &remaining_lower[pos + pattern.len()..];
            }
            result.push_str(remaining);
            result
        } else if let Some(pos) = s_lower.find(p_lower.as_str()) {
            let mut result = String::new();
            result.push_str(&s[..pos]);
            result.push_str(replacement);
            result.push_str(&s[pos + pattern.len()..]);
            result
        } else {
            s.to_string()
        }
    } else if all {
        s.replace(pattern, replacement)
    } else {
        s.replacen(pattern, replacement, 1)
    }
}

pub fn run_replace(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match ReplaceOptions::parse(args) {
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
    let mut any_replaced = false;

    if opts.regex_mode {
        let mut pat_str = String::new();
        if opts.case_insensitive {
            pat_str.push_str("(?i)");
        }
        pat_str.push_str(&opts.pattern);

        let re = match Regex::new(&pat_str) {
            Ok(r) => r,
            Err(e) => {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: format!(
                        "string replace: invalid regex '{}': {}\n",
                        opts.pattern, e
                    ),
                    exit_code: 1,
                    error: None,
                });
            }
        };

        for input in &inputs {
            let result = if opts.all {
                re.replace_all(input, opts.replacement.as_str()).into_owned()
            } else {
                re.replace(input, opts.replacement.as_str()).into_owned()
            };
            if result != *input {
                any_replaced = true;
            }
            output.push_str(&result);
            output.push('\n');
        }
    } else {
        for input in &inputs {
            let result = literal_replace(
                input,
                &opts.pattern,
                &opts.replacement,
                opts.all,
                opts.case_insensitive,
            );
            if result != *input {
                any_replaced = true;
            }
            output.push_str(&result);
            output.push('\n');
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: if any_replaced { 0 } else { 1 },
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
    fn test_string_replace_literal_basic() {
        let mut rt = make_runtime();
        let result = run_replace(&args(&["foo", "bar", "foo baz foo"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "bar baz foo\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_replace_literal_all() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["-a", "foo", "bar", "foo baz foo"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "bar baz bar\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_literal_no_match() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["xyz", "bar", "foo baz"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo baz\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_replace_literal_case_insensitive() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["-i", "FOO", "bar", "foo baz"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "bar baz\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_regex_basic() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["-r", r"\d+", "NUM", "abc123def"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "abcNUMdef\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_replace_regex_all() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["-r", "-a", r"\d+", "N", "a1b2c3"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "aNbNcN\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_regex_capture_group() {
        let mut rt = make_runtime();
        let result = run_replace(
            &args(&["-r", r"(\w+)\s+(\w+)", "$2 $1", "hello world"]),
            &mut rt,
            None,
        )
        .unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "world hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_multiple_strings() {
        let mut rt = make_runtime();
        let result =
            run_replace(&args(&["a", "b", "cat", "bat"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "cbt\nbbt\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"hello world\ngoodbye world";
        let result = run_replace(&args(&["-a", "world", "there"]), &mut rt, Some(stdin_data)).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello there\ngoodbye there\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_replace_invalid_regex() {
        let mut rt = make_runtime();
        let result = run_replace(&args(&["-r", "[invalid", "x", "foo"]), &mut rt, None).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid regex"));
    }
}
