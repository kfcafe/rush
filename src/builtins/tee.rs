use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::Write;

struct TeeOptions {
    append: bool,
    files: Vec<String>,
}

impl TeeOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut append = false;
        let mut files = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-a" | "--append" => append = true,
                "--" => {
                    files.extend_from_slice(&args[i + 1..]);
                    break;
                }
                arg if arg.starts_with('-') && arg.len() > 1 => {
                    return Err(anyhow::anyhow!("tee: invalid option -- '{}'", &arg[1..]));
                }
                _ => files.push(args[i].clone()),
            }
            i += 1;
        }
        Ok(TeeOptions { append, files })
    }
}

/// Write data to all tee output files. Returns any error messages.
fn write_to_files(data: &[u8], opts: &TeeOptions) -> (String, i32) {
    let mut stderr = String::new();
    let mut exit_code = 0;
    for path in &opts.files {
        let result = OpenOptions::new()
            .write(true)
            .create(true)
            .append(opts.append)
            .truncate(!opts.append)
            .open(path);
        match result {
            Ok(mut f) => {
                if let Err(e) = f.write_all(data) {
                    stderr.push_str(&format!("tee: {}: {}\n", path, e));
                    exit_code = 1;
                }
            }
            Err(e) => {
                stderr.push_str(&format!("tee: {}: {}\n", path, e));
                exit_code = 1;
            }
        }
    }
    (stderr, exit_code)
}

pub fn builtin_tee(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match TeeOptions::parse(args) {
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

    // Read stdin
    let mut data = Vec::new();
    use std::io::Read;
    std::io::stdin().read_to_end(&mut data).unwrap_or(0);

    let (stderr, exit_code) = write_to_files(&data, &opts);

    Ok(ExecutionResult {
        output: Output::Text(String::from_utf8_lossy(&data).into_owned()),
        stderr,
        exit_code,
        error: None,
    })
}

pub fn builtin_tee_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = match TeeOptions::parse(args) {
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

    let (stderr, exit_code) = write_to_files(stdin_data, &opts);

    Ok(ExecutionResult {
        output: Output::Text(String::from_utf8_lossy(stdin_data).into_owned()),
        stderr,
        exit_code,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    fn runtime() -> Runtime {
        Runtime::new()
    }

    fn tee_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_tee_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_tee_passthrough() {
        let result = tee_stdin(&[], b"hello\nworld\n");
        assert_eq!(result.stdout(), "hello\nworld\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_tee_writes_to_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let result = tee_stdin(&[&path], b"test data\n");
        assert_eq!(result.stdout(), "test data\n");
        assert_eq!(result.exit_code, 0);

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "test data\n");
    }

    #[test]
    fn test_tee_append() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        tee_stdin(&[&path], b"first\n");
        tee_stdin(&["-a", &path], b"second\n");

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "first\nsecond\n");
    }

    #[test]
    fn test_tee_invalid_option() {
        let result = tee_stdin(&["-z"], b"data");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
