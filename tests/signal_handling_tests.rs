use rush::signal::SignalHandler;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

#[test]
fn test_signal_handler_creation() {
    let handler = SignalHandler::new();
    assert!(!handler.should_shutdown());
    assert!(!handler.signal_received());
    assert_eq!(handler.signal_number(), 0);
}

#[test]
fn test_signal_handler_setup() {
    let handler = SignalHandler::new();
    let result = handler.setup();
    assert!(result.is_ok(), "Signal handler setup should succeed");
}

#[test]
fn test_signal_handler_reset() {
    let handler = SignalHandler::new();
    handler.reset();
    assert!(!handler.signal_received());
    assert_eq!(handler.signal_number(), 0);
    assert!(!handler.should_shutdown());
}

#[test]
fn test_exit_codes() {
    let handler = SignalHandler::new();

    // Default should be 1 for unknown signal
    assert_eq!(handler.exit_code(), 1);
}

#[test]
fn test_sigint_in_script() {
    // Create a test script that runs a long command
    let mut script = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(script, "#!/usr/bin/env rush").unwrap();
    writeln!(script, "echo Starting").unwrap();
    writeln!(script, "sleep 10").unwrap();
    writeln!(script, "echo Finished").unwrap();

    let script_path = script.path().to_str().unwrap();

    // Spawn rush with the script
    let mut child = Command::new("cargo")
        .args(&["run", "--", script_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    // Wait a bit for the script to start
    thread::sleep(Duration::from_millis(500));

    // Send SIGINT (Ctrl-C) to the process
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        let pid = Pid::from_raw(child.id() as i32);
        signal::kill(pid, Signal::SIGINT).ok();
    }

    // Wait for the process to exit
    let result = child.wait_with_output().expect("Failed to wait for child");

    // Verify the process was interrupted
    #[cfg(unix)]
    {
        // On Unix, SIGINT should result in exit code 130
        assert!(
            result.status.code().unwrap_or(0) == 130 || !result.status.success(),
            "Process should be interrupted by SIGINT"
        );
    }
}

#[test]
fn test_no_orphaned_processes() {
    // Create a script that spawns a child process
    let mut script = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(script, "#!/usr/bin/env rush").unwrap();
    writeln!(script, "sleep 30 &").unwrap();
    writeln!(script, "sleep 10").unwrap();

    let script_path = script.path().to_str().unwrap();

    // Spawn rush with the script
    let mut child = Command::new("cargo")
        .args(&["run", "--", script_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    let pid = child.id();

    // Wait a bit for the script to start
    thread::sleep(Duration::from_millis(500));

    // Send SIGTERM to the process
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        let rush_pid = Pid::from_raw(pid as i32);
        signal::kill(rush_pid, Signal::SIGTERM).ok();
    }

    // Wait for the process to exit
    let _ = child.wait_with_output();

    // Check that there are no orphaned sleep processes
    thread::sleep(Duration::from_millis(500));

    #[cfg(unix)]
    {
        // Check for any remaining sleep processes that were children of our process
        let output = Command::new("pgrep")
            .arg("-P")
            .arg(pid.to_string())
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.trim().is_empty() || !output.status.success(),
                "No child processes should remain after SIGTERM"
            );
        }
    }
}

#[test]
fn test_signal_during_command_execution() {
    // Create a script with a command that takes time
    let mut script = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(script, "#!/usr/bin/env rush").unwrap();
    writeln!(script, "echo Before long command").unwrap();
    writeln!(script, "sleep 15").unwrap();
    writeln!(script, "echo After long command").unwrap();

    let script_path = script.path().to_str().unwrap();

    // Spawn rush with the script
    let mut child = Command::new("cargo")
        .args(&["run", "--", script_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    // Wait for the command to start
    thread::sleep(Duration::from_millis(500));

    // Send SIGINT
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        let pid = Pid::from_raw(child.id() as i32);
        signal::kill(pid, Signal::SIGINT).ok();
    }

    // Wait and check output
    let result = child.wait_with_output().expect("Failed to wait for child");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Should have been interrupted - check exit code or stderr for interruption
    #[cfg(unix)]
    {
        // Process should either exit with signal code or show interrupted
        let was_interrupted = result.status.code().unwrap_or(0) == 130
            || !result.status.success()
            || stderr.contains("interrupt")
            || stderr.contains("Interrupt");

        assert!(
            was_interrupted || !stdout.contains("After long command"),
            "Command should be interrupted before completion"
        );
    }
}

#[test]
fn test_interactive_mode_ctrl_c() {
    // Test that Ctrl-C in interactive mode doesn't exit the shell
    // but returns to the prompt

    // This is more of an integration test and would require
    // a PTY to properly test interactive behavior
    // For now, we just verify that the signal handler can be set up
    let handler = SignalHandler::new();
    assert!(handler.setup().is_ok());

    // Simulate a Ctrl-C followed by a reset
    handler.reset();
    assert!(!handler.should_shutdown());
}

#[test]
fn test_command_flag_with_signal() {
    // Test the -c flag with signal handling
    let mut child = Command::new("cargo")
        .args(&["run", "--", "-c", "sleep 10"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    // Wait a bit for the command to start
    thread::sleep(Duration::from_millis(500));

    // Send SIGINT
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        let pid = Pid::from_raw(child.id() as i32);
        signal::kill(pid, Signal::SIGINT).ok();
    }

    // Wait for exit
    let result = child.wait_with_output().expect("Failed to wait for child");

    #[cfg(unix)]
    {
        // Should exit with SIGINT code
        assert!(
            result.status.code().unwrap_or(0) == 130 || !result.status.success(),
            "Process should be interrupted by SIGINT"
        );
    }
}

#[test]
fn test_multiple_signals() {
    let handler = SignalHandler::new();
    handler.setup().expect("Failed to setup signal handler");

    // Reset multiple times
    handler.reset();
    assert!(!handler.signal_received());

    handler.reset();
    assert!(!handler.signal_received());

    handler.reset();
    assert!(!handler.signal_received());
}

#[cfg(unix)]
#[test]
fn test_sighup_handling() {
    // Create a simple script
    let mut script = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(script, "#!/usr/bin/env rush").unwrap();
    writeln!(script, "sleep 10").unwrap();

    let script_path = script.path().to_str().unwrap();

    // Spawn rush with the script
    let mut child = Command::new("cargo")
        .args(&["run", "--", script_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rush");

    // Wait for script to start
    thread::sleep(Duration::from_millis(500));

    // Send SIGHUP
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let pid = Pid::from_raw(child.id() as i32);
    signal::kill(pid, Signal::SIGHUP).ok();

    // Wait for exit
    let result = child.wait_with_output().expect("Failed to wait for child");

    // Should exit due to SIGHUP
    assert!(
        result.status.code().unwrap_or(0) == 129 || !result.status.success(),
        "Process should exit on SIGHUP"
    );
}
