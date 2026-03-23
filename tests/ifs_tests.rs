use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_ifs_default_for_loop() {
    let mut executor = Executor::new();

    // for x in $var; do echo $x; done with default IFS (space, tab, newline)
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "a b c".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should print each word on a separate line
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_ifs_custom_colon() {
    let mut executor = Executor::new();

    // Set IFS to colon
    executor
        .runtime_mut()
        .set_variable("IFS".to_string(), ":".to_string());
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "a:b:c".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should split by colon
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_ifs_multiple_chars() {
    let mut executor = Executor::new();

    // Set IFS to multiple characters (colon and comma)
    executor
        .runtime_mut()
        .set_variable("IFS".to_string(), ":,".to_string());
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "a:b,c".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should split by both colon and comma
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_ifs_empty_no_splitting() {
    let mut executor = Executor::new();

    // Set IFS to empty string (no splitting)
    executor
        .runtime_mut()
        .set_variable("IFS".to_string(), "".to_string());
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "a b c".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should NOT split - print entire string
    assert_eq!(output.trim(), "a b c");
}

#[test]
fn test_ifs_variable_expansion_in_echo() {
    let mut executor = Executor::new();

    // Test that variable expansion with default IFS splits into separate arguments
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "one two three".to_string());

    let tokens = Lexer::tokenize("echo $var").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // echo joins arguments with space, so result should be the same
    assert_eq!(result.stdout().trim(), "one two three");
}

#[test]
fn test_ifs_command_substitution() {
    let mut executor = Executor::new();

    // Test command substitution with IFS splitting
    let tokens = Lexer::tokenize("for x in $(echo 'a b c'); do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should split the output of command substitution by default IFS
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_ifs_with_leading_trailing_whitespace() {
    let mut executor = Executor::new();

    // Variable with leading/trailing spaces
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "  a b c  ".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Leading/trailing IFS characters should be stripped
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_ifs_with_consecutive_separators() {
    let mut executor = Executor::new();

    // Multiple consecutive spaces should be treated as single separator
    executor
        .runtime_mut()
        .set_variable("var".to_string(), "a  b    c".to_string());

    let tokens = Lexer::tokenize("for x in $var; do echo $x; done").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    let output = result.stdout();

    // Should only create 3 fields
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn test_runtime_split_by_ifs_default() {
    use rush::runtime::Runtime;

    let runtime = Runtime::new();
    let fields = runtime.split_by_ifs("hello world  test");
    assert_eq!(fields, vec!["hello", "world", "test"]);
}

#[test]
fn test_runtime_split_by_ifs_custom() {
    use rush::runtime::Runtime;

    let mut runtime = Runtime::new();
    runtime.set_variable("IFS".to_string(), ":".to_string());
    let fields = runtime.split_by_ifs("one:two::three");
    assert_eq!(fields, vec!["one", "two", "three"]);
}

#[test]
fn test_runtime_split_by_ifs_empty() {
    use rush::runtime::Runtime;

    let mut runtime = Runtime::new();
    runtime.set_variable("IFS".to_string(), "".to_string());
    let fields = runtime.split_by_ifs("hello world");
    assert_eq!(fields, vec!["hello world"]);
}

#[test]
fn test_runtime_split_by_ifs_newline() {
    use rush::runtime::Runtime;

    let runtime = Runtime::new();
    let fields = runtime.split_by_ifs("hello\nworld\ntest");
    assert_eq!(fields, vec!["hello", "world", "test"]);
}

#[test]
fn test_runtime_split_by_ifs_tab() {
    use rush::runtime::Runtime;

    let runtime = Runtime::new();
    let fields = runtime.split_by_ifs("hello\tworld\ttest");
    assert_eq!(fields, vec!["hello", "world", "test"]);
}

#[test]
fn test_runtime_split_by_ifs_mixed_whitespace() {
    use rush::runtime::Runtime;

    let runtime = Runtime::new();
    let fields = runtime.split_by_ifs("hello \t world\n\ttest");
    assert_eq!(fields, vec!["hello", "world", "test"]);
}
