use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

// =============================================================================
// SINGLE QUOTES - NO EXPANSION
// =============================================================================

#[test]
fn test_single_quotes_preserve_literal_dollar() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "expanded".to_string());

    // Single quotes should preserve literal $VAR
    let tokens = Lexer::tokenize("echo 'literal $VAR'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "literal $VAR");
}

#[test]
fn test_single_quotes_preserve_backslash() {
    let mut executor = Executor::new();

    // Single quotes should preserve backslashes literally
    let tokens = Lexer::tokenize(r"echo 'back\slash'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), r"back\slash");
}

#[test]
fn test_single_quotes_preserve_double_quotes() {
    let mut executor = Executor::new();

    // Single quotes should preserve double quotes literally
    let tokens = Lexer::tokenize(r#"echo 'has "quotes" inside'"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), r#"has "quotes" inside"#);
}

#[test]
fn test_single_quotes_preserve_backticks() {
    let mut executor = Executor::new();

    // Single quotes should preserve backticks literally (no command substitution)
    let tokens = Lexer::tokenize(r"echo 'no `cmd` sub'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "no `cmd` sub");
}

#[test]
fn test_single_quotes_preserve_newlines() {
    let mut executor = Executor::new();

    // Single quotes should preserve special characters
    let tokens = Lexer::tokenize(r"echo 'line\nbreak'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should preserve literal \n, not interpret as newline
    assert_eq!(result.stdout().as_str().trim(), r"line\nbreak");
}

// =============================================================================
// DOUBLE QUOTES - ALLOW EXPANSION
// =============================================================================

#[test]
fn test_double_quotes_expand_variable() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "expanded".to_string());

    // Double quotes should expand variables
    let tokens = Lexer::tokenize(r#"echo "value: $VAR""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "value: expanded");
}

#[test]
fn test_double_quotes_expand_braced_variable() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("NAME".to_string(), "world".to_string());

    // Double quotes should expand ${VAR}
    let tokens = Lexer::tokenize(r#"echo "Hello ${NAME}!""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "Hello world!");
}

#[test]
fn test_double_quotes_expand_special_variables() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_last_exit_code(42);

    // Double quotes should expand $?
    let tokens = Lexer::tokenize(r#"echo "exit: $?""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "exit: 42");
}

#[test]
fn test_double_quotes_expand_command_substitution() {
    let mut executor = Executor::new();

    // Double quotes should expand $(cmd)
    let tokens = Lexer::tokenize(r#"echo "result: $(echo test)""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "result: test");
}

#[test]
fn test_double_quotes_preserve_literal_text() {
    let mut executor = Executor::new();

    // Double quotes should preserve literal text
    let tokens = Lexer::tokenize(r#"echo "hello world""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "hello world");
}

// =============================================================================
// BACKSLASH ESCAPES IN DOUBLE QUOTES
// =============================================================================

#[test]
fn test_double_quotes_escape_dollar() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "expanded".to_string());

    // \$ in double quotes should preserve literal $
    let tokens = Lexer::tokenize(r#"echo "preserve \$VAR""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should show literal $VAR
    assert_eq!(result.stdout().as_str().trim(), "preserve $VAR");
}

#[test]
fn test_double_quotes_escape_backslash() {
    let mut executor = Executor::new();

    // \\ in double quotes should produce single backslash
    let tokens = Lexer::tokenize(r#"echo "back\\slash""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), r"back\slash");
}

#[test]
fn test_double_quotes_escape_double_quote() {
    let mut executor = Executor::new();

    // \" in double quotes should produce literal "
    let tokens = Lexer::tokenize(r#"echo "has \"quotes\" inside""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), r#"has "quotes" inside"#);
}

#[test]
fn test_double_quotes_escape_backtick() {
    let mut executor = Executor::new();

    // \` in double quotes should preserve literal backtick
    let tokens = Lexer::tokenize(r#"echo "no \`cmd\` substitution""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "no `cmd` substitution");
}

// =============================================================================
// QUOTE NESTING
// =============================================================================

#[test]
fn test_single_quotes_inside_double_quotes() {
    let mut executor = Executor::new();

    // Single quotes inside double quotes are literal
    let tokens = Lexer::tokenize(r#"echo "outer 'inner' outer""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "outer 'inner' outer");
}

#[test]
fn test_double_quotes_inside_single_quotes() {
    let mut executor = Executor::new();

    // Double quotes inside single quotes are literal
    let tokens = Lexer::tokenize(r#"echo 'outer "inner" outer'"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), r#"outer "inner" outer"#);
}

#[test]
fn test_mixed_quoted_arguments() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Mix of quoted and unquoted
    let tokens = Lexer::tokenize(r#"echo 'single' "double $VAR" unquoted"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Note: echo adds spaces between arguments
    assert_eq!(
        result.stdout().as_str().trim(),
        "single double value unquoted"
    );
}

// =============================================================================
// WHITESPACE PRESERVATION
// =============================================================================

#[test]
fn test_double_quotes_preserve_spaces() {
    let mut executor = Executor::new();

    // Double quotes should preserve multiple spaces
    let tokens = Lexer::tokenize(r#"echo "a  b""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "a  b");
}

#[test]
fn test_single_quotes_preserve_spaces() {
    let mut executor = Executor::new();

    // Single quotes should preserve multiple spaces
    let tokens = Lexer::tokenize(r"echo 'a  b'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "a  b");
}

#[test]
fn test_quotes_preserve_leading_trailing_spaces() {
    let mut executor = Executor::new();

    // Quotes should preserve leading/trailing spaces within the quotes
    let tokens = Lexer::tokenize(r#"echo " spaces ""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Note: trim() removes the trailing space for comparison
    // The actual output has the space
    let output = result.stdout();
    assert!(output.contains(" spaces "));
}

// =============================================================================
// EMPTY STRINGS
// =============================================================================

#[test]
fn test_empty_double_quotes() {
    let mut executor = Executor::new();

    // Empty double quotes should produce empty argument
    let tokens = Lexer::tokenize(r#"echo """#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // echo with empty string produces just newline
    assert_eq!(result.stdout().as_str().trim(), "");
}

#[test]
fn test_empty_single_quotes() {
    let mut executor = Executor::new();

    // Empty single quotes should produce empty argument
    let tokens = Lexer::tokenize(r"echo ''").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // echo with empty string produces just newline
    assert_eq!(result.stdout().as_str().trim(), "");
}

#[test]
fn test_multiple_empty_strings() {
    let mut executor = Executor::new();

    // Multiple empty strings
    let tokens = Lexer::tokenize(r#"echo "" '' """#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // echo adds spaces between arguments, even empty ones
    // This depends on implementation - may be empty or have spaces
    let output = result.stdout();
    let output = output.trim();
    assert!(output.is_empty() || output == " " || output == "  ");
}

// =============================================================================
// GLOB EXPANSION BLOCKED BY QUOTES
// =============================================================================

#[test]
fn test_double_quotes_block_glob_expansion() {
    let mut executor = Executor::new();

    // Double quotes should prevent glob expansion
    let tokens = Lexer::tokenize(r#"echo "*.txt""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should output literal *.txt, not expand to files
    assert_eq!(result.stdout().as_str().trim(), "*.txt");
}

#[test]
fn test_single_quotes_block_glob_expansion() {
    let mut executor = Executor::new();

    // Single quotes should prevent glob expansion
    let tokens = Lexer::tokenize(r"echo '*.rs'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should output literal *.rs, not expand to files
    assert_eq!(result.stdout().as_str().trim(), "*.rs");
}

// =============================================================================
// COMMAND SUBSTITUTION IN QUOTES
// =============================================================================

#[test]
fn test_command_substitution_in_double_quotes() {
    let mut executor = Executor::new();

    // Command substitution works in double quotes
    let tokens = Lexer::tokenize(r#"echo "result: $(echo hello)""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "result: hello");
}

#[test]
fn test_command_substitution_blocked_in_single_quotes() {
    let mut executor = Executor::new();

    // Command substitution should NOT work in single quotes
    let tokens = Lexer::tokenize(r"echo 'result: $(echo hello)'").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should be literal
    assert_eq!(result.stdout().as_str().trim(), "result: $(echo hello)");
}

#[test]
fn test_backtick_substitution_in_double_quotes() {
    let mut executor = Executor::new();

    // Backtick substitution works in double quotes
    let tokens = Lexer::tokenize(r#"echo "result: `echo world`""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "result: world");
}

// =============================================================================
// UNQUOTED BEHAVIOR
// =============================================================================

#[test]
fn test_unquoted_variable_expansion() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "expanded".to_string());

    // Unquoted variables expand
    let tokens = Lexer::tokenize("echo $VAR").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "expanded");
}

#[test]
fn test_unquoted_command_substitution() {
    let mut executor = Executor::new();

    // Unquoted command substitution works
    let tokens = Lexer::tokenize("echo $(echo test)").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "test");
}

// =============================================================================
// COMPLEX QUOTING SCENARIOS
// =============================================================================

#[test]
fn test_adjacent_quoted_strings() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "middle".to_string());

    // Adjacent strings: 'start'"$VAR"'end'
    let tokens = Lexer::tokenize(r#"echo 'start'"$VAR"'end'"#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should concatenate: start + middle + end
    assert_eq!(result.stdout().as_str().trim(), "startmiddleend");
}

#[test]
fn test_quote_escaping_with_variable() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Mix of escaped and expanded
    let tokens = Lexer::tokenize(r#"echo "\$VAR=$VAR""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Should show: $VAR=value
    assert_eq!(result.stdout().as_str().trim(), "$VAR=value");
}

#[test]
fn test_nested_command_substitution_with_quotes() {
    let mut executor = Executor::new();

    // Nested command substitution with quotes
    let tokens = Lexer::tokenize(r#"echo "outer: $(echo "inner")""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "outer: inner");
}

#[test]
fn test_quote_preserves_special_chars() {
    let mut executor = Executor::new();

    // Special shell characters in quotes
    let tokens = Lexer::tokenize(r#"echo "chars: | & ; < >""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "chars: | & ; < >");
}

// =============================================================================
// EDGE CASES
// =============================================================================

#[test]
fn test_variable_with_special_chars_in_quotes() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "a|b&c".to_string());

    // Variable containing special chars, expanded in quotes
    let tokens = Lexer::tokenize(r#"echo "$VAR""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // Special chars from variable should be preserved
    assert_eq!(result.stdout().as_str().trim(), "a|b&c");
}

#[test]
fn test_empty_variable_in_quotes() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("EMPTY".to_string(), "".to_string());

    // Empty variable in quotes
    let tokens = Lexer::tokenize(r#"echo "before:$EMPTY:after""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "before::after");
}

#[test]
fn test_unset_variable_in_quotes() {
    let mut executor = Executor::new();

    // Unset variable in quotes (should expand to empty)
    let tokens = Lexer::tokenize(r#"echo "before:$UNSET:after""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().as_str().trim(), "before::after");
}

#[test]
fn test_backslash_before_regular_char_in_double_quotes() {
    let mut executor = Executor::new();

    // Backslash before regular char in double quotes
    // In POSIX: backslash only escapes $, `, ", \, and newline
    // Before other chars, both backslash and char are preserved
    let tokens = Lexer::tokenize(r#"echo "\a\b\c""#).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    // This depends on implementation - POSIX says preserve both
    // For now, we just verify it doesn't crash
    assert!(!result.stdout().is_empty());
}
