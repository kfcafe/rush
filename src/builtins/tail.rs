use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};

#[derive(Debug)]
enum TailMode {
    /// Print last N lines
    LastLines(usize),
    /// Print from line N onward (1-based)
    FromLine(usize),
    /// Print last N bytes
    LastBytes(usize),
    /// Print from byte offset N onward
    FromByte(usize),
}

#[derive(Debug)]
struct TailOptions {
    mode: TailMode,
    /// Files to read (empty = stdin)
    files: Vec<String>,
}

impl Default for TailOptions {
    fn default() -> Self {
        Self {
            mode: TailMode::LastLines(10),
            files: Vec::new(),
        }
    }
}

impl TailOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = TailOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "-n" || arg == "--lines" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("tail: option requires an argument -- 'n'"));
                }
                opts.mode = parse_line_arg(&args[i])?;
            } else if arg == "-c" || arg == "--bytes" {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("tail: option requires an argument -- 'c'"));
                }
                opts.mode = parse_byte_arg(&args[i])?;
            } else if let Some(rest) = arg.strip_prefix("-n") {
                opts.mode = parse_line_arg(rest)?;
            } else if let Some(rest) = arg.strip_prefix("-c") {
                opts.mode = parse_byte_arg(rest)?;
            } else if arg == "-f" || arg == "--follow" {
                // -f (follow) is not supported in-process; silently ignore and defer to external.
                // The executor will fall back to the system tail if -f is needed.
                // For now, we just skip the flag and behave like a normal tail.
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Numeric shorthand like -10 => last 10 lines
                let rest = &arg[1..];
                if let Ok(n) = rest.parse::<usize>() {
                    opts.mode = TailMode::LastLines(n);
                } else {
                    return Err(anyhow!("tail: invalid option -- '{}'", arg));
                }
            } else {
                opts.files.push(arg.clone());
            }
            i += 1;
        }

        Ok(opts)
    }
}

fn parse_line_arg(s: &str) -> Result<TailMode> {
    if let Some(rest) = s.strip_prefix('+') {
        // +N means start from line N
        let n: usize = rest
            .parse()
            .map_err(|_| anyhow!("tail: invalid number of lines: '{}'", s))?;
        Ok(TailMode::FromLine(n))
    } else {
        let n: usize = s
            .parse()
            .map_err(|_| anyhow!("tail: invalid number of lines: '{}'", s))?;
        Ok(TailMode::LastLines(n))
    }
}

fn parse_byte_arg(s: &str) -> Result<TailMode> {
    if let Some(rest) = s.strip_prefix('+') {
        let n: usize = rest
            .parse()
            .map_err(|_| anyhow!("tail: invalid number of bytes: '{}'", s))?;
        Ok(TailMode::FromByte(n))
    } else {
        let n: usize = s
            .parse()
            .map_err(|_| anyhow!("tail: invalid number of bytes: '{}'", s))?;
        Ok(TailMode::LastBytes(n))
    }
}

/// Output the tail of a seekable file efficiently using seek for byte modes.
fn tail_file(file: File, file_path: &str, output: &mut String, mode: &TailMode) -> Result<()> {
    match mode {
        TailMode::LastBytes(n) => {
            let mut file = file;
            let len = file.metadata()?.len();
            let skip = if len as usize > *n { len as usize - n } else { 0 };
            file.seek(SeekFrom::Start(skip as u64))?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            output.push_str(&String::from_utf8_lossy(&buf));
        }
        TailMode::FromByte(n) => {
            let mut file = file;
            let offset = if *n > 0 { *n - 1 } else { 0 };
            file.seek(SeekFrom::Start(offset as u64))?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            output.push_str(&String::from_utf8_lossy(&buf));
        }
        _ => {
            let reader = BufReader::new(file);
            tail_reader(reader, output, mode)
                .map_err(|e| anyhow!("tail: {}: {}", file_path, e))?;
        }
    }
    Ok(())
}

/// Output the tail of data from any BufRead source
fn tail_reader<R: BufRead>(reader: R, output: &mut String, mode: &TailMode) -> Result<()> {
    match mode {
        TailMode::LastLines(n) => {
            if *n == 0 {
                return Ok(());
            }
            // Use a VecDeque as a circular buffer of the last N lines
            let mut ring: VecDeque<String> = VecDeque::with_capacity(*n);
            for line_result in reader.lines() {
                let line = line_result?;
                if ring.len() == *n {
                    ring.pop_front();
                }
                ring.push_back(line);
            }
            for line in ring {
                output.push_str(&line);
                output.push('\n');
            }
        }
        TailMode::FromLine(n) => {
            // Print from line N onward (1-based)
            let start = if *n > 0 { *n - 1 } else { 0 };
            for (idx, line_result) in reader.lines().enumerate() {
                let line = line_result?;
                if idx >= start {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
        }
        TailMode::LastBytes(n) => {
            // Read all, take last N bytes
            let mut buf = Vec::new();
            let mut reader = reader;
            reader.read_to_end(&mut buf)?;
            let start = if buf.len() > *n { buf.len() - n } else { 0 };
            output.push_str(&String::from_utf8_lossy(&buf[start..]));
        }
        TailMode::FromByte(n) => {
            let mut buf = Vec::new();
            let mut reader = reader;
            reader.read_to_end(&mut buf)?;
            let start = if *n > 0 { (*n - 1).min(buf.len()) } else { 0 };
            output.push_str(&String::from_utf8_lossy(&buf[start..]));
        }
    }
    Ok(())
}

pub fn builtin_tail(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match TailOptions::parse(args) {
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
        if let Err(e) = tail_reader(reader, &mut output, &opts.mode) {
            stderr_output = format!("tail: {}", e);
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
                    if let Err(e) = tail_file(file, file_path, &mut output, &opts.mode) {
                        stderr_output.push_str(&format!("tail: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
                Err(e) => {
                    stderr_output.push_str(&format!("tail: {}: {}\n", file_path, e));
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

/// Execute tail with provided stdin data (for pipelines)
pub fn builtin_tail_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match TailOptions::parse(args) {
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
        tail_reader(reader, &mut output, &opts.mode)?;
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
                    if let Err(e) = tail_file(file, file_path, &mut output, &opts.mode) {
                        stderr_output.push_str(&format!("tail: {}: {}\n", file_path, e));
                        exit_code = 1;
                    }
                }
                Err(e) => {
                    stderr_output.push_str(&format!("tail: {}: {}\n", file_path, e));
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
    fn test_builtin_tail_default_10_lines() {
        let content = make_lines(20);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&[path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 11");
        assert_eq!(lines[9], "line 20");
    }

    #[test]
    fn test_builtin_tail_n_flag() {
        let content = make_lines(20);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&["-n".to_string(), "5".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line 16");
        assert_eq!(lines[4], "line 20");
    }

    #[test]
    fn test_builtin_tail_n_plus_offset() {
        let content = make_lines(10);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        // +3 means from line 3 onward
        let result = builtin_tail(&["-n".to_string(), "+3".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 8);
        assert_eq!(lines[0], "line 3");
        assert_eq!(lines[7], "line 10");
    }

    #[test]
    fn test_builtin_tail_c_flag() {
        let file = create_test_file("Hello, World!\n");
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&["-c".to_string(), "7".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "World!\n");
    }

    #[test]
    fn test_builtin_tail_fewer_lines_than_requested() {
        let content = make_lines(3);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&["-n".to_string(), "10".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_builtin_tail_nonexistent_file() {
        let mut runtime = Runtime::new();
        let result = builtin_tail(&["/nonexistent/file.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("tail:"));
    }

    #[test]
    fn test_builtin_tail_stdin() {
        let data = make_lines(20);
        let mut runtime = Runtime::new();
        let result = builtin_tail_with_stdin(&[], &mut runtime, data.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let _stdout = result.stdout();
        let lines: Vec<&str> = _stdout.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 11");
        assert_eq!(lines[9], "line 20");
    }

    #[test]
    fn test_builtin_tail_multiple_files_headers() {
        let f1 = create_test_file("a\nb\n");
        let f2 = create_test_file("c\nd\n");
        let p1 = f1.path().to_str().unwrap().to_string();
        let p2 = f2.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&[p1.clone(), p2.clone()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(out.contains(&format!("==> {} <==", p1)));
        assert!(out.contains(&format!("==> {} <==", p2)));
    }

    #[test]
    fn test_builtin_tail_n0_empty() {
        let content = make_lines(5);
        let file = create_test_file(&content);
        let path = file.path().to_str().unwrap().to_string();

        let mut runtime = Runtime::new();
        let result = builtin_tail(&["-n".to_string(), "0".to_string(), path], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }
}
