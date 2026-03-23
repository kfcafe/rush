use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_readonly_create_with_value() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("readonly VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should be set
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("value".to_string())
    );

    // Variable should be readonly
    assert!(executor.runtime_mut().is_readonly("VAR"));
}

#[test]
fn test_readonly_mark_existing_variable() {
    let mut executor = Executor::new();

    // First set a variable
    let tokens = Lexer::tokenize("VAR=initial").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Now mark it readonly
    let tokens = Lexer::tokenize("readonly VAR").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should still have its value
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("initial".to_string())
    );

    // Variable should be readonly
    assert!(executor.runtime_mut().is_readonly("VAR"));
}

#[test]
fn test_readonly_cannot_reassign_via_readonly() {
    let mut executor = Executor::new();

    // Create readonly variable
    let tokens = Lexer::tokenize("readonly VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Try to reassign via readonly command - should error
    let tokens = Lexer::tokenize("readonly VAR=newvalue").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("readonly variable"));

    // Value should not have changed
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("value".to_string())
    );
}

#[test]
fn test_readonly_cannot_unset() {
    let mut executor = Executor::new();

    // Create readonly variable
    let tokens = Lexer::tokenize("readonly VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Try to unset - should error
    let tokens = Lexer::tokenize("unset VAR").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("cannot unset"));
    assert!(err_msg.contains("readonly"));

    // Variable should still exist
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("value".to_string())
    );
}

#[test]
fn test_readonly_multiple_variables() {
    let mut executor = Executor::new();

    // Mark multiple variables readonly at once
    let tokens = Lexer::tokenize("readonly A=1 B=2 C=3").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // All should be readonly
    assert!(executor.runtime_mut().is_readonly("A"));
    assert!(executor.runtime_mut().is_readonly("B"));
    assert!(executor.runtime_mut().is_readonly("C"));

    // All should have values
    assert_eq!(
        executor.runtime_mut().get_variable("A"),
        Some("1".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("B"),
        Some("2".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("C"),
        Some("3".to_string())
    );
}

#[test]
fn test_readonly_print_all() {
    let mut executor = Executor::new();

    // Create some readonly variables
    let tokens = Lexer::tokenize("readonly VAR1=value1").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    let tokens = Lexer::tokenize("readonly VAR2=value2").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Print all readonly variables
    let tokens = Lexer::tokenize("readonly -p").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    let output = result.unwrap().stdout();
    assert!(output.contains("readonly VAR1='value1'"));
    assert!(output.contains("readonly VAR2='value2'"));
}

#[test]
fn test_readonly_print_no_args() {
    let mut executor = Executor::new();

    // Create a readonly variable
    let tokens = Lexer::tokenize("readonly VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Print without arguments
    let tokens = Lexer::tokenize("readonly").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    let output = result.unwrap().stdout();
    assert!(output.contains("readonly VAR='value'"));
}

#[test]
fn test_readonly_invalid_identifier() {
    let mut executor = Executor::new();

    // Invalid identifier (starts with number)
    let tokens = Lexer::tokenize("readonly 123VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not a valid identifier"));
}

#[test]
fn test_readonly_with_spaces_in_value() {
    let mut executor = Executor::new();

    // Value with spaces (shell should handle quoting)
    let tokens = Lexer::tokenize(r#"readonly VAR="hello world""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should have the value with spaces
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("hello world".to_string())
    );
    assert!(executor.runtime_mut().is_readonly("VAR"));
}

#[test]
fn test_readonly_mark_nonexistent() {
    let mut executor = Executor::new();

    // Mark a variable readonly even though it doesn't exist
    let tokens = Lexer::tokenize("readonly NONEXISTENT").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should be marked readonly
    assert!(executor.runtime_mut().is_readonly("NONEXISTENT"));

    // Variable doesn't have a value yet
    assert_eq!(executor.runtime_mut().get_variable("NONEXISTENT"), None);
}

#[test]
fn test_readonly_persists_across_function_calls() {
    let mut executor = Executor::new();

    // Create readonly variable
    let tokens = Lexer::tokenize("readonly GLOBAL=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Define a function
    let tokens = Lexer::tokenize(r#"myfunc() { echo "In function"; }"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Call the function
    let tokens = Lexer::tokenize("myfunc").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Variable should still be readonly
    assert!(executor.runtime_mut().is_readonly("GLOBAL"));
    assert_eq!(
        executor.runtime_mut().get_variable("GLOBAL"),
        Some("value".to_string())
    );
}

#[test]
fn test_readonly_empty_value() {
    let mut executor = Executor::new();

    // Create readonly variable with empty value
    let tokens = Lexer::tokenize("readonly VAR=").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should be set to empty string
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("".to_string())
    );
    assert!(executor.runtime_mut().is_readonly("VAR"));
}

#[test]
fn test_readonly_multiple_with_mixed_assignment() {
    let mut executor = Executor::new();

    // Set one variable first
    let tokens = Lexer::tokenize("EXISTING=old").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Mark multiple readonly with mixed styles
    let tokens = Lexer::tokenize("readonly NEW=value EXISTING").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Both should be readonly
    assert!(executor.runtime_mut().is_readonly("NEW"));
    assert!(executor.runtime_mut().is_readonly("EXISTING"));

    // NEW has new value, EXISTING keeps old value
    assert_eq!(
        executor.runtime_mut().get_variable("NEW"),
        Some("value".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("EXISTING"),
        Some("old".to_string())
    );
}

#[test]
fn test_readonly_output_format() {
    let mut executor = Executor::new();

    // Create readonly variable
    let tokens = Lexer::tokenize("readonly VAR=value").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).expect("Failed to execute");

    // Print it
    let tokens = Lexer::tokenize("readonly -p").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    let output = result.unwrap().stdout();

    // Should be in format: readonly VAR='value'
    assert!(output.contains("readonly VAR='value'"));
}

#[test]
fn test_readonly_special_characters_in_value() {
    let mut executor = Executor::new();

    // Value with special characters
    let tokens = Lexer::tokenize(r#"readonly VAR='$PATH:/extra'"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);
    assert!(result.is_ok());

    // Variable should preserve the literal string
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("$PATH:/extra".to_string())
    );
    assert!(executor.runtime_mut().is_readonly("VAR"));
}
