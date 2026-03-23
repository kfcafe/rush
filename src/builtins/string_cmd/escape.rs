use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

#[derive(Debug, PartialEq)]
enum EscapeStyle {
    Script, // default: single-quote shell escaping
    Url,
}

struct EscapeOptions {
    style: EscapeStyle,
    strings: Vec<String>,
}

impl EscapeOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut style = EscapeStyle::Script;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--style=script" => style = EscapeStyle::Script,
                "--style=url" => style = EscapeStyle::Url,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--style=") => {
                    let s = &a["--style=".len()..];
                    return Err(anyhow::anyhow!(
                        "string escape: unknown style '{}' (valid: script, url)",
                        s
                    ));
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string escape: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let strings = args[i..].to_vec();

        Ok(EscapeOptions { style, strings })
    }
}

struct UnescapeOptions {
    style: EscapeStyle,
    strings: Vec<String>,
}

impl UnescapeOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut style = EscapeStyle::Script;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--style=script" => style = EscapeStyle::Script,
                "--style=url" => style = EscapeStyle::Url,
                "--" => {
                    i += 1;
                    break;
                }
                a if a.starts_with("--style=") => {
                    let s = &a["--style=".len()..];
                    return Err(anyhow::anyhow!(
                        "string unescape: unknown style '{}' (valid: script, url)",
                        s
                    ));
                }
                a if a.starts_with('-') && a.len() > 1 => {
                    return Err(anyhow::anyhow!("string unescape: unknown option '{}'", a));
                }
                _ => break,
            }
            i += 1;
        }

        let strings = args[i..].to_vec();

        Ok(UnescapeOptions { style, strings })
    }
}

/// Shell-escape a string by wrapping in single quotes.
/// Embedded single quotes become '\''.
fn shell_escape(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

/// URL-encode a string (percent-encode all non-unreserved characters).
/// Unreserved chars per RFC 3986: A-Z a-z 0-9 - _ . ~
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b => {
                out.push('%');
                out.push(hex_nibble(b >> 4));
                out.push(hex_nibble(b & 0xf));
            }
        }
    }
    out
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'A' + n - 10) as char,
        _ => unreachable!(),
    }
}

/// URL-decode a percent-encoded string. Invalid sequences are left as-is.
fn url_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Unescape a shell-escaped string. Handles single-quoted strings.
fn shell_unescape(s: &str) -> String {
    // If the string is wrapped in single quotes (possibly with '\'' for literal quotes), parse it.
    // This handles the output of shell_escape.
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' {
            // Single-quoted section: read until closing '
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip closing '
            }
        } else if chars[i] == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub fn run_escape(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match EscapeOptions::parse(args) {
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
        let escaped = match opts.style {
            EscapeStyle::Script => shell_escape(input),
            EscapeStyle::Url => url_encode(input),
        };
        output.push_str(&escaped);
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

pub fn run_unescape(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let opts = match UnescapeOptions::parse(args) {
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
        let unescaped = match opts.style {
            EscapeStyle::Script => shell_unescape(input),
            EscapeStyle::Url => url_decode(input),
        };
        output.push_str(&unescaped);
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
    fn test_string_escape_basic() {
        let mut rt = make_runtime();
        let result = run_escape(&args(&["hello world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "'hello world'\n");
        } else {
            panic!("expected text output");
        }
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_string_escape_single_quote() {
        let mut rt = make_runtime();
        let result = run_escape(&args(&["it's"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "'it'\\''s'\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_escape_url() {
        let mut rt = make_runtime();
        let result = run_escape(&args(&["--style=url", "hello world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello%20world\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_escape_url_special_chars() {
        let mut rt = make_runtime();
        let result =
            run_escape(&args(&["--style=url", "foo=bar&baz=1"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "foo%3Dbar%26baz%3D1\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_unescape_script() {
        let mut rt = make_runtime();
        let result = run_unescape(&args(&["'hello world'"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello world\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_unescape_url() {
        let mut rt = make_runtime();
        let result =
            run_unescape(&args(&["--style=url", "hello%20world"]), &mut rt, None).unwrap();
        if let Output::Text(t) = result.output {
            assert_eq!(t, "hello world\n");
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_string_escape_roundtrip_url() {
        let mut rt = make_runtime();
        let original = "foo/bar?x=1&y=hello world";
        let escape_result =
            run_escape(&args(&["--style=url", original]), &mut rt, None).unwrap();
        let encoded = if let Output::Text(t) = escape_result.output {
            t.trim_end_matches('\n').to_string()
        } else {
            panic!("expected text output")
        };
        let unescape_result =
            run_unescape(&args(&["--style=url", &encoded]), &mut rt, None).unwrap();
        if let Output::Text(t) = unescape_result.output {
            assert_eq!(t, format!("{}\n", original));
        } else {
            panic!("expected text output");
        }
    }
}
