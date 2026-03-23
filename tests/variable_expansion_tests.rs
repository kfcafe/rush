use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_simple_braced_variable() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("NAME".to_string(), "world".to_string());

    // Test simple expansion: echo ${NAME}
    let tokens = Lexer::tokenize("echo ${NAME}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "world");
}

#[test]
fn test_use_default_operator_with_set_variable() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Test ${VAR:-default} - should use the set value
    let tokens = Lexer::tokenize("echo ${VAR:-default}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "value");
}

#[test]
fn test_use_default_operator_with_unset_variable() {
    let mut executor = Executor::new();

    // Test ${UNSET:-default} - should use the default
    let tokens = Lexer::tokenize("echo ${UNSET:-default}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "default");
}

#[test]
fn test_assign_default_operator_with_unset_variable() {
    let mut executor = Executor::new();

    // Test ${UNSET:=assigned} - should assign and return the default
    let tokens = Lexer::tokenize("echo ${UNSET:=assigned}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "assigned");

    // Verify the variable was actually assigned
    assert_eq!(
        executor.runtime_mut().get_variable("UNSET"),
        Some("assigned".to_string())
    );
}

#[test]
fn test_assign_default_operator_with_set_variable() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "existing".to_string());

    // Test ${VAR:=new} - should use existing value
    let tokens = Lexer::tokenize("echo ${VAR:=new}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "existing");

    // Verify the variable wasn't changed
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("existing".to_string())
    );
}

#[test]
fn test_error_if_unset_with_set_variable() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Test ${VAR:?error} - should return the value
    let tokens = Lexer::tokenize("echo ${VAR:?error message}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "value");
}

#[test]
fn test_error_if_unset_with_unset_variable() {
    let mut executor = Executor::new();

    // Test ${UNSET:?error} - should error
    let tokens = Lexer::tokenize("echo ${UNSET:?variable not set}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("variable not set"));
}

#[test]
fn test_remove_shortest_prefix() {
    let mut executor = Executor::new();

    // Set a variable with a path
    executor
        .runtime_mut()
        .set_variable("PATH".to_string(), "/usr/local/bin".to_string());

    // Test ${PATH#/usr/} - should remove shortest prefix
    let tokens = Lexer::tokenize("echo ${PATH#/usr/}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "local/bin");
}

#[test]
fn test_remove_longest_prefix() {
    let mut executor = Executor::new();

    // Set a variable with repeated pattern
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "foo/bar/foo/baz".to_string());

    // Test ${VAR##*foo/} - should remove longest prefix (up to last foo/)
    let tokens = Lexer::tokenize("echo ${VAR##*foo/}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "baz");
}

#[test]
fn test_remove_shortest_suffix() {
    let mut executor = Executor::new();

    // Set a variable with extension
    executor
        .runtime_mut()
        .set_variable("FILE".to_string(), "document.tar.gz".to_string());

    // Test ${FILE%.gz} - should remove shortest suffix
    let tokens = Lexer::tokenize("echo ${FILE%.gz}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "document.tar");
}

#[test]
fn test_remove_longest_suffix() {
    let mut executor = Executor::new();

    // Set a variable with extension
    executor
        .runtime_mut()
        .set_variable("FILE".to_string(), "document.tar.gz".to_string());

    // Test ${FILE%%.tar*} - should remove longest suffix (with pattern)
    let tokens = Lexer::tokenize("echo ${FILE%%.tar*}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "document");
}

#[test]
fn test_prefix_removal_with_glob() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "hello_world_test".to_string());

    // Test ${VAR#hello_*} with glob pattern
    let tokens = Lexer::tokenize("echo ${VAR#hello_*}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "world_test");
}

#[test]
fn test_suffix_removal_with_glob() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "test_hello_world".to_string());

    // Test ${VAR%_*} with glob pattern
    let tokens = Lexer::tokenize("echo ${VAR%_*}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "test_hello");
}

#[test]
fn test_multiple_expansions_in_one_command() {
    let mut executor = Executor::new();

    // Set multiple variables
    executor
        .runtime_mut()
        .set_variable("FIRST".to_string(), "Hello".to_string());
    executor
        .runtime_mut()
        .set_variable("LAST".to_string(), "World".to_string());

    // Test multiple expansions: echo ${FIRST} ${LAST}
    let tokens = Lexer::tokenize("echo ${FIRST} ${LAST}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "Hello World");
}

#[test]
fn test_expansion_in_let_statement() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("BASE".to_string(), "value".to_string());

    // Test let NEW = ${BASE}
    let tokens = Lexer::tokenize("let NEW = ${BASE}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Verify the new variable was set
    assert_eq!(
        executor.runtime_mut().get_variable("NEW"),
        Some("value".to_string())
    );
}

#[test]
fn test_expansion_with_default_in_let() {
    let mut executor = Executor::new();

    // Test let VAR = ${UNSET:-default_value}
    let tokens = Lexer::tokenize("let VAR = ${UNSET:-default_value}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Verify the variable was set to default
    assert_eq!(
        executor.runtime_mut().get_variable("VAR"),
        Some("default_value".to_string())
    );
}

#[test]
fn test_no_expansion_without_braces() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("NAME".to_string(), "world".to_string());

    // Test that regular $NAME still works
    let tokens = Lexer::tokenize("echo $NAME").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "world");
}

#[test]
fn test_use_alternate_with_set_variable() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Test ${VAR:+alternate} - should return alternate since VAR is set
    let tokens = Lexer::tokenize("echo ${VAR:+alternate}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "alternate");
}

#[test]
fn test_use_alternate_with_unset_variable() {
    let mut executor = Executor::new();

    // Test ${UNSET:+alternate} - should return empty since UNSET is not set
    let tokens = Lexer::tokenize("echo \"[${UNSET:+alternate}]\"").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "[]");
}

#[test]
fn test_use_alternate_with_empty_variable() {
    let mut executor = Executor::new();

    // Set an empty variable
    executor
        .runtime_mut()
        .set_variable("EMPTY".to_string(), "".to_string());

    // Test ${EMPTY:+alternate} - should return empty since EMPTY is empty
    let tokens = Lexer::tokenize("echo \"[${EMPTY:+alternate}]\"").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "[]");
}

#[test]
fn test_use_alternate_in_double_quotes() {
    let mut executor = Executor::new();

    // Set a variable
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "set".to_string());

    // Test "${VAR:+alternate value}" in double quotes
    let tokens = Lexer::tokenize("echo \"prefix ${VAR:+alternate value} suffix\"").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();
    assert_eq!(result.stdout().trim(), "prefix alternate value suffix");
}

#[test]
fn test_error_if_unset_with_empty_variable() {
    let mut executor = Executor::new();

    // Set an empty variable
    executor
        .runtime_mut()
        .set_variable("EMPTY".to_string(), "".to_string());

    // Test ${EMPTY:?error} - should error since EMPTY is empty
    let tokens = Lexer::tokenize("echo ${EMPTY:?variable is empty}").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("variable is empty"));
}
