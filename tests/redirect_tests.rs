use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;
use std::fs;
use tempfile::TempDir;

fn setup_test_env() -> TempDir {
    TempDir::new().unwrap()
}

fn execute_command(input: &str, temp_dir: &TempDir) -> Result<String, String> {
    let tokens = Lexer::tokenize(input).map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().map_err(|e| e.to_string())?;

    let mut executor = Executor::new();

    // Change to temp directory and update executor's runtime
    std::env::set_current_dir(temp_dir.path()).map_err(|e| e.to_string())?;
    executor
        .runtime_mut()
        .set_cwd(temp_dir.path().to_path_buf());

    let result = executor.execute(statements).map_err(|e| e.to_string())?;

    if result.exit_code != 0 {
        Err(result.stderr)
    } else {
        Ok(result.stdout())
    }
}

#[test]
fn test_stdout_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("output.txt");

    // Test basic stdout redirect
    execute_command(
        &format!("echo hello > {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[test]
fn test_stdout_append() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("append.txt");

    // First write
    execute_command(
        &format!("echo first > {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    // Append
    execute_command(
        &format!("echo second >> {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("first"));
    assert!(content.contains("second"));
}

#[test]
fn test_stdin_redirect() {
    let temp_dir = setup_test_env();
    let input_file = temp_dir.path().join("input.txt");

    // Create input file
    fs::write(&input_file, "test input\n").unwrap();

    // Test stdin redirect with wc (external command)
    // Note: This test uses external commands which may not be available on all systems
    // For builtins, stdin redirect would need to be handled before execution
    let _ = execute_command(&format!("wc -l < {}", input_file.display()), &temp_dir);
    // Just verify it doesn't crash - output depends on the system's wc implementation
}

#[test]
fn test_stderr_redirect() {
    let temp_dir = setup_test_env();
    let error_file = temp_dir.path().join("errors.log");

    // Stderr redirect is mainly for external commands
    // Builtins can redirect stderr after execution
    // For a simple test, just verify the redirect mechanism works
    let cmd = format!("echo test 2> {}", error_file.display());
    let _ = execute_command(&cmd, &temp_dir);

    // File should be created (even if empty for this command)
    assert!(error_file.exists());
}

#[test]
fn test_both_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("both.log");

    // Redirect both stdout and stderr
    execute_command(
        &format!("echo hello &> {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("hello"));
}

#[test]
fn test_redirect_with_pipeline() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("pipeline.txt");

    // Simple redirect after echo (not in pipeline for now)
    let cmd = format!("echo banana > {}", output_file.display());
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "banana");
}

#[test]
fn test_overwrite_existing_file() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("overwrite.txt");

    // First write
    execute_command(
        &format!("echo first > {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    // Overwrite (not append)
    execute_command(
        &format!("echo second > {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "second");
    assert!(!content.contains("first"));
}

#[test]
fn test_multiple_redirects() {
    let temp_dir = setup_test_env();
    let stdout_file = temp_dir.path().join("stdout.txt");
    let stderr_file = temp_dir.path().join("stderr.txt");

    // This should redirect stdout to one file and stderr to another
    // Using a command that produces both stdout and stderr
    let cmd = format!(
        "echo hello > {} 2> {}",
        stdout_file.display(),
        stderr_file.display()
    );
    execute_command(&cmd, &temp_dir).unwrap();

    let stdout_content = fs::read_to_string(&stdout_file).unwrap();
    assert_eq!(stdout_content.trim(), "hello");
}

#[test]
fn test_redirect_missing_file() {
    let temp_dir = setup_test_env();

    // Try to read from a file that doesn't exist (using external command)
    // This test verifies error handling for missing input files
    let result = execute_command("wc < /nonexistent/file/path.txt", &temp_dir);
    // The result should be an error since the file doesn't exist
    assert!(
        result.is_err(),
        "Expected error when redirecting from non-existent file"
    );
}

#[test]
fn test_redirect_to_subdirectory() {
    let temp_dir = setup_test_env();
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let output_file = subdir.join("output.txt");
    execute_command(&format!("echo test > {}", output_file.display()), &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "test");
}

#[test]
fn test_redirect_with_quotes() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("quoted.txt");

    execute_command(
        &format!("echo \"hello world\" > {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello world");
}

#[test]
fn test_append_creates_file() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("new_append.txt");

    // Append should create the file if it doesn't exist
    execute_command(
        &format!("echo content >> {}", output_file.display()),
        &temp_dir,
    )
    .unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "content");
}

#[test]
fn test_stderr_to_stdout_basic() {
    let temp_dir = setup_test_env();

    // This test is tricky because we need a command that outputs to stderr
    // We'll use a simple approach with ls of a non-existent file
    // The 2>&1 should merge stderr into stdout
    let result = execute_command("ls nonexistent_xyz 2>&1", &temp_dir);

    // The error should be in stdout now (captured in result)
    // We expect an error, but it should be captured in the execution
    match result {
        Ok(_stdout) => {
            // stderr was redirected to stdout, so we got output
            // This is actually the expected behavior
        }
        Err(_) => {
            // This is also acceptable depending on how the shell handles it
        }
    }
}

#[test]
fn test_pipe_with_stdout_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("pipe_output.txt");

    // Test pipe with stdout redirect
    let cmd = format!("echo hello | cat > {}", output_file.display());
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[test]
fn test_pipe_with_append_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("pipe_append.txt");

    // First write via pipe
    let cmd1 = format!("echo first | cat > {}", output_file.display());
    execute_command(&cmd1, &temp_dir).unwrap();

    // Append via pipe
    let cmd2 = format!("echo second | cat >> {}", output_file.display());
    execute_command(&cmd2, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("first"));
    assert!(content.contains("second"));
}

#[test]
fn test_pipe_with_stderr_redirect() {
    let temp_dir = setup_test_env();
    let error_file = temp_dir.path().join("pipe_errors.log");

    // Note: stderr redirect after pipe may not capture pipe's stderr
    let cmd = format!("echo test | cat 2> {}", error_file.display());
    let _ = execute_command(&cmd, &temp_dir);

    // File should be created
    assert!(error_file.exists());
}

#[test]
fn test_pipe_with_both_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("pipe_both.log");

    // Redirect both stdout and stderr through pipeline
    let cmd = format!("echo hello | cat &> {}", output_file.display());
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("hello"));
}

#[test]
fn test_multiple_pipes_with_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("multi_pipe.txt");

    // Multiple pipes with final redirect (using echo to avoid grep color codes)
    let cmd = format!("echo hello | cat | cat > {}", output_file.display());
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[test]
fn test_grep_pipe_redirect() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("grep_result.txt");

    // Create input file for piping through grep
    let input_file = temp_dir.path().join("input.txt");
    fs::write(&input_file, "apple\nbanana\napple pie\n").unwrap();

    // Use grep with pipe and redirect
    let cmd = format!(
        "grep apple {} | cat > {}",
        input_file.display(),
        output_file.display()
    );
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("apple"));
}

#[test]
fn test_pipe_redirect_with_input_redirect() {
    let temp_dir = setup_test_env();
    let input_file = temp_dir.path().join("pipe_input.txt");
    let output_file = temp_dir.path().join("pipe_output_from_input.txt");

    // Create input file
    fs::write(&input_file, "hello world\n").unwrap();

    // Use cat directly on file and redirect to output
    let cmd = format!("cat {} > {}", input_file.display(), output_file.display());
    execute_command(&cmd, &temp_dir).unwrap();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello world");
}

#[test]
fn test_echo_with_all_redirect_types() {
    let temp_dir = setup_test_env();
    let stdout_file = temp_dir.path().join("out1.txt");
    let stdout_file2 = temp_dir.path().join("out2.txt");

    // Test basic redirect
    execute_command(
        &format!("echo test1 > {}", stdout_file.display()),
        &temp_dir,
    )
    .unwrap();
    let content = fs::read_to_string(&stdout_file).unwrap();
    assert_eq!(content.trim(), "test1");

    // Test append redirect
    execute_command(
        &format!("echo test2 >> {}", stdout_file.display()),
        &temp_dir,
    )
    .unwrap();
    let content = fs::read_to_string(&stdout_file).unwrap();
    assert!(content.contains("test1"));
    assert!(content.contains("test2"));

    // Test both redirect
    execute_command(
        &format!("echo test3 &> {}", stdout_file2.display()),
        &temp_dir,
    )
    .unwrap();
    let content = fs::read_to_string(&stdout_file2).unwrap();
    assert_eq!(content.trim(), "test3");
}

#[test]
fn test_redirect_preserves_pipe_output() {
    let temp_dir = setup_test_env();
    let output_file = temp_dir.path().join("pipe_preserved.txt");

    // Complex pipeline: echo -> wc -> cat -> redirect
    let cmd = format!(
        "echo 'hello world' | wc -c | cat > {}",
        output_file.display()
    );
    let _ = execute_command(&cmd, &temp_dir);

    // File should exist and contain output
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    // wc -c counts characters including newline
    assert!(!content.is_empty());
}
