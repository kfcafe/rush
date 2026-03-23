use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;

#[derive(Debug, Default)]
struct SortOptions {
    numeric: bool,
    reverse: bool,
    unique: bool,
    /// 1-based field index to sort by (0 = whole line)
    key: usize,
    separator: Option<char>,
    files: Vec<String>,
}

impl SortOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = SortOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "-n" {
                opts.numeric = true;
            } else if arg == "-r" {
                opts.reverse = true;
            } else if arg == "-u" {
                opts.unique = true;
            } else if arg == "-k" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("sort: option requires an argument -- 'k'"));
                }
                let raw = &args[i];
                // Accept "N" or "N,N" — we only use the start field
                let field_str = raw.split(',').next().unwrap_or(raw);
                // Strip any trailing flags like 'n', 'r' for compatibility
                let field_num: String = field_str
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                opts.key = field_num
                    .parse::<usize>()
                    .map_err(|_| anyhow!("sort: invalid field number: '{}'", raw))?;
            } else if let Some(rest) = arg.strip_prefix("-k") {
                // -k2 style
                let field_str = rest.split(',').next().unwrap_or(rest);
                let field_num: String = field_str
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                opts.key = field_num
                    .parse::<usize>()
                    .map_err(|_| anyhow!("sort: invalid field number: '{}'", rest))?;
            } else if arg == "-t" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("sort: option requires an argument -- 't'"));
                }
                let sep = &args[i];
                if sep.chars().count() != 1 {
                    return Err(anyhow!("sort: separator must be a single character"));
                }
                opts.separator = sep.chars().next();
            } else if let Some(rest) = arg.strip_prefix("-t") {
                // -t: style
                if rest.chars().count() != 1 {
                    return Err(anyhow!("sort: separator must be a single character"));
                }
                opts.separator = rest.chars().next();
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Allow combined flags like -nr, -ru
                for ch in arg[1..].chars() {
                    match ch {
                        'n' => opts.numeric = true,
                        'r' => opts.reverse = true,
                        'u' => opts.unique = true,
                        _ => return Err(anyhow!("sort: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }

            i += 1;
        }

        Ok(opts)
    }
}

/// Extract the sort key from a line given the options.
fn extract_key<'a>(line: &'a str, opts: &SortOptions) -> &'a str {
    if opts.key == 0 {
        return line;
    }
    let sep = opts.separator.unwrap_or(' ');
    let fields: Vec<&str> = if sep == ' ' {
        // Whitespace-split: skip leading whitespace, split on any whitespace
        line.split_whitespace().collect()
    } else {
        line.split(sep).collect()
    };
    let idx = opts.key.saturating_sub(1);
    fields.get(idx).copied().unwrap_or(line)
}

/// Sort lines according to opts.
fn sort_lines(lines: Vec<String>, opts: &SortOptions) -> Vec<String> {
    let mut sorted = lines;

    if opts.numeric {
        sorted.sort_by(|a, b| {
            let ka = extract_key(a, opts);
            let kb = extract_key(b, opts);
            let na: f64 = ka.trim().parse().unwrap_or(0.0);
            let nb: f64 = kb.trim().parse().unwrap_or(0.0);
            na.partial_cmp(&nb)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        });
    } else {
        sorted.sort_by(|a, b| {
            let ka = extract_key(a, opts);
            let kb = extract_key(b, opts);
            ka.cmp(kb).then_with(|| a.cmp(b))
        });
    }

    if opts.reverse {
        sorted.reverse();
    }

    if opts.unique {
        sorted.dedup_by(|a, b| {
            let ka = extract_key(a, opts);
            let kb = extract_key(b, opts);
            ka == kb
        });
    }

    sorted
}

fn read_file_lines(path: &str) -> std::io::Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().map(|l| l.to_string()).collect())
}

fn lines_from_bytes(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    text.lines().map(|l| l.to_string()).collect()
}

pub fn builtin_sort(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match SortOptions::parse(args) {
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

    let mut all_lines: Vec<String> = Vec::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    if opts.files.is_empty() {
        // Read from stdin
        use std::io::Read;
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data).unwrap_or(0);
        all_lines.extend(lines_from_bytes(&data));
    } else {
        for path in &opts.files {
            if path == "-" {
                use std::io::Read;
                let mut data = Vec::new();
                std::io::stdin().read_to_end(&mut data).unwrap_or(0);
                all_lines.extend(lines_from_bytes(&data));
            } else {
                match read_file_lines(path) {
                    Ok(lines) => all_lines.extend(lines),
                    Err(e) => {
                        stderr_output.push_str(&format!("sort: {}: {}\n", path, e));
                        exit_code = 1;
                    }
                }
            }
        }
    }

    let sorted = sort_lines(all_lines, &opts);
    let mut output = String::new();
    for line in &sorted {
        output.push_str(line);
        output.push('\n');
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

pub fn builtin_sort_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match SortOptions::parse(args) {
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

    let mut all_lines: Vec<String> = Vec::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    if opts.files.is_empty() {
        all_lines.extend(lines_from_bytes(stdin_data));
    } else {
        for path in &opts.files {
            if path == "-" {
                all_lines.extend(lines_from_bytes(stdin_data));
            } else {
                match read_file_lines(path) {
                    Ok(lines) => all_lines.extend(lines),
                    Err(e) => {
                        stderr_output.push_str(&format!("sort: {}: {}\n", path, e));
                        exit_code = 1;
                    }
                }
            }
        }
    }

    let sorted = sort_lines(all_lines, &opts);
    let mut output = String::new();
    for line in &sorted {
        output.push_str(line);
        output.push('\n');
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Runtime {
        Runtime::new()
    }

    fn sort_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_sort_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_builtin_sort_basic_alphabetical() {
        let result = sort_stdin(&[], b"banana\napple\ncherry\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "apple\nbanana\ncherry\n");
    }

    #[test]
    fn test_builtin_sort_numeric() {
        let result = sort_stdin(&["-n"], b"10\n2\n30\n1\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "1\n2\n10\n30\n");
    }

    #[test]
    fn test_builtin_sort_reverse() {
        let result = sort_stdin(&["-r"], b"banana\napple\ncherry\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "cherry\nbanana\napple\n");
    }

    #[test]
    fn test_builtin_sort_unique() {
        let result = sort_stdin(&["-u"], b"banana\napple\nbanana\ncherry\napple\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "apple\nbanana\ncherry\n");
    }

    #[test]
    fn test_builtin_sort_numeric_reverse() {
        let result = sort_stdin(&["-nr"], b"10\n2\n30\n1\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "30\n10\n2\n1\n");
    }

    #[test]
    fn test_builtin_sort_by_field() {
        let result = sort_stdin(&["-k", "2"], b"b 3\na 1\nc 2\n");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "a 1");
        assert_eq!(lines[1], "c 2");
        assert_eq!(lines[2], "b 3");
    }

    #[test]
    fn test_builtin_sort_field_separator() {
        let result = sort_stdin(&["-t", ":", "-k", "2"], b"b:3\na:1\nc:2\n");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "a:1");
        assert_eq!(lines[1], "c:2");
        assert_eq!(lines[2], "b:3");
    }

    #[test]
    fn test_builtin_sort_empty_input() {
        let result = sort_stdin(&[], b"");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_builtin_sort_already_sorted() {
        let result = sort_stdin(&[], b"apple\nbanana\ncherry\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "apple\nbanana\ncherry\n");
    }

    #[test]
    fn test_builtin_sort_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"zebra\nant\nmouse\n").unwrap();
        f.flush().unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let args: Vec<String> = vec![path];
        let result = builtin_sort(&args, &mut runtime()).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "ant\nmouse\nzebra\n");
    }

    #[test]
    fn test_builtin_sort_nonexistent_file() {
        let args: Vec<String> = vec!["/nonexistent/sort_test.txt".to_string()];
        let result = builtin_sort(&args, &mut runtime()).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("sort:"));
    }

    #[test]
    fn test_builtin_sort_invalid_option() {
        let result = sort_stdin(&["-z"], b"test\n");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
