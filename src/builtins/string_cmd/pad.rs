use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct PadOptions {
    width: usize,
    right: bool,
    pad_char: char,
    strings: Vec<String>,
}

impl PadOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut width: Option<usize> = None;
        let mut right = false;
        let mut pad_char = ' ';
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "-w" | "--width" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string pad: -w requires an argument")
                    })?;
                    width = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string pad: invalid width '{}'", val)
                    })?);
                }
                "-r" | "--right" => right = true,
                "-c" | "--char" => {
                    i += 1;
                    let val = args.get(i).ok_or_else(|| {
                        anyhow::anyhow!("string pad: -c requires an argument")
                    })?;
                    let mut chars = val.chars();
                    pad_char = chars.next().ok_or_else(|| {
                        anyhow::anyhow!("string pad: pad character cannot be empty")
                    })?;
                    if chars.next().is_some() {
                        return Err(anyhow::anyhow!(
                            "string pad: pad character must be a single character"
                        ));
                    }
                }
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--width=") => {
                    let val = &a["--width=".len()..];
                    width = Some(val.parse().map_err(|_| {
                        anyhow::anyhow!("string pad: invalid width '{}'", val)
                    })?);
                }
                a if a.starts_with("--char=") => {
                    let val = &a["--char=".len()..];
                    let mut chars = val.chars();
                    pad_char = chars.next().ok_or_else(|| {
                        anyhow::anyhow!("string pad: pad character cannot be empty")
                    })?;
                    if chars.next().is_some() {
                        return Err(anyhow::anyhow!(
                            "string pad: pad character must be a single character"
                        ));
                    }
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string pad: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let width = width.ok_or_else(|| anyhow::anyhow!("string pad: -w/--width is required"))?;
        let strings = args[i..].to_vec();

        Ok(PadOptions {
            width,
            right,
            pad_char,
            strings,
        })
    }
}

fn pad_string(s: &str, opts: &PadOptions) -> String {
    let char_count = s.chars().count();
    if char_count >= opts.width {
        return s.to_string();
    }
    let pad_count = opts.width - char_count;
    let padding: String = std::iter::repeat(opts.pad_char).take(pad_count).collect();
    if opts.right {
        format!("{}{}", s, padding)
    } else {
        format!("{}{}", padding, s)
    }
}

pub fn run_pad(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match PadOptions::parse(args) {
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
        let padded = pad_string(input, &opts);
        output.push_str(&padded);
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
    fn test_string_pad_basic() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["-w", "10", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "     hello\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_pad_right() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["-w", "10", "-r", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello     \n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_pad_custom_char() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["-w", "10", "-c", "0", "42"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "0000000042\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_pad_already_wide_enough() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["-w", "3", "hello"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_pad_multiple_strings() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["-w", "5", "a", "bb", "ccc"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "    a\n   bb\n  ccc\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_pad_missing_width() {
        let mut rt = make_runtime();
        let result = run_pad(&args(&["hello"]), &mut rt, None).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_pad_stdin() {
        let mut rt = make_runtime();
        let stdin_data = b"hi\nbye";
        let result = run_pad(&args(&["-w", "5"]), &mut rt, Some(stdin_data)).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "   hi\n  bye\n");
        } else {
            panic!("expected text output");
        }
    }
}
