/// Integration tests for structured error handling (rush-ai.8)
use rush::error::{should_output_json_errors, RushError};
use std::env;
use std::path::Path;

#[test]
fn test_error_json_format_env_var() {
    // Clean slate
    env::remove_var("RUSH_ERROR_FORMAT");
    assert!(!should_output_json_errors());

    // Enable JSON errors
    env::set_var("RUSH_ERROR_FORMAT", "json");
    assert!(should_output_json_errors());

    // Case insensitive
    env::set_var("RUSH_ERROR_FORMAT", "JSON");
    assert!(should_output_json_errors());

    // Disable JSON errors
    env::set_var("RUSH_ERROR_FORMAT", "text");
    assert!(!should_output_json_errors());

    // Clean up
    env::remove_var("RUSH_ERROR_FORMAT");
}

#[test]
fn test_rush_error_construction() {
    let error = RushError::new("TEST_ERROR", "test message", 1);

    assert_eq!(error.error_code, "TEST_ERROR");
    assert_eq!(error.message, "test message");
    assert_eq!(error.exit_code, 1);
    assert!(error.context.is_none());
}

#[test]
fn test_rush_error_with_context() {
    let error = RushError::new("TEST_ERROR", "test", 1)
        .with_context(serde_json::json!({"additional": "info"}));

    assert!(error.context.is_some());
    assert_eq!(error.context.unwrap()["additional"], "info");
}

#[test]
fn test_rush_error_json_serialization() {
    let error = RushError::file_not_found(Path::new("/tmp/test.txt"));
    let json = error.to_json();

    assert!(json.contains("FILE_NOT_FOUND"));
    assert!(json.contains("/tmp/test.txt"));
    assert!(json.contains("No such file or directory"));
}

#[test]
fn test_rush_error_text_output() {
    let error = RushError::file_not_found(Path::new("/tmp/test.txt"));
    let text = error.to_text();

    assert!(text.contains("/tmp/test.txt"));
    assert!(text.contains("No such file or directory"));
}

#[test]
fn test_file_not_found_constructor() {
    let error = RushError::file_not_found(Path::new("/tmp/test.txt"));

    assert_eq!(error.error_code, "FILE_NOT_FOUND");
    assert_eq!(error.exit_code, 1);
    assert!(error.message.contains("/tmp/test.txt"));
    assert!(error.message.contains("No such file or directory"));
}

#[test]
fn test_is_a_directory_constructor() {
    let error = RushError::is_a_directory(Path::new("/tmp"));

    assert_eq!(error.error_code, "IS_A_DIRECTORY");
    assert_eq!(error.exit_code, 1);
    assert!(error.message.contains("/tmp"));
    assert!(error.message.contains("Is a directory"));
}

#[test]
fn test_json_output_format() {
    let error = RushError::new("CUSTOM_ERROR", "custom message", 42);
    let json = error.to_json();

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["error_code"], "CUSTOM_ERROR");
    assert_eq!(parsed["message"], "custom message");
    assert_eq!(parsed["exit_code"], 42);
}

#[test]
fn test_json_output_with_context() {
    let error =
        RushError::new("TEST", "message", 1).with_context(serde_json::json!({"key": "value"}));
    let json = error.to_json();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["context"]["key"], "value");
}

#[test]
fn test_error_backwards_compatibility() {
    // Ensure text format is the default
    env::remove_var("RUSH_ERROR_FORMAT");
    assert!(!should_output_json_errors());

    let error = RushError::new("TEST_ERROR", "test message", 1);
    let text = error.to_text();

    // Should be plain text, not JSON
    assert!(!text.starts_with('{'));
    assert_eq!(text, "test message");
}
