/// Tests for shell-level output budget (--max-output / RUSH_MAX_OUTPUT / RunOptions::max_output_bytes).
use rush::{run, RunOptions};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_with_budget(cmd: &str, max_bytes: usize) -> rush::RunResult {
    let opts = RunOptions {
        max_output_bytes: Some(max_bytes),
        ..Default::default()
    };
    run(cmd, &opts).expect("run() should not fail")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Produce ~10KB of output with a 1024-byte budget. Output should be truncated
/// and the truncated flag should be set.
#[test]
fn test_output_budget_basic() {
    // 200 lines × ~65 bytes each ≈ 13,000 bytes — well over 1024.
    let result = run_with_budget(
        r#"printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n%.0s' $(seq 1 200)"#,
        1024,
    );

    assert!(result.truncated, "expected truncated=true");
    // The budget is 1024 bytes, but the truncation notice adds a few dozen bytes on top.
    // We allow up to 1024 + 100 to accommodate the notice.
    assert!(
        result.stdout.len() <= 1024 + 100,
        "stdout len {} is too large (budget=1024)",
        result.stdout.len()
    );
}

/// Produce only ~100 bytes with a 1024-byte budget. Nothing should be truncated.
#[test]
fn test_output_budget_no_truncation() {
    let result = run_with_budget("echo hello", 1024);

    assert!(
        !result.truncated,
        "expected truncated=false for small output"
    );
    assert!(
        result.stdout.contains("hello"),
        "stdout should contain 'hello', got: {:?}",
        result.stdout
    );
}

/// The RUSH_MAX_OUTPUT environment variable should act as a default budget when
/// RunOptions::max_output_bytes is not set.
#[test]
fn test_output_budget_via_env() {
    // 200 lines × ~65 bytes each ≈ 13,000 bytes.
    let opts = RunOptions {
        env: Some({
            let mut m = std::collections::HashMap::new();
            m.insert("RUSH_MAX_OUTPUT".to_string(), "512".to_string());
            m
        }),
        ..Default::default()
    };
    // Note: the env var is read from the *process* environment, not from RunOptions::env,
    // because execute_inner calls std::env::var directly. So we set it on the process here.
    // (RunOptions::env is for variables passed *into* the child shell environment.)
    std::env::set_var("RUSH_MAX_OUTPUT", "512");
    let result = run(
        r#"printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n%.0s' $(seq 1 200)"#,
        &RunOptions::default(),
    )
    .expect("run() should not fail");
    std::env::remove_var("RUSH_MAX_OUTPUT");

    assert!(
        result.truncated,
        "expected truncated=true via RUSH_MAX_OUTPUT env var"
    );
    assert!(
        result.stdout.len() <= 512 + 100,
        "stdout len {} exceeds budget 512 (+ notice)",
        result.stdout.len()
    );
}

/// When output is truncated, the truncation notice must be present in stdout.
#[test]
fn test_output_budget_truncation_notice() {
    let result = run_with_budget(
        r#"printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n%.0s' $(seq 1 200)"#,
        512,
    );

    assert!(result.truncated, "expected truncated=true");
    assert!(
        result.stdout.contains("[Output truncated:"),
        "truncation notice missing from stdout: {:?}",
        &result.stdout[result.stdout.len().saturating_sub(200)..]
    );
    assert!(
        result.stdout.contains("limit 512 bytes"),
        "truncation notice should mention limit, got: {:?}",
        &result.stdout[result.stdout.len().saturating_sub(200)..]
    );
}

/// External commands (seq piped to other tools) should still respect the budget.
#[test]
fn test_output_budget_external_cmd() {
    // `seq 1 10000` produces ~60KB of output.
    let result = run_with_budget("seq 1 10000", 1024);

    assert!(result.truncated, "expected truncated=true for external seq");
    assert!(
        result.stdout.len() <= 1024 + 100,
        "stdout len {} exceeds budget 1024 (+ notice)",
        result.stdout.len()
    );
    // Exit code should still be 0 — the budget doesn't kill the command.
    assert_eq!(
        result.exit_code, 0,
        "external command exit code should be 0, got {}",
        result.exit_code
    );
}
