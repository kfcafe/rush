use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;

#[derive(Debug, Default)]
struct UniqOptions {
    /// Prefix each output line with its count
    count: bool,
    /// Only print lines that appear more than once (adjacent duplicates)
    repeated: bool,
    /// Only print lines that appear exactly once
    unique: bool,
    files: Vec<String>,
}

impl UniqOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = UniqOptions::default();

        for arg in args {
            if arg == "--" {
                break;
            } else if arg.starts_with('-') && arg.len() > 1 {
                for ch in arg[1..].chars() {
                    match ch {
                        'c' => opts.count = true,
                        'd' => opts.repeated = true,
                        'u' => opts.unique = true,
                        _ => return Err(anyhow!("uniq: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }
        }

        Ok(opts)
    }
}

fn process_lines(lines: &[String], opts: &UniqOptions) -> String {
    let mut output = String::new();

    if lines.is_empty() {
        return output;
    }

    let mut i = 0;
    while i < lines.len() {
        let current = &lines[i];
        let mut run_count = 1usize;

        // Count consecutive duplicates
        while i + run_count < lines.len() && lines[i + run_count] == *current {
            run_count += 1;
        }

        let emit = if opts.repeated && opts.unique {
            // -d and -u together: emit nothing (contradictory filters)
            false
        } else if opts.repeated {
            run_count > 1
        } else if opts.unique {
            run_count == 1
        } else {
            true
        };

        if emit {
            if opts.count {
                output.push_str(&format!("{:>7} {}\n", run_count, current));
            } else {
                output.push_str(current);
                output.push('\n');
            }
        }

        i += run_count;
    }

    output
}

fn read_file_lines(path: &str) -> std::io::Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().map(|l| l.to_string()).collect())
}

fn lines_from_bytes(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    text.lines().map(|l| l.to_string()).collect()
}

pub fn builtin_uniq(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match UniqOptions::parse(args) {
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

    let mut exit_code = 0;
    let mut stderr_output = String::new();
    let lines: Vec<String>;

    if opts.files.is_empty() {
        use std::io::Read;
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data).unwrap_or(0);
        lines = lines_from_bytes(&data);
    } else {
        let path = &opts.files[0];
        match read_file_lines(path) {
            Ok(l) => lines = l,
            Err(e) => {
                stderr_output = format!("uniq: {}: {}\n", path, e);
                exit_code = 1;
                lines = Vec::new();
            }
        }
    }

    let output = process_lines(&lines, &opts);

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

pub fn builtin_uniq_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match UniqOptions::parse(args) {
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

    let mut exit_code = 0;
    let mut stderr_output = String::new();
    let lines: Vec<String>;

    if opts.files.is_empty() {
        lines = lines_from_bytes(stdin_data);
    } else {
        let path = &opts.files[0];
        match read_file_lines(path) {
            Ok(l) => lines = l,
            Err(e) => {
                stderr_output = format!("uniq: {}: {}\n", path, e);
                exit_code = 1;
                lines = Vec::new();
            }
        }
    }

    let output = process_lines(&lines, &opts);

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

    fn uniq_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_uniq_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_builtin_uniq_removes_adjacent_duplicates() {
        let result = uniq_stdin(&[], b"apple\napple\nbanana\nbanana\ncherry\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "apple\nbanana\ncherry\n");
    }

    #[test]
    fn test_builtin_uniq_does_not_remove_nonadjacent_duplicates() {
        let result = uniq_stdin(&[], b"apple\nbanana\napple\n");
        assert_eq!(result.exit_code, 0);
        // Non-adjacent duplicates are NOT removed (uniq only removes adjacent)
        assert_eq!(result.stdout(), "apple\nbanana\napple\n");
    }

    #[test]
    fn test_builtin_uniq_count() {
        let result = uniq_stdin(&["-c"], b"apple\napple\nbanana\n");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(
            out.contains("2 apple"),
            "expected count 2 for apple: {}",
            out
        );
        assert!(
            out.contains("1 banana"),
            "expected count 1 for banana: {}",
            out
        );
    }

    #[test]
    fn test_builtin_uniq_repeated_only() {
        let result = uniq_stdin(&["-d"], b"apple\napple\nbanana\ncherry\ncherry\n");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(out.contains("apple"), "expected apple: {}", out);
        assert!(out.contains("cherry"), "expected cherry: {}", out);
        assert!(
            !out.contains("banana"),
            "should not contain banana: {}",
            out
        );
    }

    #[test]
    fn test_builtin_uniq_unique_only() {
        let result = uniq_stdin(&["-u"], b"apple\napple\nbanana\ncherry\ncherry\n");
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(!out.contains("apple"), "should not contain apple: {}", out);
        assert!(out.contains("banana"), "expected banana: {}", out);
        assert!(
            !out.contains("cherry"),
            "should not contain cherry: {}",
            out
        );
    }

    #[test]
    fn test_builtin_uniq_empty_input() {
        let result = uniq_stdin(&[], b"");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_builtin_uniq_single_line() {
        let result = uniq_stdin(&[], b"only\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "only\n");
    }

    #[test]
    fn test_builtin_uniq_all_same() {
        let result = uniq_stdin(&[], b"same\nsame\nsame\n");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "same\n");
    }

    #[test]
    fn test_builtin_uniq_count_all_same() {
        let result = uniq_stdin(&["-c"], b"same\nsame\nsame\n");
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("3 same"));
    }

    #[test]
    fn test_builtin_uniq_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"a\na\nb\n").unwrap();
        f.flush().unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let args: Vec<String> = vec![path];
        let result = builtin_uniq(&args, &mut runtime()).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "a\nb\n");
    }

    #[test]
    fn test_builtin_uniq_nonexistent_file() {
        let args: Vec<String> = vec!["/nonexistent/uniq_test.txt".to_string()];
        let result = builtin_uniq(&args, &mut runtime()).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("uniq:"));
    }

    #[test]
    fn test_builtin_uniq_invalid_option() {
        let result = uniq_stdin(&["-z"], b"test\n");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
