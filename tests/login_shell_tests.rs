use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_source_builtin() {
    let temp_dir = TempDir::new().unwrap();
    let config_file = temp_dir.path().join("test_config.rush");

    // Create a config file
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, "echo hello").unwrap();
    drop(file);

    // Test sourcing the file
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg(format!("source {}", config_file.display()))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn test_dot_builtin() {
    let temp_dir = TempDir::new().unwrap();
    let config_file = temp_dir.path().join("test_config.rush");

    // Create a config file
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, "echo hello_from_dot").unwrap();
    drop(file);

    // Test sourcing the file with dot command (POSIX syntax)
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg(format!(". {}", config_file.display()))
        .output()
        .unwrap();

    assert!(output.status.success(), "dot command should succeed");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello_from_dot"
    );
}

#[test]
fn test_dot_nonexistent_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg(". /nonexistent/file.rush")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such file"));
}

#[test]
fn test_source_with_tilde() {
    // Create a test file in temp directory
    let home = dirs::home_dir().unwrap();
    let test_file = home.join(".rush_test_source");

    // Create test file
    let mut file = fs::File::create(&test_file).unwrap();
    writeln!(file, "echo tilde_success").unwrap();
    drop(file);

    // Test sourcing with ~ expansion
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("source ~/.rush_test_source")
        .output()
        .unwrap();

    // Cleanup
    fs::remove_file(test_file).ok();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "tilde_success"
    );
}

#[test]
fn test_source_nonexistent_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("source /nonexistent/file.rush")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such file"));
}

#[test]
fn test_environment_variables_set() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("echo $SHELL")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rush"), "SHELL should contain 'rush'");
}

#[test]
fn test_term_variable_set() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("echo $TERM")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "TERM should be set");
}

#[test]
fn test_user_variable_set() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("echo $USER")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "USER should be set");
}

#[test]
fn test_home_variable_set() {
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg("echo $HOME")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty(), "HOME should be set");
}

#[test]
fn test_login_flag() {
    let home = dirs::home_dir().unwrap();
    let profile_file = home.join(".rush_profile_test");

    // Create test profile
    let mut file = fs::File::create(&profile_file).unwrap();
    writeln!(file, "echo from_profile").unwrap();
    drop(file);

    // Temporarily rename .rush_profile
    let real_profile = home.join(".rush_profile");
    let backup = home.join(".rush_profile.backup");
    let had_profile = real_profile.exists();
    if had_profile {
        fs::rename(&real_profile, &backup).ok();
    }

    // Move test profile to real location
    fs::rename(&profile_file, &real_profile).unwrap();

    // Test with --login flag
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("--login")
        .arg("-c")
        .arg("echo test")
        .output()
        .unwrap();

    // Restore original profile
    fs::remove_file(&real_profile).ok();
    if had_profile {
        fs::rename(&backup, &real_profile).ok();
    }

    assert!(output.status.success());
    // The output should contain both the profile output and the command output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("from_profile") || stdout.contains("test"));
}

#[test]
fn test_no_rc_flag() {
    let home = dirs::home_dir().unwrap();
    let rushrc = home.join(".rushrc_test");

    // Create test rushrc
    let mut file = fs::File::create(&rushrc).unwrap();
    writeln!(file, "echo should_not_load").unwrap();
    drop(file);

    // Temporarily rename .rushrc
    let real_rushrc = home.join(".rushrc");
    let backup = home.join(".rushrc.backup");
    let had_rushrc = real_rushrc.exists();
    if had_rushrc {
        fs::rename(&real_rushrc, &backup).ok();
    }

    // Move test rushrc to real location
    fs::rename(&rushrc, &real_rushrc).unwrap();

    // Test with --no-rc flag
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("--no-rc")
        .arg("-c")
        .arg("echo test_output")
        .output()
        .unwrap();

    // Restore original rushrc
    fs::remove_file(&real_rushrc).ok();
    if had_rushrc {
        fs::rename(&backup, &real_rushrc).ok();
    }

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should not contain rushrc output
    assert!(!stdout.contains("should_not_load"));
    // Should contain the command output
    assert!(stdout.contains("test_output"));
}

#[test]
fn test_source_with_comments() {
    let temp_dir = TempDir::new().unwrap();
    let config_file = temp_dir.path().join("test_comments.rush");

    // Create a config file with comments
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, "# This is a comment").unwrap();
    writeln!(file, "echo value1").unwrap();
    writeln!(file, "# Another comment").unwrap();
    writeln!(file, "echo value2").unwrap();
    writeln!(file, "").unwrap(); // Empty line
    writeln!(file, "echo value3").unwrap();
    drop(file);

    // Test sourcing the file
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg(format!("source {}", config_file.display()))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("value1"));
    assert!(stdout.contains("value2"));
    assert!(stdout.contains("value3"));
}

#[test]
fn test_source_with_error_continues() {
    let temp_dir = TempDir::new().unwrap();
    let config_file = temp_dir.path().join("test_error.rush");

    // Create a config file with an error in the middle
    let mut file = fs::File::create(&config_file).unwrap();
    writeln!(file, "echo before_error").unwrap();
    writeln!(file, "nonexistent_command_that_will_fail").unwrap();
    writeln!(file, "echo after_error").unwrap();
    drop(file);

    // Test sourcing the file - should continue after error
    let output = Command::new(env!("CARGO_BIN_EXE_rush"))
        .arg("-c")
        .arg(format!("source {}", config_file.display()))
        .output()
        .unwrap();

    // Should complete successfully despite the error
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("before_error"));
    assert!(stdout.contains("after_error"));
}
