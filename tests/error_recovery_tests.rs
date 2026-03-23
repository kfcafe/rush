use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_parse_error_recovery() {
    // Test that parse errors return errors instead of panicking

    // Invalid syntax - missing closing brace
    let result = Lexer::tokenize("if x { echo test");
    assert!(result.is_ok());

    let tokens = result.unwrap();
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse();

    // Should return an error, not panic
    assert!(parse_result.is_err());
}

#[test]
fn test_parse_error_invalid_command() {
    // Test parsing with invalid token sequence
    let result = Lexer::tokenize("let = 5");
    assert!(result.is_ok());

    let tokens = result.unwrap();
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse();

    // Should return an error, not panic
    assert!(parse_result.is_err());
}

#[test]
fn test_execution_error_recovery() {
    // Test that execution errors don't panic
    let mut executor = Executor::new();

    // Test with non-existent command
    let tokens = Lexer::tokenize("nonexistent_command_xyz").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);

    // Should return an error, not panic
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Command not found"));
}

#[test]
fn test_execution_continues_after_error() {
    // Test that executor can continue after an error
    let mut executor = Executor::new();

    // First command fails
    let tokens = Lexer::tokenize("false").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    // Should succeed but with non-zero exit code
    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_ne!(exec_result.exit_code, 0);

    // Second command should succeed
    let tokens = Lexer::tokenize("echo hello").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_eq!(exec_result.exit_code, 0);
    assert_eq!(exec_result.stdout(), "hello\n");
}

#[test]
fn test_parse_error_empty_pipeline() {
    // Test parsing empty pipeline
    let result = Lexer::tokenize("echo hello |");
    assert!(result.is_ok());

    let tokens = result.unwrap();
    let mut parser = Parser::new(tokens);
    let parse_result = parser.parse();

    // Should handle gracefully
    // This might succeed or fail depending on implementation
    // Key is that it doesn't panic
    let _ = parse_result;
}

#[test]
fn test_invalid_redirect_target() {
    // Test with invalid redirect
    let mut executor = Executor::new();

    // Try to redirect to a directory that doesn't exist
    let tokens = Lexer::tokenize("echo test > /nonexistent/path/file.txt").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);

    // Should return an error, not panic
    assert!(result.is_err());
}

#[test]
fn test_multiple_parse_errors() {
    // Test that parser can handle multiple different error conditions

    let test_cases = vec![
        "let",   // Incomplete assignment
        "if",    // Incomplete if statement
        "for",   // Incomplete for loop
        "fn",    // Incomplete function definition
        "match", // Incomplete match expression
    ];

    for test_case in test_cases {
        let result = Lexer::tokenize(test_case);
        if let Ok(tokens) = result {
            let mut parser = Parser::new(tokens);
            let parse_result = parser.parse();

            // Should return error, not panic
            // Some might succeed as empty, which is also acceptable
            let _ = parse_result;
        }
    }
}

#[test]
fn test_execution_state_not_corrupted() {
    // Test that executor state remains valid after errors
    let mut executor = Executor::new();

    // Set a variable
    let tokens = Lexer::tokenize("let x = 42").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Cause an error
    let tokens = Lexer::tokenize("nonexistent_cmd").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let _ = executor.execute(statements);

    // Verify variable is still accessible
    let tokens = Lexer::tokenize("echo $x").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout(), "42\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_nested_error_recovery() {
    // Test error recovery in nested constructs
    let mut executor = Executor::new();

    // Subshell with error should not break main shell
    let tokens = Lexer::tokenize("(nonexistent_cmd)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    // Should return error, not panic
    assert!(result.is_err());

    // Next command should work
    let tokens = Lexer::tokenize("echo recovered").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout(), "recovered\n");
}

#[test]
fn test_conditional_and_error_recovery() {
    // Test that conditional && stops on error but doesn't crash
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("false && echo should_not_run").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_ne!(exec_result.exit_code, 0);
    assert!(!exec_result.stdout().contains("should_not_run"));
}

#[test]
fn test_conditional_or_error_recovery() {
    // Test that conditional || continues on error but doesn't crash
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("false || echo should_run").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    assert!(result.is_ok());
    let exec_result = result.unwrap();
    assert_eq!(exec_result.exit_code, 0);
    assert_eq!(exec_result.stdout(), "should_run\n");
}
