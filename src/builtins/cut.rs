use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
enum CutMode {
    Fields { delimiter: char, fields: Vec<Range> },
    Chars(Vec<Range>),
    Bytes(Vec<Range>),
}

/// An inclusive range (1-indexed, None = open-ended)
#[derive(Debug, Clone)]
struct Range {
    start: usize,       // 1-indexed
    end: Option<usize>, // None = unbounded
}

impl Range {
    fn contains(&self, pos: usize) -> bool {
        pos >= self.start && self.end.map_or(true, |e| pos <= e)
    }
}

/// Parse a field/char/byte spec like "1", "1,3", "1-3", "2-", "-3", "1,3-5,7-"
fn parse_ranges(spec: &str) -> Result<Vec<Range>> {
    let mut ranges = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(anyhow!("cut: invalid field specification '{}'", spec));
        }
        if let Some(dash_pos) = part.find('-') {
            let start_str = &part[..dash_pos];
            let end_str = &part[dash_pos + 1..];
            let start = if start_str.is_empty() {
                1
            } else {
                start_str
                    .parse::<usize>()
                    .map_err(|_| anyhow!("cut: invalid field specification '{}'", spec))?
            };
            if start == 0 {
                return Err(anyhow!("cut: fields are numbered from 1"));
            }
            let end = if end_str.is_empty() {
                None
            } else {
                let n = end_str
                    .parse::<usize>()
                    .map_err(|_| anyhow!("cut: invalid field specification '{}'", spec))?;
                if n == 0 {
                    return Err(anyhow!("cut: fields are numbered from 1"));
                }
                if n < start {
                    return Err(anyhow!(
                        "cut: invalid decreasing range in field specification '{}'",
                        spec
                    ));
                }
                Some(n)
            };
            ranges.push(Range { start, end });
        } else {
            let n = part
                .parse::<usize>()
                .map_err(|_| anyhow!("cut: invalid field specification '{}'", spec))?;
            if n == 0 {
                return Err(anyhow!("cut: fields are numbered from 1"));
            }
            ranges.push(Range {
                start: n,
                end: Some(n),
            });
        }
    }
    ranges.sort_by_key(|r| r.start);
    Ok(ranges)
}

fn is_selected(pos: usize, ranges: &[Range]) -> bool {
    ranges.iter().any(|r| r.contains(pos))
}

struct CutOptions {
    mode: CutMode,
    files: Vec<String>,
}

impl CutOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut delimiter: Option<char> = None;
        let mut fields_spec: Option<String> = None;
        let mut chars_spec: Option<String> = None;
        let mut bytes_spec: Option<String> = None;
        let mut files = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            // Handle -dX and -d X forms
            if arg == "-d" {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| anyhow!("cut: option requires an argument -- 'd'"))?;
                let mut chars = val.chars();
                delimiter = Some(chars.next().unwrap_or('\t'));
            } else if let Some(rest) = arg.strip_prefix("-d") {
                let mut chars = rest.chars();
                delimiter = Some(chars.next().unwrap_or('\t'));
            } else if arg == "-f" {
                i += 1;
                fields_spec = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow!("cut: option requires an argument -- 'f'"))?
                        .clone(),
                );
            } else if let Some(rest) = arg.strip_prefix("-f") {
                fields_spec = Some(rest.to_string());
            } else if arg == "-c" {
                i += 1;
                chars_spec = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow!("cut: option requires an argument -- 'c'"))?
                        .clone(),
                );
            } else if let Some(rest) = arg.strip_prefix("-c") {
                chars_spec = Some(rest.to_string());
            } else if arg == "-b" {
                i += 1;
                bytes_spec = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow!("cut: option requires an argument -- 'b'"))?
                        .clone(),
                );
            } else if let Some(rest) = arg.strip_prefix("-b") {
                bytes_spec = Some(rest.to_string());
            } else if arg == "--" {
                files.extend_from_slice(&args[i + 1..]);
                break;
            } else if arg.starts_with('-') && arg.len() > 1 {
                return Err(anyhow!("cut: invalid option -- '{}'", &arg[1..]));
            } else {
                files.push(arg.clone());
            }
            i += 1;
        }

        // Determine mode
        let mode = if let Some(spec) = fields_spec {
            let ranges = parse_ranges(&spec)?;
            let delim = delimiter.unwrap_or('\t');
            CutMode::Fields {
                delimiter: delim,
                fields: ranges,
            }
        } else if let Some(spec) = chars_spec {
            let ranges = parse_ranges(&spec)?;
            CutMode::Chars(ranges)
        } else if let Some(spec) = bytes_spec {
            let ranges = parse_ranges(&spec)?;
            CutMode::Bytes(ranges)
        } else {
            return Err(anyhow!(
                "cut: you must specify a list of bytes, characters, or fields"
            ));
        };

        Ok(CutOptions { mode, files })
    }
}

fn cut_line(line: &str, mode: &CutMode) -> String {
    match mode {
        CutMode::Fields { delimiter, fields } => {
            let parts: Vec<&str> = line.split(*delimiter).collect();
            // If line has no delimiter, output the line unchanged (POSIX)
            if parts.len() == 1 {
                return line.to_string();
            }
            let selected: Vec<&str> = parts
                .iter()
                .enumerate()
                .filter(|(i, _)| is_selected(i + 1, fields))
                .map(|(_, s)| *s)
                .collect();
            selected.join(&delimiter.to_string())
        }
        CutMode::Chars(ranges) => {
            let chars: Vec<char> = line.chars().collect();
            let selected: String = chars
                .iter()
                .enumerate()
                .filter(|(i, _)| is_selected(i + 1, ranges))
                .map(|(_, c)| *c)
                .collect();
            selected
        }
        CutMode::Bytes(ranges) => {
            let bytes = line.as_bytes();
            let selected: Vec<u8> = bytes
                .iter()
                .enumerate()
                .filter(|(i, _)| is_selected(i + 1, ranges))
                .map(|(_, b)| *b)
                .collect();
            String::from_utf8_lossy(&selected).into_owned()
        }
    }
}

fn process_input(text: &str, mode: &CutMode) -> String {
    let mut output = String::new();
    for line in text.lines() {
        output.push_str(&cut_line(line, mode));
        output.push('\n');
    }
    // Preserve trailing newline absence if input had none (for single-line no-newline input)
    // Actually POSIX cut always terminates each selected line with newline, so this is fine.
    output
}

pub fn builtin_cut(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match CutOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut output = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    if opts.files.is_empty() {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).unwrap_or(0);
        output.push_str(&process_input(&buf, &opts.mode));
    } else {
        for path in &opts.files {
            if path == "-" {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf).unwrap_or(0);
                output.push_str(&process_input(&buf, &opts.mode));
            } else {
                match std::fs::read_to_string(path) {
                    Ok(content) => output.push_str(&process_input(&content, &opts.mode)),
                    Err(e) => {
                        stderr.push_str(&format!("cut: {}: {}\n", path, e));
                        exit_code = 1;
                    }
                }
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr,
        exit_code,
        error: None,
    })
}

pub fn builtin_cut_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    // If files are specified, fall through to regular execute; otherwise use stdin_data
    let opts = match CutOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    if opts.files.is_empty() {
        let text = String::from_utf8_lossy(stdin_data);
        let out = process_input(&text, &opts.mode);
        Ok(ExecutionResult {
            output: Output::Text(out),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        })
    } else {
        // Has explicit file args — delegate to the regular function which reads files
        builtin_cut(args, runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Runtime {
        Runtime::new()
    }

    fn cut_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_cut_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_cut_fields_default_delimiter() {
        // Default delimiter is tab
        let result = cut_stdin(&["-f1"], b"alpha\tbeta\tgamma\n");
        assert_eq!(result.stdout(), "alpha\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_fields_colon_delimiter() {
        let result = cut_stdin(&["-d:", "-f1"], b"root:x:0:0\n");
        assert_eq!(result.stdout(), "root\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_fields_range() {
        let result = cut_stdin(&["-d:", "-f1-3"], b"a:b:c:d:e\n");
        assert_eq!(result.stdout(), "a:b:c\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_fields_open_ended() {
        let result = cut_stdin(&["-d:", "-f2-"], b"a:b:c:d\n");
        assert_eq!(result.stdout(), "b:c:d\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_chars() {
        let result = cut_stdin(&["-c1-3"], b"hello\n");
        assert_eq!(result.stdout(), "hel\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_chars_specific() {
        let result = cut_stdin(&["-c1,3,5"], b"abcde\n");
        assert_eq!(result.stdout(), "ace\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cut_no_mode_error() {
        let result = cut_stdin(&[], b"data");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("you must specify"));
    }

    #[test]
    fn test_cut_fields_no_delimiter_passthrough() {
        // Line without delimiter should be passed through unchanged
        let result = cut_stdin(&["-d:", "-f1"], b"nodlim\n");
        assert_eq!(result.stdout(), "nodlim\n");
    }

    #[test]
    fn test_cut_multiline() {
        let result = cut_stdin(&["-d:", "-f1"], b"a:b\nc:d\n");
        assert_eq!(result.stdout(), "a\nc\n");
    }
}
