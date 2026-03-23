use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::io::{self, BufRead, Write};
use std::time::Duration;

#[cfg(unix)]
use nix::libc;

/// Options for the read command
#[derive(Debug, Default)]
struct ReadOptions {
    /// Prompt string to display
    prompt: Option<String>,
    /// Silent mode (don't echo input)
    silent: bool,
    /// Timeout in seconds
    timeout: Option<u64>,
    /// Raw mode (no backslash processing)
    raw: bool,
    /// Variable names to read into
    variables: Vec<String>,
}

impl ReadOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = ReadOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg.starts_with('-') && arg.len() > 1 {
                match arg.as_str() {
                    "-p" => {
                        // Next arg is the prompt
                        i += 1;
                        if i >= args.len() {
                            return Err(anyhow!("read: -p: option requires an argument"));
                        }
                        opts.prompt = Some(args[i].clone());
                    }
                    "-s" => {
                        opts.silent = true;
                    }
                    "-r" => {
                        opts.raw = true;
                    }
                    "-t" => {
                        // Next arg is the timeout
                        i += 1;
                        if i >= args.len() {
                            return Err(anyhow!("read: -t: option requires an argument"));
                        }
                        let timeout = args[i]
                            .parse::<u64>()
                            .map_err(|_| anyhow!("read: -t: invalid timeout value: {}", args[i]))?;
                        opts.timeout = Some(timeout);
                    }
                    _ => return Err(anyhow!("read: {}: invalid option", arg)),
                }
            } else {
                // This is a variable name
                opts.variables.push(arg.clone());
            }

            i += 1;
        }

        // If no variables specified, default to REPLY
        if opts.variables.is_empty() {
            opts.variables.push("REPLY".to_string());
        }

        Ok(opts)
    }
}

/// Implement the `read` builtin command
///
/// Usage:
/// - `read varname` - Read a line into varname
/// - `read first second rest` - Split line into multiple variables
/// - `read -p "Enter name: " name` - Display a prompt
/// - `read -s password` - Silent input (for passwords)
/// - `read -t 5 answer` - Timeout after 5 seconds
/// - `read -r line` - Raw mode (no backslash processing)
pub fn builtin_read(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = ReadOptions::parse(args)?;

    // Display prompt if specified
    if let Some(prompt) = &opts.prompt {
        print!("{}", prompt);
        io::stdout().flush()?;
    }

    // Read the input line
    let line = if let Some(timeout) = opts.timeout {
        read_line_with_timeout(timeout, opts.silent)?
    } else {
        read_line(opts.silent)?
    };

    // Check for EOF
    if line.is_none() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: String::new(),
            exit_code: 1,
            error: None,
        });
    }

    let line = line.unwrap();

    // Process backslash escapes unless in raw mode
    let processed_line = if opts.raw {
        line
    } else {
        process_backslash_escapes(&line)
    };

    // Split the line according to IFS and assign to variables
    assign_variables(&opts.variables, &processed_line, runtime);

    Ok(ExecutionResult::success(String::new()))
}

/// Execute read with provided stdin data (for pipelines)
pub fn builtin_read_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = ReadOptions::parse(args)?;

    // Read one line from stdin_data
    let cursor = std::io::Cursor::new(stdin_data);
    let mut reader = io::BufReader::new(cursor);
    let mut line = String::new();

    match reader.read_line(&mut line) {
        Ok(0) => {
            // EOF
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: String::new(),
                exit_code: 1,
                error: None,
            });
        }
        Ok(_) => {
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
        }
        Err(e) => {
            return Err(anyhow!("read: error reading from stdin: {}", e));
        }
    }

    // Process backslash escapes unless in raw mode
    let processed_line = if opts.raw {
        line
    } else {
        process_backslash_escapes(&line)
    };

    // Split the line according to IFS and assign to variables
    assign_variables(&opts.variables, &processed_line, runtime);

    Ok(ExecutionResult::success(String::new()))
}

/// Read a line from stdin
fn read_line(silent: bool) -> Result<Option<String>> {
    if silent {
        // Use raw mode for silent input (password entry)
        read_line_silent()
    } else {
        // Normal line reading
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                // Remove trailing newline
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                Ok(Some(line))
            }
            Err(e) => Err(anyhow!("read: error reading from stdin: {}", e)),
        }
    }
}

/// Read a line silently (for password entry)
fn read_line_silent() -> Result<Option<String>> {
    // Disable echo
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdin_fd = io::stdin().as_raw_fd();

        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(stdin_fd, &mut termios) != 0 {
                return Err(anyhow!("read: failed to get terminal attributes"));
            }

            let original_lflag = termios.c_lflag;
            termios.c_lflag &= !libc::ECHO;

            if libc::tcsetattr(stdin_fd, libc::TCSANOW, &termios) != 0 {
                return Err(anyhow!("read: failed to set terminal attributes"));
            }

            // Read the line
            let stdin = io::stdin();
            let mut reader = stdin.lock();
            let mut line = String::new();

            let result = reader.read_line(&mut line);

            // Restore echo
            termios.c_lflag = original_lflag;
            libc::tcsetattr(stdin_fd, libc::TCSANOW, &termios);

            // Print newline since echo was disabled
            println!();

            match result {
                Ok(0) => Ok(None), // EOF
                Ok(_) => {
                    // Remove trailing newline
                    if line.ends_with('\n') {
                        line.pop();
                        if line.ends_with('\r') {
                            line.pop();
                        }
                    }
                    Ok(Some(line))
                }
                Err(e) => Err(anyhow!("read: error reading from stdin: {}", e)),
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback for non-Unix systems: just read normally
        // TODO: Implement proper silent reading for Windows
        read_line(false)
    }
}

/// Read a line with timeout
fn read_line_with_timeout(timeout_secs: u64, silent: bool) -> Result<Option<String>> {
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();

    // Spawn a thread to read the line
    thread::spawn(move || {
        let result = read_line(silent);
        let _ = tx.send(result);
    });

    // Wait for the result with timeout
    let timeout = Duration::from_secs(timeout_secs);
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Timeout occurred
            Ok(None)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow!("read: channel disconnected")),
    }
}

/// Process backslash escapes in a string (for non-raw mode)
///
/// In non-raw mode, backslash acts as an escape character:
/// - `\\` becomes `\`
/// - `\<newline>` is line continuation (removed)
/// - Other backslash sequences: the backslash is removed
fn process_backslash_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Look at the next character
            match chars.peek() {
                Some(&'\\') => {
                    // \\ -> \
                    result.push('\\');
                    chars.next();
                }
                Some(&'\n') => {
                    // Line continuation: skip both backslash and newline
                    chars.next();
                }
                Some(_) => {
                    // Any other backslash sequence: remove backslash, keep char
                    if let Some(next) = chars.next() {
                        result.push(next);
                    }
                }
                None => {
                    // Trailing backslash at end of line: remove it
                    // (acts as line continuation, but since there's no more input, just drop it)
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Split line according to IFS and assign to variables
fn assign_variables(variables: &[String], line: &str, runtime: &mut Runtime) {
    // Get IFS (Internal Field Separator) from runtime, default to space/tab/newline
    let ifs = runtime
        .get_variable("IFS")
        .unwrap_or_else(|| " \t\n".to_string());

    if variables.len() == 1 {
        // Single variable: assign entire line
        runtime.set_variable(variables[0].clone(), line.to_string());
    } else {
        // Multiple variables: split by IFS
        let fields = split_by_ifs(line, &ifs);

        for (i, var_name) in variables.iter().enumerate() {
            if i < variables.len() - 1 {
                // Not the last variable: assign one field
                let value = fields.get(i).map(|s| s.to_string()).unwrap_or_default();
                runtime.set_variable(var_name.clone(), value);
            } else {
                // Last variable: assign all remaining fields
                let remaining: Vec<&str> = fields.iter().skip(i).copied().collect();
                let value = remaining.join(" ");
                runtime.set_variable(var_name.clone(), value);
            }
        }
    }
}

/// Split a string by IFS characters
fn split_by_ifs<'a>(s: &'a str, ifs: &str) -> Vec<&'a str> {
    if ifs.is_empty() {
        // If IFS is empty, return the whole string as one field
        return vec![s];
    }

    // Split by any character in IFS
    let mut fields = Vec::new();
    let mut current_field_start = 0;
    let mut in_field = false;

    for (i, ch) in s.char_indices() {
        if ifs.contains(ch) {
            if in_field {
                fields.push(&s[current_field_start..i]);
                in_field = false;
            }
        } else if !in_field {
            current_field_start = i;
            in_field = true;
        }
    }

    // Add the last field if any
    if in_field {
        fields.push(&s[current_field_start..]);
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_by_ifs_default() {
        let fields = split_by_ifs("hello world  test", " \t\n");
        assert_eq!(fields, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_split_by_ifs_custom() {
        let fields = split_by_ifs("one:two::three", ":");
        assert_eq!(fields, vec!["one", "two", "three"]);
    }

    #[test]
    fn test_split_by_ifs_empty() {
        let fields = split_by_ifs("hello world", "");
        assert_eq!(fields, vec!["hello world"]);
    }

    #[test]
    fn test_assign_single_variable() {
        let mut runtime = Runtime::new();
        assign_variables(&["name".to_string()], "John Doe", &mut runtime);
        assert_eq!(runtime.get_variable("name"), Some("John Doe".to_string()));
    }

    #[test]
    fn test_assign_multiple_variables() {
        let mut runtime = Runtime::new();
        assign_variables(
            &["first".to_string(), "last".to_string()],
            "John Doe",
            &mut runtime,
        );
        assert_eq!(runtime.get_variable("first"), Some("John".to_string()));
        assert_eq!(runtime.get_variable("last"), Some("Doe".to_string()));
    }

    #[test]
    fn test_assign_multiple_variables_with_remainder() {
        let mut runtime = Runtime::new();
        assign_variables(
            &["first".to_string(), "rest".to_string()],
            "John Doe Smith",
            &mut runtime,
        );
        assert_eq!(runtime.get_variable("first"), Some("John".to_string()));
        assert_eq!(runtime.get_variable("rest"), Some("Doe Smith".to_string()));
    }

    #[test]
    fn test_assign_with_custom_ifs() {
        let mut runtime = Runtime::new();
        runtime.set_variable("IFS".to_string(), ":".to_string());
        assign_variables(
            &["user".to_string(), "home".to_string()],
            "root:/root",
            &mut runtime,
        );
        assert_eq!(runtime.get_variable("user"), Some("root".to_string()));
        assert_eq!(runtime.get_variable("home"), Some("/root".to_string()));
    }

    #[test]
    fn test_parse_options_simple() {
        let opts = ReadOptions::parse(&["name".to_string()]).unwrap();
        assert_eq!(opts.variables, vec!["name"]);
        assert_eq!(opts.prompt, None);
        assert!(!opts.silent);
        assert!(!opts.raw);
        assert_eq!(opts.timeout, None);
    }

    #[test]
    fn test_parse_options_with_prompt() {
        let opts = ReadOptions::parse(&[
            "-p".to_string(),
            "Enter name: ".to_string(),
            "name".to_string(),
        ])
        .unwrap();
        assert_eq!(opts.variables, vec!["name"]);
        assert_eq!(opts.prompt, Some("Enter name: ".to_string()));
    }

    #[test]
    fn test_parse_options_silent() {
        let opts = ReadOptions::parse(&["-s".to_string(), "password".to_string()]).unwrap();
        assert_eq!(opts.variables, vec!["password"]);
        assert!(opts.silent);
    }

    #[test]
    fn test_parse_options_raw() {
        let opts = ReadOptions::parse(&["-r".to_string(), "line".to_string()]).unwrap();
        assert_eq!(opts.variables, vec!["line"]);
        assert!(opts.raw);
    }

    #[test]
    fn test_parse_options_timeout() {
        let opts =
            ReadOptions::parse(&["-t".to_string(), "5".to_string(), "answer".to_string()]).unwrap();
        assert_eq!(opts.variables, vec!["answer"]);
        assert_eq!(opts.timeout, Some(5));
    }

    #[test]
    fn test_parse_options_combined() {
        let opts = ReadOptions::parse(&[
            "-p".to_string(),
            "Password: ".to_string(),
            "-s".to_string(),
            "-t".to_string(),
            "10".to_string(),
            "pass".to_string(),
        ])
        .unwrap();
        assert_eq!(opts.variables, vec!["pass"]);
        assert_eq!(opts.prompt, Some("Password: ".to_string()));
        assert!(opts.silent);
        assert_eq!(opts.timeout, Some(10));
    }

    #[test]
    fn test_parse_options_no_variables() {
        let opts = ReadOptions::parse(&[]).unwrap();
        assert_eq!(opts.variables, vec!["REPLY"]);
    }

    #[test]
    fn test_parse_options_invalid() {
        let result = ReadOptions::parse(&["-x".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_options_missing_prompt() {
        let result = ReadOptions::parse(&["-p".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_options_missing_timeout() {
        let result = ReadOptions::parse(&["-t".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_read_with_stdin() {
        let mut runtime = Runtime::new();
        let stdin_data = b"test line\n";

        let result =
            builtin_read_with_stdin(&["var".to_string()], &mut runtime, stdin_data).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(runtime.get_variable("var"), Some("test line".to_string()));
    }

    #[test]
    fn test_builtin_read_with_stdin_multiple_vars() {
        let mut runtime = Runtime::new();
        let stdin_data = b"first second third\n";

        let result = builtin_read_with_stdin(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            &mut runtime,
            stdin_data,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(runtime.get_variable("a"), Some("first".to_string()));
        assert_eq!(runtime.get_variable("b"), Some("second".to_string()));
        assert_eq!(runtime.get_variable("c"), Some("third".to_string()));
    }

    #[test]
    fn test_builtin_read_with_stdin_eof() {
        let mut runtime = Runtime::new();
        let stdin_data = b"";

        let result =
            builtin_read_with_stdin(&["var".to_string()], &mut runtime, stdin_data).unwrap();

        // EOF should return exit code 1
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_builtin_read_with_stdin_custom_ifs() {
        let mut runtime = Runtime::new();
        runtime.set_variable("IFS".to_string(), ":".to_string());
        let stdin_data = b"user:password:home\n";

        let result = builtin_read_with_stdin(
            &["u".to_string(), "p".to_string(), "h".to_string()],
            &mut runtime,
            stdin_data,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(runtime.get_variable("u"), Some("user".to_string()));
        assert_eq!(runtime.get_variable("p"), Some("password".to_string()));
        assert_eq!(runtime.get_variable("h"), Some("home".to_string()));
    }

    #[test]
    fn test_builtin_read_with_stdin_remainder() {
        let mut runtime = Runtime::new();
        let stdin_data = b"one two three four five\n";

        let result = builtin_read_with_stdin(
            &["first".to_string(), "rest".to_string()],
            &mut runtime,
            stdin_data,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(runtime.get_variable("first"), Some("one".to_string()));
        assert_eq!(
            runtime.get_variable("rest"),
            Some("two three four five".to_string())
        );
    }

    #[test]
    fn test_process_backslash_escapes_double_backslash() {
        assert_eq!(process_backslash_escapes(r"hello\\world"), r"hello\world");
    }

    #[test]
    fn test_process_backslash_escapes_escaped_char() {
        // \n should become just n (backslash removed)
        assert_eq!(process_backslash_escapes(r"hello\nworld"), "hellonworld");
    }

    #[test]
    fn test_process_backslash_escapes_trailing_backslash() {
        // Trailing backslash is removed
        assert_eq!(process_backslash_escapes(r"hello\"), "hello");
    }

    #[test]
    fn test_process_backslash_escapes_no_backslash() {
        assert_eq!(process_backslash_escapes("hello world"), "hello world");
    }

    #[test]
    fn test_builtin_read_with_stdin_raw_mode() {
        let mut runtime = Runtime::new();
        // Input contains backslashes - with -r they should be preserved
        let stdin_data = b"hello\\nworld\n";

        let result = builtin_read_with_stdin(
            &["-r".to_string(), "var".to_string()],
            &mut runtime,
            stdin_data,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        // In raw mode, backslashes are preserved
        assert_eq!(
            runtime.get_variable("var"),
            Some(r"hello\nworld".to_string())
        );
    }

    #[test]
    fn test_builtin_read_with_stdin_non_raw_mode() {
        let mut runtime = Runtime::new();
        // Input contains backslashes - without -r they should be processed
        let stdin_data = b"hello\\nworld\n";

        let result =
            builtin_read_with_stdin(&["var".to_string()], &mut runtime, stdin_data).unwrap();

        assert_eq!(result.exit_code, 0);
        // In non-raw mode, \n becomes just n
        assert_eq!(runtime.get_variable("var"), Some("hellonworld".to_string()));
    }

    #[test]
    fn test_builtin_read_with_stdin_raw_preserves_double_backslash() {
        let mut runtime = Runtime::new();
        let stdin_data = b"path\\\\to\\\\file\n";

        let result = builtin_read_with_stdin(
            &["-r".to_string(), "path".to_string()],
            &mut runtime,
            stdin_data,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        // In raw mode, both backslashes preserved
        assert_eq!(
            runtime.get_variable("path"),
            Some(r"path\\to\\file".to_string())
        );
    }

    #[test]
    fn test_builtin_read_with_stdin_non_raw_double_backslash() {
        let mut runtime = Runtime::new();
        let stdin_data = b"path\\\\to\\\\file\n";

        let result =
            builtin_read_with_stdin(&["path".to_string()], &mut runtime, stdin_data).unwrap();

        assert_eq!(result.exit_code, 0);
        // In non-raw mode, \\ becomes \
        assert_eq!(
            runtime.get_variable("path"),
            Some(r"path\to\file".to_string())
        );
    }
}
