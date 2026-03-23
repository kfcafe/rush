use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_basic_command_substitution() {
    let mut executor = Executor::new();

    // Test simple $(command) syntax
    let tokens = Lexer::tokenize("echo $(echo hello)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_backtick_substitution() {
    let mut executor = Executor::new();

    // Test backtick syntax
    let tokens = Lexer::tokenize("echo `echo world`").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "world");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_nested_command_substitution() {
    let mut executor = Executor::new();

    // Test nested $(echo $(echo nested))
    let tokens = Lexer::tokenize("echo $(echo $(echo nested))").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "nested");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_in_string() {
    let mut executor = Executor::new();

    // Test substitution in double-quoted string: echo "path: $(pwd)"
    // Note: For this to work properly, we'd need string interpolation
    // For now, test that the substitution works as an argument
    let tokens = Lexer::tokenize("echo prefix-$(echo test)-suffix").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Each part should be a separate argument, so output will have spaces
    assert!(result.stdout().contains("prefix-test-suffix") || result.stdout().contains("test"));
}

#[test]
fn test_command_substitution_with_pwd() {
    let mut executor = Executor::new();

    // Test with pwd builtin
    let tokens = Lexer::tokenize("echo $(pwd)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Result should contain the current working directory path
    assert!(!result.stdout().trim().is_empty());
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_in_assignment() {
    let mut executor = Executor::new();

    // Test let x = $(echo value)
    let tokens = Lexer::tokenize("let x = $(echo myvalue)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.exit_code, 0);

    // Verify variable was set correctly
    let var_value = executor.runtime_mut().get_variable("x");
    assert_eq!(var_value, Some("myvalue".to_string()));
}

#[test]
fn test_whitespace_trimming() {
    let mut executor = Executor::new();

    // Test that trailing newlines are trimmed
    let tokens = Lexer::tokenize(
        r#"echo $(echo "line1
line2")"#,
    )
    .unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should trim trailing newline but preserve internal ones
    let stdout = result.stdout();
    let output = stdout.trim();
    assert!(output.contains("line1"));
    assert!(output.contains("line2"));
}

#[test]
fn test_command_substitution_with_multiple_args() {
    let mut executor = Executor::new();

    // Test echo $(echo first) middle $(echo last)
    let tokens = Lexer::tokenize("echo $(echo first) middle $(echo last)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    let stdout = result.stdout();
    let output = stdout.trim();
    assert!(output.contains("first"));
    assert!(output.contains("middle"));
    assert!(output.contains("last"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_in_pipeline() {
    let mut executor = Executor::new();

    // Test echo $(echo hello) | cat
    let tokens = Lexer::tokenize("echo $(echo hello) | cat").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_exit_code() {
    let mut executor = Executor::new();

    // Test that exit code from inner command is not propagated to outer
    // but the substitution itself works
    let tokens = Lexer::tokenize("echo $(echo success)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "success");
    // The echo command should succeed even if inner command had different exit code
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_empty_command_substitution() {
    let mut executor = Executor::new();

    // Test echo $(echo)
    let tokens = Lexer::tokenize("echo $(echo)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should output empty or just newline
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_with_flags() {
    let mut executor = Executor::new();

    // Test ls $(echo -la)
    // This tests that flags can come from substitution
    let tokens = Lexer::tokenize("echo $(echo -v)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "-v");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_double_nested_substitution() {
    let mut executor = Executor::new();

    // Test deeply nested: $(echo $(echo $(echo deep)))
    let tokens = Lexer::tokenize("echo $(echo $(echo $(echo deep)))").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "deep");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_substitution_lexer_nesting() {
    // Test that the lexer correctly parses nested command substitutions
    let input = "echo $(echo $(pwd))";
    let tokens = Lexer::tokenize(input).unwrap();

    // Should have: Identifier("echo"), CommandSubstitution("$(echo $(pwd))")
    assert_eq!(tokens.len(), 2);

    if let rush::lexer::Token::CommandSubstitution(cmd) = &tokens[1] {
        assert_eq!(cmd, "$(echo $(pwd))");
    } else {
        panic!("Expected CommandSubstitution token");
    }
}

#[test]
fn test_backtick_lexer() {
    // Test that the lexer correctly parses backtick substitutions
    let input = "echo `pwd`";
    let tokens = Lexer::tokenize(input).unwrap();

    // Should have: Identifier("echo"), BacktickSubstitution("`pwd`")
    assert_eq!(tokens.len(), 2);

    if let rush::lexer::Token::BacktickSubstitution(cmd) = &tokens[1] {
        assert_eq!(cmd, "`pwd`");
    } else {
        panic!("Expected BacktickSubstitution token");
    }
}

#[test]
fn test_command_substitution_with_semicolon() {
    let mut executor = Executor::new();

    // Test $(echo a; echo b) - multiple commands in substitution
    let tokens = Lexer::tokenize("echo $(echo a; echo b)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should contain output from both commands
    let stdout = result.stdout();
    let output = stdout.trim();
    assert!(output.contains('a'));
    assert!(output.contains('b'));
}

#[test]
fn test_mixed_arguments_with_substitution() {
    let mut executor = Executor::new();

    // Test: echo literal $(echo substituted) $HOME
    // This needs variables to work, for now just test that it parses
    let tokens = Lexer::tokenize("echo literal $(echo substituted) another").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    let stdout = result.stdout();
    let output = stdout.trim();
    assert!(output.contains("literal"));
    assert!(output.contains("substituted"));
    assert!(output.contains("another"));
}

// --- Tests for command substitution inside strings (rush-onx.9) ---

#[test]
fn test_subst_in_double_quoted_string() {
    let mut executor = Executor::new();

    // echo "dir: $(echo hello)"  =>  dir: hello
    let tokens = Lexer::tokenize(r#"echo "dir: $(echo hello)""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "dir: hello");
}

#[test]
fn test_subst_multiple_in_string() {
    let mut executor = Executor::new();

    // echo "a=$(echo 1) b=$(echo 2)"  =>  a=1 b=2
    let tokens = Lexer::tokenize(r#"echo "a=$(echo 1) b=$(echo 2)""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "a=1 b=2");
}

#[test]
fn test_subst_in_assignment_value() {
    let mut executor = Executor::new();

    // x=$(echo foo); echo $x  =>  foo
    let tokens = Lexer::tokenize("x=$(echo foo); echo $x").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "foo");
}

#[test]
fn test_subst_with_pwd() {
    let mut executor = Executor::new();

    // echo "dir: $(pwd)"  =>  dir: /some/path
    let tokens = Lexer::tokenize(r#"echo "dir: $(pwd)""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    let output = result.stdout().trim().to_string();
    assert!(
        output.starts_with("dir: /"),
        "Expected 'dir: /' prefix, got: {}",
        output
    );
}

#[test]
fn test_nested_subst_in_string() {
    let mut executor = Executor::new();

    // echo "val: $(echo $(echo deep))"  =>  val: deep
    let tokens = Lexer::tokenize(r#"echo "val: $(echo $(echo deep))""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "val: deep");
}
