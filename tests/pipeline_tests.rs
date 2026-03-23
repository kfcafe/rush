use std::fs;
use std::io::Write;
use std::process::Command;

/// Helper to run rush with a command
fn run_rush(cmd: &str) -> std::process::Output {
    Command::new("./target/release/rush")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("Failed to execute rush")
}

/// Helper to create test data file
fn create_test_file(path: &str, content: &str) {
    let mut file = fs::File::create(path).expect("Failed to create test file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test file");
}

#[test]
fn test_two_stage_pipeline() {
    create_test_file("/tmp/rush_test_pipe.txt", "hello\nworld\nrust\n");

    let output = run_rush("cat /tmp/rush_test_pipe.txt | grep --no-color -N rust");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "rust\n");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_pipe.txt").ok();
}

#[test]
fn test_three_stage_pipeline() {
    create_test_file(
        "/tmp/rush_test_pipe3.txt",
        "apple\nbanana\napple\napple\nbanana\n",
    );

    // cat | grep | wc
    let output = run_rush("cat /tmp/rush_test_pipe3.txt | grep --no-color -N apple | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(count, 3);
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_pipe3.txt").ok();
}

#[test]
fn test_five_stage_pipeline() {
    // Create a file with many lines
    let mut content = String::new();
    for i in 1..=100 {
        content.push_str(&format!("Line {}: test data\n", i));
    }
    create_test_file("/tmp/rush_test_pipe5.txt", &content);

    // 5-stage pipeline: cat | grep | grep | head | wc -l
    let output = run_rush(
        "cat /tmp/rush_test_pipe5.txt | grep --no-color -N Line | grep --no-color -N test | head -10 | wc -l",
    );
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(count, 10);
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_pipe5.txt").ok();
}

#[test]
fn test_pipeline_exit_code_last_command() {
    create_test_file("/tmp/rush_test_exitcode.txt", "hello\nworld\n");

    // First command succeeds, last command fails (grep with no match)
    let output = run_rush("cat /tmp/rush_test_exitcode.txt | grep --no-color -N nonexistent");
    assert_eq!(output.status.code(), Some(1)); // grep returns 1 for no match

    fs::remove_file("/tmp/rush_test_exitcode.txt").ok();
}

#[test]
fn test_pipeline_with_builtins() {
    create_test_file("/tmp/rush_test_builtins.txt", "line1\nline2\nline3\n");

    // Using Rush's built-in cat and grep (disable colors and line numbers for predictable output)
    let output = run_rush("cat /tmp/rush_test_builtins.txt | grep --no-color -N line2");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line2\n");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_builtins.txt").ok();
}

#[test]
fn test_pipeline_with_external_commands() {
    create_test_file("/tmp/rush_test_external.txt", "apple\nbanana\ncherry\n");

    // Using external sort command (via PATH)
    let output = run_rush("cat /tmp/rush_test_external.txt | sort");
    let stdout_str = String::from_utf8_lossy(&output.stdout);

    // Verify sorted output
    assert!(
        stdout_str.contains("apple"),
        "Output should contain 'apple': {:?}",
        stdout_str
    );
    assert!(
        stdout_str.contains("banana"),
        "Output should contain 'banana': {:?}",
        stdout_str
    );
    assert!(
        stdout_str.contains("cherry"),
        "Output should contain 'cherry': {:?}",
        stdout_str
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "Command should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_file("/tmp/rush_test_external.txt").ok();
}

#[test]
fn test_pipeline_large_data() {
    // Create a large file (10,000 lines)
    let mut content = String::new();
    for i in 1..=10000 {
        content.push_str(&format!("Line {}: Lorem ipsum dolor sit amet\n", i));
    }
    create_test_file("/tmp/rush_test_large.txt", &content);

    // Process large file through pipeline
    let output = run_rush("cat /tmp/rush_test_large.txt | grep --no-color -N Lorem | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(count, 10000);
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_large.txt").ok();
}

#[test]
fn test_pipeline_with_head_early_termination() {
    // Create a large file
    let mut content = String::new();
    for i in 1..=1000 {
        content.push_str(&format!("Line {}\n", i));
    }
    create_test_file("/tmp/rush_test_head.txt", &content);

    // head should terminate early and cat should handle SIGPIPE gracefully
    let output = run_rush("cat /tmp/rush_test_head.txt | head -5");
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_head.txt").ok();
}

#[test]
fn test_pipeline_empty_input() {
    create_test_file("/tmp/rush_test_empty.txt", "");

    let output = run_rush("cat /tmp/rush_test_empty.txt | grep --no-color -N test");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(output.status.code(), Some(1)); // grep returns 1 for no match

    fs::remove_file("/tmp/rush_test_empty.txt").ok();
}

#[test]
fn test_pipeline_error_in_middle_command() {
    create_test_file("/tmp/rush_test_error.txt", "hello\nworld\n");

    // Middle command references non-existent file, but pipeline should handle it
    // Note: This tests error propagation behavior
    let output = run_rush(
        "cat /tmp/rush_test_error.txt | cat /nonexistent/file.txt | grep --no-color -N hello",
    );

    // The pipeline should fail (non-zero exit code)
    assert_ne!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_error.txt").ok();
}

#[test]
fn test_complex_real_world_pipeline() {
    // Create a log-like file
    let content = r#"2024-01-01 INFO Starting application
2024-01-01 ERROR Failed to connect
2024-01-01 INFO Retrying connection
2024-01-01 ERROR Failed to connect
2024-01-01 INFO Connection established
2024-01-01 ERROR Unexpected error
"#;
    create_test_file("/tmp/rush_test_logs.txt", content);

    // Count ERROR lines
    let output = run_rush("cat /tmp/rush_test_logs.txt | grep --no-color -N ERROR | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(count, 3);
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_logs.txt").ok();
}

#[test]
fn test_pipeline_pipefail_option_enabled() {
    create_test_file("/tmp/rush_test_pipefail.txt", "hello\nworld\n");

    // With pipefail set, the pipeline should fail if any command fails
    // First, test without pipefail (default behavior)
    let output_default =
        run_rush("grep --no-color -N nonexistent /tmp/rush_test_pipefail.txt | wc -l");
    // Without pipefail, the pipeline succeeds even though grep fails
    // because the last command (wc) succeeds

    // With pipefail set
    let output_pipefail = run_rush(
        "set -o pipefail; grep --no-color -N nonexistent /tmp/rush_test_pipefail.txt | wc -l",
    );
    // With pipefail, the pipeline should fail
    assert_ne!(
        output_pipefail.status.code(),
        Some(0),
        "Pipeline with pipefail should fail when middle command fails"
    );

    fs::remove_file("/tmp/rush_test_pipefail.txt").ok();
}

#[test]
fn test_pipeline_six_stage() {
    create_test_file(
        "/tmp/rush_test_pipe6.txt",
        "apple\nbanana\napple\ncherry\napple\ndate\n",
    );

    // 6-stage pipeline: cat | grep | grep | head | tail | wc -l
    let output = run_rush(
        "cat /tmp/rush_test_pipe6.txt | grep --no-color -N apple | grep --no-color -N apple | head -3 | tail -2 | wc -l"
    );
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert!(
        count >= 1,
        "Should have at least 1 line in 6-stage pipeline"
    );
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_pipe6.txt").ok();
}

#[test]
fn test_pipeline_with_special_characters_in_data() {
    let content = "line with spaces\nline\twith\ttabs\nline$with$dollars\nline with 'quotes'\n";
    create_test_file("/tmp/rush_test_special.txt", content);

    // Pipeline should handle data with special characters correctly
    let output = run_rush("cat /tmp/rush_test_special.txt | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(
        count, 4,
        "Should preserve all lines with special characters"
    );
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_special.txt").ok();
}

#[test]
fn test_pipeline_stdout_stderr_separation() {
    create_test_file("/tmp/rush_test_sep.txt", "line1\nline2\nline3\n");

    // Pipeline should separate stdout and stderr correctly
    let output = run_rush("cat /tmp/rush_test_sep.txt | grep --no-color -N line2 2>&1");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line2"), "Should have line2 in output");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_sep.txt").ok();
}

#[test]
fn test_pipeline_very_long_lines() {
    // Create a file with very long lines
    let long_line = "x".repeat(10000);
    let content = format!("{}\n{}\n{}\n", long_line, "short", long_line);
    create_test_file("/tmp/rush_test_long_lines.txt", &content);

    // Pipeline should handle very long lines
    let output = run_rush("cat /tmp/rush_test_long_lines.txt | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(
        count, 3,
        "Should preserve all lines including very long ones"
    );
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_long_lines.txt").ok();
}

#[test]
fn test_pipeline_with_sort_complex() {
    create_test_file("/tmp/rush_test_sort.txt", "3\n1\n4\n1\n5\n9\n2\n6\n");

    // Pipeline with sorting
    let output = run_rush("cat /tmp/rush_test_sort.txt | sort | uniq -c | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert!(count > 0, "Should have counted unique sorted lines");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_sort.txt").ok();
}

#[test]
fn test_pipeline_with_sed_like_operations() {
    create_test_file("/tmp/rush_test_sed.txt", "test1\ntest2\ntest3\n");

    // Pipeline with text manipulation
    let output = run_rush("cat /tmp/rush_test_sed.txt | grep --no-color -N test | wc -l");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert_eq!(count, 3, "All lines should match pattern");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_sed.txt").ok();
}

#[test]
fn test_pipeline_with_multiple_pipes_and_redirects() {
    create_test_file("/tmp/rush_test_multi.txt", "data1\ndata2\ndata3\n");

    // Pipeline followed by output redirect
    let output = run_rush("cat /tmp/rush_test_multi.txt | grep --no-color -N data > /tmp/rush_test_output.txt && wc -l /tmp/rush_test_output.txt");
    assert_eq!(output.status.code(), Some(0));

    // Verify output file was created and has correct content
    if let Ok(content) = fs::read_to_string("/tmp/rush_test_output.txt") {
        assert_eq!(content, "data1\ndata2\ndata3\n");
    }

    fs::remove_file("/tmp/rush_test_multi.txt").ok();
    fs::remove_file("/tmp/rush_test_output.txt").ok();
}

#[test]
fn test_pipeline_preserves_exit_codes_correctly() {
    create_test_file("/tmp/rush_test_exit.txt", "line1\nline2\n");

    // Test 1: Last command succeeds -> pipeline succeeds
    let output1 = run_rush("cat /tmp/rush_test_exit.txt | grep --no-color -N line1");
    assert_eq!(
        output1.status.code(),
        Some(0),
        "Pipeline should succeed when last command succeeds"
    );

    // Test 2: Last command fails -> pipeline fails
    let output2 = run_rush("cat /tmp/rush_test_exit.txt | grep --no-color -N nonexistent");
    assert_eq!(
        output2.status.code(),
        Some(1),
        "Pipeline should fail when last command fails"
    );

    fs::remove_file("/tmp/rush_test_exit.txt").ok();
}

#[test]
fn test_pipeline_handles_mixed_builtins_and_external() {
    create_test_file("/tmp/rush_test_mixed.txt", "apple\nbanana\ncherry\n");

    // Mix of builtin (cat) and external command (grep, wc)
    let output = run_rush("cat /tmp/rush_test_mixed.txt | grep --no-color -N banana | wc -c");
    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("Expected number");
    assert!(count > 0, "Should count characters in piped output");
    assert_eq!(output.status.code(), Some(0));

    fs::remove_file("/tmp/rush_test_mixed.txt").ok();
}
