/// Integration tests for rush::run() one-shot programmatic API.
use rush::{run, RunOptions};
use std::collections::HashMap;
use std::fs;

fn default_opts() -> RunOptions {
    RunOptions::default()
}

#[test]
fn test_run_one_shot_echo() {
    let result = run("echo hello", &default_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("hello"),
        "stdout: {:?}",
        result.stdout
    );
    assert!(!result.timed_out);
    assert!(!result.truncated);
}

#[test]
fn test_run_one_shot_exit_code() {
    let result = run("exit 42", &default_opts()).unwrap();
    assert_eq!(result.exit_code, 42);
}

#[test]
fn test_run_one_shot_builtin_ls() {
    let result = run("ls /tmp", &default_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(!result.stdout.is_empty(), "ls /tmp should produce output");
}

#[test]
fn test_run_one_shot_pipeline() {
    let result = run("echo foo | cat", &default_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("foo"), "stdout: {:?}", result.stdout);
}

#[test]
fn test_run_one_shot_custom_cwd() {
    let tmp = tempdir();
    let sentinel = tmp.join("sentinel_file.txt");
    fs::write(&sentinel, "hello").unwrap();

    let opts = RunOptions {
        cwd: Some(tmp.clone()),
        ..Default::default()
    };

    let result = run("ls", &opts).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("sentinel_file.txt"),
        "Expected sentinel_file.txt in ls output, got: {:?}",
        result.stdout
    );

    // Clean up.
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_run_one_shot_timeout() {
    let opts = RunOptions {
        timeout: Some(1),
        ..Default::default()
    };
    let result = run("sleep 60", &opts).unwrap();
    assert!(result.timed_out, "expected timed_out=true");
    assert_eq!(result.exit_code, 124);
}

#[test]
fn test_run_one_shot_max_output() {
    // Generate a large output: 100 lines of 80 chars each = ~8100 bytes.
    let cmd = r#"printf '%0.s----------------------------------------------------------------\n' $(seq 1 100)"#;
    let opts = RunOptions {
        max_output_bytes: Some(256),
        ..Default::default()
    };
    let result = run(cmd, &opts).unwrap();
    assert!(result.truncated, "expected truncated=true");
    assert!(
        result.stdout.len() <= 256,
        "stdout len {} exceeds max 256",
        result.stdout.len()
    );
}

/// Create a temporary directory with a unique name and return its path.
fn tempdir() -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let path = std::env::temp_dir().join(format!("rush_run_api_test_{}", nanos));
    fs::create_dir_all(&path).unwrap();
    path
}
