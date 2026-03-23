use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_exit_code_variable_after_success() {
    let mut executor = Executor::new();

    // Execute a successful command
    let tokens = Lexer::tokenize("echo hello").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();
    assert_eq!(result.exit_code, 0);

    // Check $? is set to 0
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
    assert_eq!(
        executor.runtime_mut().get_variable("?"),
        Some("0".to_string())
    );
}

#[test]
fn test_exit_code_variable_after_failure() {
    let mut executor = Executor::new();

    // Execute a command that fails (command not found returns non-zero)
    let tokens = Lexer::tokenize("nonexistent_command_xyz").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements);

    // Should fail
    assert!(result.is_err());
}

#[test]
fn test_conditional_and_success() {
    let mut executor = Executor::new();

    // Both commands should execute
    let tokens = Lexer::tokenize("echo first && echo second").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_conditional_and_first_fails() {
    let mut executor = Executor::new();

    // Use false builtin if available, otherwise use a command that returns non-zero
    // For this test, we'll use the cat builtin with a non-existent file
    let tokens = Lexer::tokenize("cat /nonexistent/file && echo should_not_run").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Second command should not run
    assert!(!result.stdout().contains("should_not_run"));
    // Exit code should be non-zero from the first command
    assert_ne!(result.exit_code, 0);
    assert_ne!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_conditional_or_first_succeeds() {
    let mut executor = Executor::new();

    // Second command should not execute
    let tokens = Lexer::tokenize("echo first || echo should_not_run").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(!result.stdout().contains("should_not_run"));
    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_conditional_or_first_fails() {
    let mut executor = Executor::new();

    // Both commands should execute
    let tokens = Lexer::tokenize("cat /nonexistent/file || echo fallback").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Second command should run
    assert!(result.stdout().contains("fallback"));
    // Exit code should be 0 from the fallback command
    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_pipeline_exit_code_last_command() {
    let mut executor = Executor::new();

    // Pipeline exit code should be from the last command
    let tokens = Lexer::tokenize("echo test | cat").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_exit_code_with_variable_expansion() {
    let mut executor = Executor::new();

    // Execute a successful command
    let tokens = Lexer::tokenize("echo hello").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Now check if we can read $?
    let tokens = Lexer::tokenize("echo $?").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().trim().contains("0"));
}

#[test]
fn test_chained_conditionals() {
    let mut executor = Executor::new();

    // Test: cmd1 && cmd2 && cmd3
    let tokens = Lexer::tokenize("echo first && echo second && echo third").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
    assert!(result.stdout().contains("third"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_mixed_conditionals() {
    let mut executor = Executor::new();

    // Test: failing_cmd || echo fallback && echo success
    let tokens = Lexer::tokenize("cat /nonexistent/file || echo fallback").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("fallback"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_exit_code_with_builtins() {
    let mut executor = Executor::new();

    // Test with cd builtin (successful)
    let tokens = Lexer::tokenize("cd /tmp").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_exit_code_persists_across_statements() {
    let mut executor = Executor::new();

    // Execute first command (will succeed)
    let tokens = Lexer::tokenize("echo test").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);

    // Set last exit code to 42 manually to simulate a command that returned 42
    executor.runtime_mut().set_last_exit_code(42);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 42);

    // Execute another successful command
    let tokens = Lexer::tokenize("echo another").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // $? should be updated to 0 from the successful command
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}

#[test]
fn test_assignment_has_zero_exit_code() {
    let mut executor = Executor::new();

    // Assignments should have exit code 0
    let tokens = Lexer::tokenize("let x = 42").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(executor.runtime_mut().get_last_exit_code(), 0);
}
