use rush::executor::{ExecutionResult, Executor};
use rush::lexer::Lexer;
use rush::parser::Parser;

/// Test basic subshell execution
#[test]
fn test_basic_subshell() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(echo hello)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

/// Test variable isolation - variables set in subshell don't affect parent
#[test]
fn test_variable_isolation() {
    let mut executor = Executor::new();

    // Set a variable in parent, modify it in subshell, check it's unchanged
    let tokens = Lexer::tokenize("let x = parent && (let x = child) && echo $x").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // The variable should still be "parent" after the subshell
    assert_eq!(result.stdout().trim(), "parent");
    assert_eq!(result.exit_code, 0);
}

/// Test cd isolation - cd in subshell doesn't affect parent
#[test]
fn test_cd_isolation() {
    let mut executor = Executor::new();

    // Get current directory
    let original_cwd = executor.runtime_mut().get_cwd().clone();

    // Change directory in a subshell to /tmp
    let tokens = Lexer::tokenize("(cd /tmp)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Current directory should be unchanged
    let current_cwd = executor.runtime_mut().get_cwd().clone();
    assert_eq!(current_cwd, original_cwd);
}

/// Test that subshell exit code propagates correctly
#[test]
fn test_exit_code_propagation() {
    let mut executor = Executor::new();

    // Run a command that fails in a subshell
    let tokens = Lexer::tokenize("(false)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // The exit code should be non-zero
    assert_ne!(result.exit_code, 0);
}

/// Test nested subshells
#[test]
fn test_nested_subshells() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("((echo nested))").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "nested");
    assert_eq!(result.exit_code, 0);
}

/// Test multiple statements in a subshell
#[test]
fn test_multiple_statements_in_subshell() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(echo first && echo second)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
    assert_eq!(result.exit_code, 0);
}

/// Test subshell with variable expansion
#[test]
fn test_subshell_variable_expansion() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("let x = hello && (echo $x)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

/// Test deeply nested subshells with variable isolation
#[test]
fn test_deeply_nested_variable_isolation() {
    let mut executor = Executor::new();

    // Set variable at each level and ensure proper isolation
    let tokens =
        Lexer::tokenize("let x = level0 && (let x = level1 && (let x = level2)) && echo $x")
            .unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should still be level0
    assert_eq!(result.stdout().trim(), "level0");
}

/// Test subshell inherits parent variables
#[test]
fn test_subshell_inherits_variables() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("let x = parent && (echo $x)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Subshell should see parent's variable
    assert_eq!(result.stdout().trim(), "parent");
}

/// Test subshell with cd and pwd
#[test]
fn test_subshell_cd_pwd() {
    let mut executor = Executor::new();

    // Change to /tmp in subshell and print working directory
    let tokens = Lexer::tokenize("(cd /tmp && pwd)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should print /tmp
    assert_eq!(result.stdout().trim(), "/tmp");
    assert_eq!(result.exit_code, 0);
}

/// Test empty subshell
#[test]
fn test_empty_subshell() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("()").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout(), "");
    assert_eq!(result.exit_code, 0);
}

/// Test subshell with newlines
#[test]
fn test_subshell_with_newlines() {
    let mut executor = Executor::new();

    let code = r#"(
echo first
echo second
)"#;

    let tokens = Lexer::tokenize(code).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
    assert_eq!(result.exit_code, 0);
}

/// Test sequential subshells
#[test]
fn test_sequential_subshells() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(echo first) && (echo second)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
    assert_eq!(result.exit_code, 0);
}

/// Test subshell in complex expression
#[test]
fn test_subshell_complex() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("let x = outer && (let x = inner && echo $x) && echo $x").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should output "inner" from subshell, then "outer" from parent
    assert!(result.stdout().contains("inner"));
    assert!(result.stdout().contains("outer"));
}

/// Test that modifications in nested subshells don't leak
#[test]
fn test_nested_modifications_dont_leak() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("let x = a && (let x = b && (let x = c)) && echo $x").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Outer variable should be unchanged
    assert_eq!(result.stdout().trim(), "a");
}

/// Test subshell exit on error doesn't stop parent
#[test]
fn test_subshell_error_isolation() {
    let mut executor = Executor::new();

    // Even if subshell fails, parent continues (with && this would stop, but we're testing the subshell itself)
    let tokens = Lexer::tokenize("(false); echo continued").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert!(result.stdout().contains("continued"));
}

/// ACCEPTANCE CRITERIA: (exit 5); echo $?  →  outputs "5"
/// `exit` in a subshell should only exit the subshell, not the entire process.
/// The parent shell continues and $? reflects the subshell's exit code.
#[test]
fn test_exit_only_exits_subshell() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(exit 5); echo $?").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // The parent should continue running and echo $? which should be 5
    assert_eq!(result.stdout().trim(), "5");
}

/// ACCEPTANCE CRITERIA: echo test | (cat)  →  works
/// Subshells should be usable as pipeline elements.
#[test]
fn test_subshell_in_pipeline_parses() {
    // First verify parsing succeeds (previously returned error)
    let tokens = Lexer::tokenize("echo test | (cat)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse();
    assert!(
        statements.is_ok(),
        "Parsing 'echo test | (cat)' should succeed"
    );
}

/// Test (cd /tmp; pwd) outputs /tmp - subshell cd isolation
#[test]
fn test_subshell_cd_semicolon_pwd() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(cd /tmp; pwd)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert!(
        result.stdout().trim().contains("tmp"),
        "Expected /tmp in output, got: {}",
        result.stdout()
    );
    assert_eq!(result.exit_code, 0);
}

/// Test exit with no args in subshell (exit code 0)
#[test]
fn test_exit_no_args_in_subshell() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(exit); echo $?").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0");
}

/// Test (exit 42) captures exit code without killing process
#[test]
fn test_subshell_exit_code_capture() {
    let mut executor = Executor::new();

    let tokens = Lexer::tokenize("(exit 42)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    assert_eq!(result.exit_code, 42);
}

/// Test subshell in pipeline - parsing creates PipelineElement::Subshell
#[test]
fn test_pipeline_with_subshell_at_end() {
    // (echo hello) | (cat) should parse successfully
    let tokens = Lexer::tokenize("(echo hello) | (cat)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse();
    assert!(
        statements.is_ok(),
        "Pipeline with subshells should parse: {:?}",
        statements.err()
    );
}
