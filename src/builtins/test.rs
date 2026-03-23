use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub fn builtin_test(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let result = evaluate_test(args, runtime, false)?;
    let exit_code = if result { 0 } else { 1 };
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

pub fn builtin_bracket(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // [ requires closing ]
    if args.is_empty() || args.last() != Some(&"]".to_string()) {
        return Err(anyhow!("[: missing closing ']'"));
    }

    // Remove the closing ] and evaluate
    let test_args = &args[..args.len() - 1];
    let result = evaluate_test(test_args, runtime, true)?;
    let exit_code = if result { 0 } else { 1 };
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

fn evaluate_test(args: &[String], runtime: &Runtime, _is_bracket: bool) -> Result<bool> {
    if args.is_empty() {
        return Ok(false);
    }

    evaluate_expression(args, runtime)
}

fn evaluate_expression(args: &[String], runtime: &Runtime) -> Result<bool> {
    if args.is_empty() {
        return Ok(false);
    }

    // Handle negation
    if args[0] == "!" {
        let rest = &args[1..];
        return Ok(!evaluate_expression(rest, runtime)?);
    }

    // Handle -o (or) operator - lower precedence
    if let Some(pos) = args.iter().position(|s| s == "-o") {
        let left = &args[..pos];
        let right = &args[pos + 1..];
        return Ok(evaluate_expression(left, runtime)? || evaluate_expression(right, runtime)?);
    }

    // Handle -a (and) operator - higher precedence
    if let Some(pos) = args.iter().position(|s| s == "-a") {
        let left = &args[..pos];
        let right = &args[pos + 1..];
        return Ok(evaluate_expression(left, runtime)? && evaluate_expression(right, runtime)?);
    }

    // Single argument: true if non-empty string
    if args.len() == 1 {
        return Ok(!args[0].is_empty());
    }

    // Two arguments: unary operators
    if args.len() == 2 {
        return evaluate_unary(&args[0], &args[1], runtime);
    }

    // Three arguments: binary operators
    if args.len() == 3 {
        return evaluate_binary(&args[0], &args[1], &args[2], runtime);
    }

    // Four or more arguments: complex expression
    // Try to parse as unary followed by expression
    if args.len() > 3 {
        // This handles cases like: ! -f file -a -d dir
        // We already handled ! at the start, so this shouldn't happen
        return Err(anyhow!("test: too many arguments"));
    }

    Err(anyhow!("test: invalid expression"))
}

fn evaluate_unary(op: &str, arg: &str, runtime: &Runtime) -> Result<bool> {
    match op {
        // String tests
        "-z" => Ok(arg.is_empty()),
        "-n" => Ok(!arg.is_empty()),

        // File tests
        "-e" => Ok(resolve_path(arg, runtime).exists()),
        "-f" => {
            let path = resolve_path(arg, runtime);
            Ok(path.exists() && path.is_file())
        }
        "-d" => {
            let path = resolve_path(arg, runtime);
            Ok(path.exists() && path.is_dir())
        }
        "-r" => {
            let path = resolve_path(arg, runtime);
            Ok(is_readable(&path))
        }
        "-w" => {
            let path = resolve_path(arg, runtime);
            Ok(is_writable(&path))
        }
        "-x" => {
            let path = resolve_path(arg, runtime);
            Ok(is_executable(&path))
        }
        "-s" => {
            let path = resolve_path(arg, runtime);
            Ok(path.exists() && { fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) })
        }

        _ => Err(anyhow!("test: unknown unary operator: {}", op)),
    }
}

fn evaluate_binary(left: &str, op: &str, right: &str, _runtime: &Runtime) -> Result<bool> {
    match op {
        // String comparisons
        "=" | "==" => Ok(left == right),
        "!=" => Ok(left != right),

        // Numeric comparisons
        "-eq" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l == r)
        }
        "-ne" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l != r)
        }
        "-lt" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l < r)
        }
        "-le" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l <= r)
        }
        "-gt" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l > r)
        }
        "-ge" => {
            let l = parse_number(left)?;
            let r = parse_number(right)?;
            Ok(l >= r)
        }

        _ => Err(anyhow!("test: unknown binary operator: {}", op)),
    }
}

fn resolve_path(path: &str, runtime: &Runtime) -> std::path::PathBuf {
    let path_buf = std::path::PathBuf::from(path);
    if path_buf.is_absolute() {
        path_buf
    } else {
        runtime.get_cwd().join(path_buf)
    }
}

fn is_readable(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

fn is_writable(path: &Path) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        let permissions = metadata.permissions();
        // On Unix, check if we have write permission
        #[cfg(unix)]
        {
            permissions.mode() & 0o200 != 0
        }
        #[cfg(not(unix))]
        {
            !permissions.readonly()
        }
    } else {
        false
    }
}

fn is_executable(path: &Path) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        #[cfg(unix)]
        {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, check if it's a file
            metadata.is_file()
        }
    } else {
        false
    }
}

fn parse_number(s: &str) -> Result<i64> {
    s.parse::<i64>()
        .map_err(|_| anyhow!("test: integer expression expected: {}", s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn test_string_empty() {
        let mut runtime = test_runtime();

        // -z with empty string
        let result = builtin_test(&["-z".to_string(), "".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // -z with non-empty string
        let result = builtin_test(&["-z".to_string(), "hello".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_nonempty() {
        let mut runtime = test_runtime();

        // -n with non-empty string
        let result = builtin_test(&["-n".to_string(), "hello".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // -n with empty string
        let result = builtin_test(&["-n".to_string(), "".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_string_equality() {
        let mut runtime = test_runtime();

        // Equal strings
        let result = builtin_test(
            &["hello".to_string(), "=".to_string(), "hello".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // Not equal strings
        let result = builtin_test(
            &["hello".to_string(), "=".to_string(), "world".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);

        // != operator
        let result = builtin_test(
            &["hello".to_string(), "!=".to_string(), "world".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_numeric_comparison() {
        let mut runtime = test_runtime();

        // -eq
        let result = builtin_test(
            &["5".to_string(), "-eq".to_string(), "5".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // -ne
        let result = builtin_test(
            &["5".to_string(), "-ne".to_string(), "3".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // -lt
        let result = builtin_test(
            &["3".to_string(), "-lt".to_string(), "5".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // -le
        let result = builtin_test(
            &["5".to_string(), "-le".to_string(), "5".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // -gt
        let result = builtin_test(
            &["5".to_string(), "-gt".to_string(), "3".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // -ge
        let result = builtin_test(
            &["5".to_string(), "-ge".to_string(), "5".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_file_exists() {
        let mut runtime = test_runtime();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        runtime.set_cwd(temp_dir.path().to_path_buf());

        // File exists
        let result =
            builtin_test(&["-e".to_string(), "test.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // File doesn't exist
        let result = builtin_test(
            &["-e".to_string(), "nonexistent.txt".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_file_regular() {
        let mut runtime = test_runtime();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Regular file
        let result =
            builtin_test(&["-f".to_string(), "test.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Directory is not a regular file
        let result = builtin_test(&["-f".to_string(), ".".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_file_directory() {
        let mut runtime = test_runtime();
        let temp_dir = TempDir::new().unwrap();

        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Directory
        let result = builtin_test(&["-d".to_string(), ".".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // File is not a directory
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();
        let result =
            builtin_test(&["-d".to_string(), "test.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_file_nonempty() {
        let mut runtime = test_runtime();
        let temp_dir = TempDir::new().unwrap();

        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Non-empty file
        let file_path = temp_dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"content").unwrap();

        let result =
            builtin_test(&["-s".to_string(), "test.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Empty file
        let empty_file = temp_dir.path().join("empty.txt");
        File::create(&empty_file).unwrap();
        let result =
            builtin_test(&["-s".to_string(), "empty.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_negation() {
        let mut runtime = test_runtime();

        // ! -z with non-empty string (should be true)
        let result = builtin_test(
            &["!".to_string(), "-z".to_string(), "hello".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // ! -z with empty string (should be false)
        let result = builtin_test(
            &["!".to_string(), "-z".to_string(), "".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_bracket_builtin() {
        let mut runtime = test_runtime();

        // Valid bracket test
        let result = builtin_bracket(
            &[
                "5".to_string(),
                "-eq".to_string(),
                "5".to_string(),
                "]".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // Missing closing bracket
        let result = builtin_bracket(
            &["5".to_string(), "-eq".to_string(), "5".to_string()],
            &mut runtime,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_single_argument() {
        let mut runtime = test_runtime();

        // Non-empty string is true
        let result = builtin_test(&["hello".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Empty string is false
        let result = builtin_test(&["".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_boolean_and() {
        let mut runtime = test_runtime();

        // Both true
        let result = builtin_test(
            &[
                "-n".to_string(),
                "hello".to_string(),
                "-a".to_string(),
                "5".to_string(),
                "-eq".to_string(),
                "5".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // First false
        let result = builtin_test(
            &[
                "-z".to_string(),
                "hello".to_string(),
                "-a".to_string(),
                "5".to_string(),
                "-eq".to_string(),
                "5".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_boolean_or() {
        let mut runtime = test_runtime();

        // First true
        let result = builtin_test(
            &[
                "-n".to_string(),
                "hello".to_string(),
                "-o".to_string(),
                "5".to_string(),
                "-eq".to_string(),
                "3".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // Both false
        let result = builtin_test(
            &[
                "-z".to_string(),
                "hello".to_string(),
                "-o".to_string(),
                "5".to_string(),
                "-eq".to_string(),
                "3".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
    }
}
