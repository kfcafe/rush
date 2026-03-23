// POSIX Compliance Test Suite Integration
//
// This test module integrates the POSIX compliance test suite
// into the Rust test infrastructure, allowing tests to be run
// with `cargo test`.
//
// The actual POSIX tests are implemented in ShellSpec format
// and located in tests/posix/shellspec/. This module provides
// a bridge between cargo test and the shell-based test suite.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Get the project root directory
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Get the Rush binary path
fn rush_binary() -> PathBuf {
    let mut path = project_root();
    path.push("target");
    path.push("release");
    path.push("rush");
    path
}

/// Build Rush in release mode if not already built
fn ensure_rush_binary() -> Result<(), String> {
    let binary = rush_binary();

    if !binary.exists() {
        eprintln!("Building Rush binary in release mode...");
        let status = Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(project_root())
            .status()
            .map_err(|e| format!("Failed to run cargo build: {}", e))?;

        if !status.success() {
            return Err("Failed to build Rush binary".to_string());
        }
    }

    Ok(())
}

/// Check if ShellSpec is installed
fn check_shellspec_installed() -> bool {
    Command::new("shellspec")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Run ShellSpec tests with given arguments
fn run_shellspec(args: &[&str]) -> Result<std::process::Output, std::io::Error> {
    let posix_dir = project_root().join("tests/posix");

    Command::new("shellspec")
        .args(args)
        .current_dir(posix_dir)
        .env("RUSH_BINARY", rush_binary())
        .output()
}

#[test]
fn test_posix_compliance_suite_available() {
    // This test verifies that the POSIX test suite structure exists
    let posix_dir = project_root().join("tests/posix");
    assert!(posix_dir.exists(), "POSIX test directory should exist");

    let run_script = posix_dir.join("run_tests.sh");
    assert!(run_script.exists(), "Test runner script should exist");

    let shellspec_dir = posix_dir.join("shellspec");
    assert!(
        shellspec_dir.exists(),
        "ShellSpec test directory should exist"
    );

    // Check for expected test files
    let test_files = [
        "builtins_spec.sh",
        "control_flow_spec.sh",
        "redirection_spec.sh",
        "variables_spec.sh",
        "pipelines_spec.sh",
        "signals_spec.sh",
        "functions_spec.sh",
    ];

    for file in &test_files {
        let path = shellspec_dir.join(file);
        assert!(path.exists(), "Test file {} should exist", file);
    }
}

#[test]
fn test_posix_builtins() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX builtin tests");
        eprintln!("To install: curl -fsSL https://git.io/shellspec | sh");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/builtins_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX builtin tests failed");
    }
}

#[test]
fn test_posix_control_flow() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX control flow tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/control_flow_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX control flow tests failed");
    }
}

#[test]
fn test_posix_redirection() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX redirection tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/redirection_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX redirection tests failed");
    }
}

#[test]
fn test_posix_variables() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX variable tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/variables_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX variable tests failed");
    }
}

#[test]
fn test_posix_pipelines() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX pipeline tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/pipelines_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX pipeline tests failed");
    }
}

#[test]
fn test_posix_signals() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX signal tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/signals_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX signal tests failed");
    }
}

#[test]
fn test_posix_functions() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping POSIX function tests");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let output = run_shellspec(&["shellspec/functions_spec.sh", "--format", "tap"])
        .expect("Failed to run ShellSpec tests");

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("POSIX function tests failed");
    }
}

#[test]
#[ignore] // This is a slow comprehensive test, run with --ignored
fn test_posix_full_suite() {
    if !check_shellspec_installed() {
        eprintln!("ShellSpec not installed, skipping full POSIX test suite");
        eprintln!("To install: curl -fsSL https://git.io/shellspec | sh");
        return;
    }

    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    eprintln!("\n=== Running Full POSIX Compliance Test Suite ===\n");

    let output =
        run_shellspec(&["--format", "documentation"]).expect("Failed to run ShellSpec tests");

    // Always print output for comprehensive test
    println!("{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        panic!("\nFull POSIX test suite failed. See output above for details.");
    }
}

/// This test can be run manually to execute the shell script runner
#[test]
#[ignore] // Run manually with: cargo test --test posix_compliance_tests test_run_posix_script -- --ignored --nocapture
fn test_run_posix_script() {
    if let Err(e) = ensure_rush_binary() {
        panic!("Cannot run POSIX tests: {}", e);
    }

    let posix_dir = project_root().join("tests/posix");
    let run_script = posix_dir.join("run_tests.sh");

    eprintln!("\n=== Running POSIX Test Suite via Shell Script ===\n");

    let status = Command::new("bash")
        .arg(&run_script)
        .current_dir(&posix_dir)
        .env("RUSH_BINARY", rush_binary())
        .status()
        .expect("Failed to run test script");

    if !status.success() {
        panic!("POSIX test script failed");
    }
}
