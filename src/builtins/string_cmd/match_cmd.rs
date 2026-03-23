use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use regex::Regex;

struct MatchOptions {
    regex_mode: bool,
    entire: bool,
    case_insensitive: bool,
    invert: bool,
    pattern: String,
    strings: Vec<String>,
}

impl MatchOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut regex_mode = false;
        let mut entire = false;
        let mut case_insensitive = false;
        let mut invert = false;
        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "-r" | "--regex" => regex_mode = true,
                "-e" | "--entire" => entire = true,
                "-i" | "--ignore-case" => case_insensitive = true,
                "-v" | "--invert" => invert = true,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string match: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let pattern = args
            .get(i)
            .ok_or_else(|| anyhow::anyhow!("string match: missing pattern"))?
            .clone();
        i += 1;

        let strings = args[i..].to_vec();

        Ok(MatchOptions {
            regex_mode,
            entire,
            case_insensitive,
            invert,
            pattern,
            strings,
        })
    }
}

fn glob_match(pattern: &str, s: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        let p = pattern.to_lowercase();
        let s2 = s.to_lowercase();
        glob::Pattern::new(&p)
            .map(|pat| pat.matches(&s2))
            .unwrap_or(false)
    } else {
        glob::Pattern::new(pattern)
            .map(|pat| pat.matches(s))
            .unwrap_or(false)
    }
}

pub fn run_match(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match MatchOptions::parse(args) {
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
    let mut any_match = false;

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
                        "string match: invalid regex '{}': {}\n",
                        opts.pattern, e
                    ),
                    exit_code: 1,
                    error: None,
                });
            }
        };

        for input in &inputs {
            let matched = re.is_match(input);
            let emit = if opts.invert { !matched } else { matched };
            if emit {
                // With -e or -v: print entire string.
                // Without either: print just the matched portion.
                if opts.entire || opts.invert {
                    output.push_str(input);
                } else if let Some(m) = re.find(input) {
                    output.push_str(m.as_str());
                } else {
                    output.push_str(input);
                }
                output.push('\n');
                any_match = true;
            }
        }
    } else {
        // Glob mode: pattern must match the entire string
        for input in &inputs {
            let matched = glob_match(&opts.pattern, input, opts.case_insensitive);
            let emit = if opts.invert { !matched } else { matched };
            if emit {
                output.push_str(input);
                output.push('\n');
                any_match = true;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: if any_match { 0 } else { 1 },
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
    fn test_string_match_glob_basic() {
        let mut rt = make_runtime();
        let result = run_match(&args(&["*.rs", "foo.rs", "bar.txt"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo.rs\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_match_glob_no_match() {
        let mut rt = make_runtime();
        let result = run_match(&args(&["*.rs", "bar.txt"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_match_glob_case_insensitive() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-i", "*.RS", "foo.rs", "bar.txt"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo.rs\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_match_glob_invert() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-v", "*.rs", "foo.rs", "bar.txt"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "bar.txt\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_match_regex_basic() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-r", r"\d+", "abc123", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "123\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_match_regex_entire() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-r", "-e", r"\d+", "abc123", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "abc123\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_match_regex_case_insensitive() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-r", "-i", "hello", "Hello World", "bye"]), &mut rt, None)
                .unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "Hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_match_regex_invert() {
        let mut rt = make_runtime();
        let result =
            run_match(&args(&["-r", "-v", r"\d+", "abc123", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_match_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"foo.rs\nbar.txt\nbaz.rs";
        let result = run_match(&args(&["*.rs"]), &mut rt, Some(stdin_data)).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo.rs\nbaz.rs\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_match_invalid_regex() {
        let mut rt = make_runtime();
        let result = run_match(&args(&["-r", "[invalid", "foo"]), &mut rt, None).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid regex"));
    }
}
