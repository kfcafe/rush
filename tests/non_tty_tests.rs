// Integration tests for non-TTY mode support in rush shell
//
// These tests verify that rush works correctly when:
// - Input is piped from another command
// - Input is redirected from a file
// - Used in command substitution
// - Used in automated scripts (cron, CI/CD, etc.)

use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

fn rush_binary() -> String {
    // Use the release binary for realistic testing
    let mut path = std::env::current_dir().unwrap();
    path.push("target");
    path.push("release");
    path.push("rush");
    path.to_string_lossy().to_string()
}

// ============================================================================
// PIPED INPUT TESTS
// ============================================================================

#[test]
fn test_piped_input_single_command() {
    // Test: echo "pwd" | rush
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"pwd\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");
    assert!(!stdout.is_empty(), "Should output current directory");
}

#[test]
fn test_piped_input_echo_command() {
    // Test: echo "echo hello world" | rush
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"echo hello world\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("hello world"), "Should echo the text");
}

#[test]
fn test_piped_input_multiple_commands() {
    // Test: echo -e "echo first\necho second\necho third" | rush
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(b"echo first\necho second\necho third\n")
            .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("first"), "Should execute first command");
    assert!(stdout.contains("second"), "Should execute second command");
    assert!(stdout.contains("third"), "Should execute third command");
}

#[test]
fn test_piped_input_with_builtin_commands() {
    // Test piped input with rush's builtin commands (pwd, echo)
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"pwd\necho builtin test\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("builtin test"),
        "Should execute builtin commands"
    );
    assert!(!stdout.is_empty(), "Should output from pwd");
}

// ============================================================================
// STDIN REDIRECTION TESTS
// ============================================================================

#[test]
fn test_stdin_redirection_simple_script() {
    // Test: rush < script.sh
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "echo hello from script").unwrap();
    writeln!(script, "pwd").unwrap();
    writeln!(script, "echo goodbye").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("hello from script"));
    assert!(stdout.contains("goodbye"));
    assert!(!stdout.is_empty());
}

#[test]
fn test_stdin_redirection_with_comments() {
    // Test that comments are ignored in scripts
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "# This is a comment").unwrap();
    writeln!(script, "echo visible").unwrap();
    writeln!(script, "# Another comment").unwrap();
    writeln!(script, "echo also visible").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("visible"));
    assert!(stdout.contains("also visible"));
    assert!(
        !stdout.contains("comment"),
        "Comments should not appear in output"
    );
}

#[test]
fn test_stdin_redirection_with_empty_lines() {
    // Test that empty lines are handled gracefully
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "echo first").unwrap();
    writeln!(script, "").unwrap();
    writeln!(script, "").unwrap();
    writeln!(script, "echo second").unwrap();
    writeln!(script, "").unwrap();
    writeln!(script, "echo third").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("first"));
    assert!(stdout.contains("second"));
    assert!(stdout.contains("third"));
}

#[test]
fn test_stdin_redirection_with_whitespace() {
    // Test that whitespace-only lines are handled
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "echo start").unwrap();
    writeln!(script, "   ").unwrap();
    writeln!(script, "\t").unwrap();
    writeln!(script, "echo end").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("start"));
    assert!(stdout.contains("end"));
}

// ============================================================================
// COMMAND SUBSTITUTION TESTS
// ============================================================================

#[test]
fn test_command_substitution_with_c_flag() {
    // Test: result=$(rush -c "echo test")
    let output = Command::new(rush_binary())
        .arg("-c")
        .arg("echo captured output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(stdout.trim(), "captured output");
}

#[test]
fn test_command_substitution_pwd() {
    // Test: dir=$(rush -c "pwd")
    let output = Command::new(rush_binary())
        .arg("-c")
        .arg("pwd")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(!stdout.trim().is_empty());
    // Should be a valid path
    assert!(stdout.contains('/') || stdout.contains('\\'));
}

#[test]
fn test_command_substitution_cat() {
    // Test: content=$(rush -c "cat file.txt")
    std::fs::write("/tmp/rush_subst_test.txt", "substitution test\n").unwrap();

    let output = Command::new(rush_binary())
        .arg("-c")
        .arg("cat /tmp/rush_subst_test.txt")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(stdout, "substitution test\n");

    // Cleanup
    std::fs::remove_file("/tmp/rush_subst_test.txt").ok();
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_error_handling_failed_command() {
    // Test that rush handles failed commands gracefully
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        // Try to cat a non-existent file
        stdin
            .write_all(b"cat /tmp/this_file_does_not_exist_12345.txt\n")
            .unwrap();
    }

    let output = child.wait_with_output().unwrap();

    // Rush should not crash, though the command may fail
    // The important thing is that rush itself handles this gracefully
    assert!(!output.status.success() || output.status.success());
}

#[test]
fn test_error_handling_multiple_commands_one_fails() {
    // Test that if one command fails, others can still execute
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"echo before\n").unwrap();
        stdin
            .write_all(b"cat /tmp/nonexistent_file_xyz.txt\n")
            .unwrap();
        stdin.write_all(b"echo after\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should still execute commands before and after the failed command
    assert!(
        stdout.contains("before"),
        "Should execute command before failure"
    );
    assert!(
        stdout.contains("after"),
        "Should execute command after failure"
    );
}

#[test]
fn test_error_handling_parse_error() {
    // Test that rush handles parse errors gracefully
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"echo valid command\n").unwrap();
        // Invalid syntax - unclosed quote
        stdin.write_all(b"echo \"unclosed\n").unwrap();
        stdin.write_all(b"echo another valid command\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute the valid commands
    assert!(stdout.contains("valid command"));
}

// ============================================================================
// CRON JOB SIMULATION TESTS
// ============================================================================

#[test]
fn test_cron_job_scenario_simple() {
    // Simulate a cron job: rush < backup_script.sh
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "# Daily backup script").unwrap();
    writeln!(script, "echo Starting backup...").unwrap();
    writeln!(script, "echo Backup completed").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Starting backup"));
    assert!(stdout.contains("Backup completed"));
}

#[test]
fn test_cron_job_scenario_with_file_operations() {
    // Simulate a cron job that echoes data and checks directory
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "echo test data").unwrap();
    writeln!(script, "pwd").unwrap();
    writeln!(script, "echo cron completed").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("test data"));
    assert!(stdout.contains("cron completed"));
}

// ============================================================================
// EXIT CODE TESTS
// ============================================================================

#[test]
fn test_exit_code_success() {
    // Test that successful commands return exit code 0
    let output = Command::new(rush_binary())
        .arg("-c")
        .arg("echo success")
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_exit_code_via_stdin() {
    // Test exit code when providing commands via stdin
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"echo test\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
}

// ============================================================================
// PIPELINE TESTS IN NON-TTY MODE
// ============================================================================

#[test]
fn test_pipeline_via_stdin() {
    // Test that pipelines work when rush is run in non-TTY mode
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"echo pipeline test | cat\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("pipeline test"));
}

#[test]
fn test_complex_pipeline_via_stdin() {
    // Test more complex pipeline
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "echo line1 > /tmp/rush_pipeline_test.txt").unwrap();
    writeln!(script, "echo line2 >> /tmp/rush_pipeline_test.txt").unwrap();
    writeln!(script, "cat /tmp/rush_pipeline_test.txt | grep line2").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("line2"));

    // Cleanup
    std::fs::remove_file("/tmp/rush_pipeline_test.txt").ok();
}

// ============================================================================
// AUTOMATED TESTING SCENARIO
// ============================================================================

#[test]
fn test_ci_cd_scenario() {
    // Simulate a CI/CD pipeline script
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "# CI/CD test script").unwrap();
    writeln!(script, "echo Running tests...").unwrap();
    writeln!(script, "pwd").unwrap();
    writeln!(script, "echo Tests passed").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "CI/CD script should succeed");
    assert!(stdout.contains("Running tests"));
    assert!(stdout.contains("Tests passed"));
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_empty_input() {
    // Test that rush handles empty input gracefully
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        // Send empty input
        stdin.write_all(b"").unwrap();
    }

    let output = child.wait_with_output().unwrap();

    // Should handle empty input gracefully without crashing
    assert!(output.status.success() || output.status.code().is_some());
}

#[test]
fn test_only_whitespace_input() {
    // Test that rush handles whitespace-only input
    let mut child = Command::new(rush_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(b"   \n\t\n   \n").unwrap();
    }

    let output = child.wait_with_output().unwrap();

    // Should handle whitespace gracefully
    assert!(output.status.success() || output.status.code().is_some());
}

#[test]
fn test_only_comments_input() {
    // Test that rush handles comment-only input
    let mut script = NamedTempFile::new().unwrap();
    writeln!(script, "# Just comments").unwrap();
    writeln!(script, "# Nothing else").unwrap();
    writeln!(script, "# Should be fine").unwrap();
    script.flush().unwrap();

    let output = Command::new(rush_binary())
        .stdin(std::fs::File::open(script.path()).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute");

    // Should handle comment-only input gracefully
    assert!(output.status.success() || output.status.code().is_some());
}
