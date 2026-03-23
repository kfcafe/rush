use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Counts for a single input source
#[derive(Default, Clone, Copy)]
struct Counts {
    lines: u64,
    words: u64,
    bytes: u64,
    chars: u64,
}

/// Which fields to display
#[derive(Default)]
struct WcOptions {
    lines: bool,
    words: bool,
    bytes: bool,
    chars: bool,
    files: Vec<String>,
}

impl WcOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = WcOptions::default();
        let mut explicit = false;

        for arg in args {
            if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        'l' => { opts.lines = true; explicit = true; }
                        'w' => { opts.words = true; explicit = true; }
                        'c' => { opts.bytes = true; explicit = true; }
                        'm' => { opts.chars = true; explicit = true; }
                        _ => return Err(anyhow!("wc: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }
        }

        // Default: show all three (lines, words, bytes)
        if !explicit {
            opts.lines = true;
            opts.words = true;
            opts.bytes = true;
        }

        Ok(opts)
    }

}

/// Count lines, words, bytes, and chars in a byte slice (single pass).
fn count_bytes(data: &[u8]) -> Counts {
    let mut counts = Counts::default();
    counts.bytes = data.len() as u64;

    let mut in_word = false;

    for &b in data {
        // Count newlines for lines
        if b == b'\n' {
            counts.lines += 1;
        }

        // Word boundary: ASCII whitespace
        let is_space = b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';
        if is_space {
            in_word = false;
        } else if !in_word {
            in_word = true;
            counts.words += 1;
        }
    }

    // Count Unicode characters
    counts.chars = String::from_utf8_lossy(data).chars().count() as u64;

    counts
}

/// Format one row of counts for a single source.
fn format_counts(counts: &Counts, opts: &WcOptions, label: &str) -> String {
    let mut parts = Vec::new();
    if opts.lines { parts.push(counts.lines.to_string()); }
    if opts.words { parts.push(counts.words.to_string()); }
    if opts.chars { parts.push(counts.chars.to_string()); }
    if opts.bytes { parts.push(counts.bytes.to_string()); }

    // Right-align numbers with a consistent width (matches GNU wc behaviour for single-column)
    let width = parts.iter().map(|s| s.len()).max().unwrap_or(1).max(7);
    let row: Vec<String> = parts.iter().map(|s| format!("{:>width$}", s, width = width)).collect();

    if label.is_empty() {
        row.join(" ")
    } else {
        format!("{} {}", row.join(" "), label)
    }
}

pub fn builtin_wc(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match WcOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: crate::executor::Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;
    let mut total = Counts::default();
    let multi = opts.files.len() > 1;

    if opts.files.is_empty() {
        // Read from stdin
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data).unwrap_or(0);
        let counts = count_bytes(&data);
        output.push_str(&format_counts(&counts, &opts, ""));
        output.push('\n');
    } else {
        for file_path in &opts.files {
            if file_path == "-" {
                let mut data = Vec::new();
                std::io::stdin().read_to_end(&mut data).unwrap_or(0);
                let counts = count_bytes(&data);
                total = accumulate(total, counts);
                output.push_str(&format_counts(&counts, &opts, "-"));
                output.push('\n');
            } else {
                match read_file(file_path) {
                    Ok(data) => {
                        let counts = count_bytes(&data);
                        total = accumulate(total, counts);
                        output.push_str(&format_counts(&counts, &opts, file_path));
                        output.push('\n');
                    }
                    Err(e) => {
                        stderr_output.push_str(&format!("wc: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
            }
        }

        if multi {
            output.push_str(&format_counts(&total, &opts, "total"));
            output.push('\n');
        }
    }

    Ok(ExecutionResult {
        output: crate::executor::Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

/// Execute wc with provided stdin data (for pipelines).
pub fn builtin_wc_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match WcOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: crate::executor::Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;
    let mut total = Counts::default();
    let multi = opts.files.len() > 1;

    if opts.files.is_empty() {
        let counts = count_bytes(stdin_data);
        output.push_str(&format_counts(&counts, &opts, ""));
        output.push('\n');
    } else {
        for file_path in &opts.files {
            if file_path == "-" {
                let counts = count_bytes(stdin_data);
                total = accumulate(total, counts);
                output.push_str(&format_counts(&counts, &opts, "-"));
                output.push('\n');
            } else {
                match read_file(file_path) {
                    Ok(data) => {
                        let counts = count_bytes(&data);
                        total = accumulate(total, counts);
                        output.push_str(&format_counts(&counts, &opts, file_path));
                        output.push('\n');
                    }
                    Err(e) => {
                        stderr_output.push_str(&format!("wc: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
            }
        }

        if multi {
            output.push_str(&format_counts(&total, &opts, "total"));
            output.push('\n');
        }
    }

    Ok(ExecutionResult {
        output: crate::executor::Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

fn accumulate(mut a: Counts, b: Counts) -> Counts {
    a.lines += b.lines;
    a.words += b.words;
    a.bytes += b.bytes;
    a.chars += b.chars;
    a
}

fn read_file(path: &str) -> std::io::Result<Vec<u8>> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No such file or directory",
        ));
    }
    let mut file = File::open(p)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Runtime {
        Runtime::new()
    }

    // Helper: run builtin_wc_with_stdin
    fn wc_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_wc_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_builtin_wc_default_counts_lines_words_bytes() {
        let result = wc_stdin(&[], b"hello world\nfoo bar baz\n");
        let out = result.stdout();
        // Should show lines=2, words=5, bytes=24
        assert!(out.contains('2'), "expected 2 lines in: {}", out);
        assert!(out.contains('5'), "expected 5 words in: {}", out);
        assert!(out.contains("24"), "expected 24 bytes in: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_lines_only() {
        let result = wc_stdin(&["-l"], b"line1\nline2\nline3\n");
        let out = result.stdout();
        assert!(out.contains('3'), "expected 3 lines in: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_words_only() {
        let result = wc_stdin(&["-w"], b"one two three\n");
        let out = result.stdout();
        assert!(out.contains('3'), "expected 3 words in: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_bytes_only() {
        let result = wc_stdin(&["-c"], b"hello\n");
        let out = result.stdout();
        assert!(out.contains('6'), "expected 6 bytes in: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_chars_only() {
        let result = wc_stdin(&["-m"], "héllo\n".as_bytes());
        let out = result.stdout();
        // "héllo\n" = 6 Unicode chars (é is one char)
        assert!(out.contains('6'), "expected 6 chars in: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_empty_input() {
        let result = wc_stdin(&[], b"");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(out.contains('0'), "expected zeros in: {}", out);
    }

    #[test]
    fn test_builtin_wc_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"alpha beta gamma\ndelta\n").unwrap();
        f.flush().unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let args: Vec<String> = vec![path.clone()];
        let result = builtin_wc(&args, &mut runtime()).unwrap();
        let out = result.stdout();
        // 2 lines, 4 words, 22 bytes
        assert!(out.contains('2'), "expected 2 lines: {}", out);
        assert!(out.contains('4'), "expected 4 words: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_multiple_files_total() {
        use std::io::Write;
        let mut f1 = tempfile::NamedTempFile::new().unwrap();
        f1.write_all(b"one\ntwo\n").unwrap();
        f1.flush().unwrap();
        let mut f2 = tempfile::NamedTempFile::new().unwrap();
        f2.write_all(b"three\nfour\nfive\n").unwrap();
        f2.flush().unwrap();

        let args: Vec<String> = vec![
            f1.path().to_str().unwrap().to_string(),
            f2.path().to_str().unwrap().to_string(),
        ];
        let result = builtin_wc(&args, &mut runtime()).unwrap();
        let out = result.stdout();
        assert!(out.contains("total"), "expected 'total' line: {}", out);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_wc_nonexistent_file() {
        let args: Vec<String> = vec!["/nonexistent/wc_test_file.txt".to_string()];
        let result = builtin_wc(&args, &mut runtime()).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("wc:"));
    }

    #[test]
    fn test_builtin_wc_invalid_option() {
        let result = wc_stdin(&["-z"], b"test");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
