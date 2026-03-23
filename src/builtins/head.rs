use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

#[derive(Debug)]
struct HeadOptions {
    /// Number of lines to print (default 10)
    lines: Option<usize>,
    /// Number of bytes to print
    bytes: Option<usize>,
    /// Files to read (empty = stdin)
    files: Vec<String>,
}

impl Default for HeadOptions {
    fn default() -> Self {
        Self {
            lines: None,
            bytes: None,
            files: Vec::new(),
        }
    }
}

impl HeadOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = HeadOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                // Everything after -- is a file
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "-n" || arg == "--lines" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("head: option requires an argument -- 'n'"));
                }
                let n: usize = args[i]
                    .parse()
                    .map_err(|_| anyhow!("head: invalid number of lines: '{}'", args[i]))?;
                opts.lines = Some(n);
            } else if arg == "-c" || arg == "--bytes" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("head: option requires an argument -- 'c'"));
                }
                let n: usize = args[i]
                    .parse()
                    .map_err(|_| anyhow!("head: invalid number of bytes: '{}'", args[i]))?;
                opts.bytes = Some(n);
            } else if let Some(rest) = arg.strip_prefix("-n") {
                // -n10 style
                let n: usize = rest
                    .parse()
                    .map_err(|_| anyhow!("head: invalid number of lines: '{}'", rest))?;
                opts.lines = Some(n);
            } else if let Some(rest) = arg.strip_prefix("-c") {
                // -c10 style
                let n: usize = rest
                    .parse()
                    .map_err(|_| anyhow!("head: invalid number of bytes: '{}'", rest))?;
                opts.bytes = Some(n);
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Check if it's a numeric flag like -10 (shorthand for -n 10)
                let rest = &arg[1..];
                if let Ok(n) = rest.parse::<usize>() {
                    opts.lines = Some(n);
                } else {
                    return Err(anyhow!("head: invalid option -- '{}'", arg));
                }
            } else {
                opts.files.push(arg.clone());
            }
            i += 1;
        }

        Ok(opts)
    }

    fn line_count(&self) -> usize {
        self.lines.unwrap_or(10)
    }
}

/// Print the first N lines (or bytes) of a reader into output
fn head_reader<R: BufRead>(
    reader: R,
    output: &mut String,
    opts: &HeadOptions,
) -> Result<()> {
    if let Some(byte_count) = opts.bytes {
        // Byte mode: read exactly N bytes
        let mut buf = vec![0u8; byte_count];
        let mut reader = reader;
        let n = reader.read(&mut buf)?;
        output.push_str(&String::from_utf8_lossy(&buf[..n]));
    } else {
        // Line mode
        let line_count = opts.line_count();
        let mut count = 0;
        for line_result in reader.lines() {
            if count >= line_count {
                break;
            }
            let line = line_result?;
            output.push_str(&line);
            output.push('\n');
            count += 1;
        }
    }
    Ok(())
}

pub fn builtin_head(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match HeadOptions::parse(args) {
        Ok(opts) => opts,
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
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    if opts.files.is_empty() {
        // Read from stdin
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        if let Err(e) = head_reader(reader, &mut output, &opts) {
            stderr_output = format!("head: {}", e);
            exit_code = 1;
        }
    } else {
        let print_headers = opts.files.len() > 1;
        for (idx, file_path) in opts.files.iter().enumerate() {
            if idx > 0 {
                output.push('\n');
            }
            if print_headers {
                output.push_str(&format!("==> {} <==\n", file_path));
            }
            match File::open(file_path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    if let Err(e) = head_reader(reader, &mut output, &opts) {
                        stderr_output.push_str(&format!("head: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
                Err(e) => {
                    stderr_output.push_str(&format!("head: {}: {}\n", file_path, e));
                    exit_code = 1;
                }
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

/// Execute head with provided stdin data (for pipelines)
pub fn builtin_head_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match HeadOptions::parse(args) {
        Ok(opts) => opts,
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

    if opts.files.is_empty() {
        let cursor = std::io::Cursor::new(stdin_data);
        let reader = BufReader::new(cursor);
        head_reader(reader, &mut output, &opts)?;
    } else {
        let print_headers = opts.files.len() > 1;
        let mut exit_code = 0;
        let mut stderr_output = String::new();
        for (idx, file_path) in opts.files.iter().enumerate() {
            if idx > 0 {
                output.push('\n');
            }
            if print_headers {
                output.push_str(&format!("==> {} <==\n", file_path));
            }
            match File::open(file_path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    if let Err(e) = head_reader(reader, &mut output, &opts) {
                        stderr_output.push_str(&format!("head: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
                Err(e) => {
                    stderr_output.push_str(&format!("head: {}: {}\n", file_path, e));
                    exit_code = 1;
                }
            }
        }
        return Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: stderr_output,
            exit_code,
            error: None,
        });
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    fn make_lines(n: usize) -> String {
        (1..=n).map(|i| format!("line {}\n", i)).collect()
    }

    #[test]
    fn test_builtin_head_default_10_lines() {
        let content = make_lines(20);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_head(&[path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[9], "line 10");
    }

    #[test]
    fn test_builtin_head_n_flag() {
        let content = make_lines(20);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_head(&["-n".to_string(), "5".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[4], "line 5");
    }

    #[test]
    fn test_builtin_head_c_flag() {
        let file = create_test_file("Hello, World!\n");
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_head(&["-c".to_string(), "5".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "Hello");
    }

    #[test]
    fn test_builtin_head_fewer_lines_than_requested() {
        let content = make_lines(3);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_head(&["-n".to_string(), "10".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_builtin_head_nonexistent_file() {
        let mut runtime = Runtime::new();
        let result = builtin_head(&["/nonexistent/file.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("head:"));
    }

    #[test]
    fn test_builtin_head_stdin() {
        let data = make_lines(20);
        let mut runtime = Runtime::new();
        let result = builtin_head_with_stdin(&[], &mut runtime, data.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn test_builtin_head_multiple_files_headers() {
        let f1 = create_test_file("a\nb\n");
        let f2 = create_test_file("c\nd\n");
        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_head(&[p1.clone(), p2.clone()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(out.contains(&format!("==> {} <==", p1)));
        assert!(out.contains(&format!("==> {} <==", p2)));
        assert!(out.contains("a\nb\n"));
        assert!(out.contains("c\nd\n"));
    }
}
