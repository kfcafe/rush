use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct SubOptions {
    start: i64,       // 1-based; negative = from end
    end: Option<i64>, // 1-based inclusive; negative = from end
    length: Option<usize>,
    strings: Vec<String>,
}

impl SubOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut start: i64 = 1;
        let mut end: Option<i64> = None;
        let mut length: Option<usize> = None;
        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "-s" | "--start" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| anyhow::anyhow!("string sub: -s requires an argument"))?;
                    start = val.parse().map_err(|_| anyhow::anyhow!("string sub: invalid start value '{}'", val))?;
                }
                "-e" | "--end" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| anyhow::anyhow!("string sub: -e requires an argument"))?;
                    let v: i64 = val.parse().map_err(|_| anyhow::anyhow!("string sub: invalid end value '{}'", val))?;
                    end = Some(v);
                }
                "-l" | "--length" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| anyhow::anyhow!("string sub: -l requires an argument"))?;
                    let v: i64 = val.parse().map_err(|_| anyhow::anyhow!("string sub: invalid length value '{}'", val))?;
                    if v < 0 {
                        return Err(anyhow::anyhow!("string sub: length must be non-negative"));
                    }
                    length = Some(v as usize);
                }
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string sub: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        if end.is_some() && length.is_some() {
            return Err(anyhow::anyhow!("string sub: -l and -e are mutually exclusive"));
        }

        Ok(SubOptions {
            start,
            end,
            length,
            strings: args[i..].to_vec(),
        })
    }
}

/// Convert a 1-based fish-style index to a 0-based char index.
/// Negative indices count from the end (len-based).
/// Returns None if the index is out of bounds in a clamped-zero way, else Some(0-based).
fn fish_index_to_zero_based(idx: i64, char_len: usize) -> usize {
    let len = char_len as i64;
    if idx >= 0 {
        // 1-based positive: clamp to [0, len]
        let zero = idx - 1;
        zero.max(0).min(len) as usize
    } else {
        // Negative: -1 = last char
        let zero = len + idx;
        zero.max(0).min(len) as usize
    }
}

fn extract_sub(s: &str, opts: &SubOptions) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    let start_idx = fish_index_to_zero_based(opts.start, len);

    let end_idx = if let Some(l) = opts.length {
        (start_idx + l).min(len)
    } else if let Some(e) = opts.end {
        let end_zero = fish_index_to_zero_based(e, len);
        // end is inclusive, so add 1
        (end_zero + 1).min(len)
    } else {
        len
    };

    if start_idx >= end_idx {
        return String::new();
    }

    chars[start_idx..end_idx].iter().collect()
}

pub fn run_sub(args: &[String], _runtime: &mut Runtime, stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let opts = match SubOptions::parse(args) {
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

    for s in &inputs {
        let sub = extract_sub(s, &opts);
        output.push_str(&sub);
        output.push('\n');
        if !sub.is_empty() {
            any_output = true;
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: if any_output { 0 } else { 1 },
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
    fn test_string_sub_whole_string() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_sub_start() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "3", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "llo\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_start_length() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "2", "-l", "3", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "ell\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_start_end() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "2", "-e", "4", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "ell\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_negative_start() {
        let mut rt = make_runtime();
        // -1 = last char
        let result = run_sub(&args(&["-s", "-2", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "lo\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_length_zero() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "2", "-l", "0", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_sub_multiple_strings() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "1", "-l", "2", "hello", "world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "he\nwo\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_unicode() {
        let mut rt = make_runtime();
        // "café": c=1, a=2, f=3, é=4
        let result = run_sub(&args(&["-s", "3", "-l", "2", "café"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "fé\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_sub_stdin() {
        let mut rt = make_runtime();
        let result = run_sub(&args(&["-s", "1", "-l", "3"]), &mut rt, Some(b"hello\nworld")).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hel\nwor\n");
        } else {
            panic!("expected text output");
        }
    }
}
