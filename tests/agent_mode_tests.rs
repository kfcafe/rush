/// Tests for RUSH_AGENT_MODE — automatic JSON output and ANSI stripping.
use rush::{run, RunOptions};

fn agent_opts() -> RunOptions {
    RunOptions {
        json_output: true,
        ..Default::default()
    }
}

/// In agent mode, `ls /tmp` should produce valid JSON output.
#[test]
fn test_agent_mode_ls_json() {
    let result = run("ls /tmp", &agent_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    // Output should parse as valid JSON (array of file entries).
    let parsed: serde_json::Value =
        serde_json::from_str(result.stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "Expected valid JSON from ls in agent mode, got error: {}\nstdout: {:?}",
                e, result.stdout
            )
        });
    assert!(parsed.is_array(), "ls JSON output should be an array");
}

/// The run() API with json_output=true should set agent_mode on the runtime.
#[test]
fn test_agent_mode_via_run_api() {
    let result = run("ls /tmp", &agent_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    // Same as above — valid JSON proves agent_mode was activated.
    let parsed: serde_json::Value =
        serde_json::from_str(result.stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "Expected valid JSON via run API, got error: {}\nstdout: {:?}",
                e, result.stdout
            )
        });
    assert!(parsed.is_array());
}

/// In agent mode, output should contain no ANSI escape sequences.
#[test]
fn test_agent_mode_no_ansi() {
    // ls with colors would normally include ANSI codes.
    let result = run("ls /tmp", &agent_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(
        !result.stdout.contains("\x1b["),
        "Agent mode output should not contain ANSI escape codes, got: {:?}",
        &result.stdout[..result.stdout.len().min(200)]
    );
}

/// External commands still work in agent mode — output is plain text (ANSI-stripped).
#[test]
fn test_agent_mode_external_passthrough() {
    let result = run("/bin/echo external_works", &agent_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("external_works"),
        "External command output should pass through, got: {:?}",
        result.stdout
    );
}

/// echo is not a JSON-capable command — it should return plain text unchanged.
#[test]
fn test_agent_mode_echo_unchanged() {
    let result = run("echo hello", &agent_opts()).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("hello"),
        "echo should return plain text, got: {:?}",
        result.stdout
    );
    // Should NOT be wrapped in JSON.
    assert!(
        !result.stdout.trim().starts_with('[') && !result.stdout.trim().starts_with('{'),
        "echo output should not be JSON-wrapped, got: {:?}",
        result.stdout
    );
}
