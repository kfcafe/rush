use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_set_errexit_stops_on_error() {
    let mut executor = Executor::new();

    // Parse and execute: set -e; false; echo "should not print"
    let input = "set -e; false; echo should_not_print";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should exit after false, so echo should not execute
    assert_ne!(result.exit_code, 0);
    assert!(!result.stdout().contains("should_not_print"));
}

#[test]
fn test_set_errexit_can_be_disabled() {
    let mut executor = Executor::new();

    // Parse and execute: set -e; set +e; false; echo "should print"
    let input = "set -e; set +e; false; echo should_print";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should continue after false because errexit is disabled
    assert!(result.stdout().contains("should_print"));
}

#[test]
fn test_set_nounset_errors_on_undefined_var() {
    let mut executor = Executor::new();

    // Parse and execute: set -u; echo $UNDEFINED_VAR
    let input = "set -u; echo $UNDEFINED_VAR";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);

    // Should error on undefined variable
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("unbound variable") || err_msg.contains("UNDEFINED_VAR"));
}

#[test]
fn test_set_nounset_allows_defined_var() {
    let mut executor = Executor::new();

    // Set variable in runtime directly
    executor
        .runtime_mut()
        .set_variable("MYVAR".to_string(), "hello".to_string());

    // Set -u and use the variable
    let input = "set -u; echo $MYVAR";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should work fine with defined variable
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("hello"));
}

#[test]
fn test_set_nounset_can_be_disabled() {
    let mut executor = Executor::new();

    // Parse and execute: set -u; set +u; echo $UNDEFINED_VAR
    let input = "set -u; set +u; echo $UNDEFINED_VAR";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);

    // Should work fine when nounset is disabled
    assert!(result.is_ok());
}

#[test]
fn test_set_combined_options() {
    let mut executor = Executor::new();

    // Test setting multiple options at once: set -eux
    let input = "set -eux";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Check all options are set
    assert!(executor.runtime_mut().options.errexit);
    assert!(executor.runtime_mut().options.nounset);
    assert!(executor.runtime_mut().options.xtrace);
}

#[test]
fn test_set_no_args_shows_options() {
    let mut executor = Executor::new();

    // Set some options
    let input = "set -e";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Now run set with no args
    let input = "set";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should show current options
    assert!(result.stdout().contains("set -e"));
    assert!(result.stdout().contains("set +u"));
}

#[test]
fn test_set_o_pipefail() {
    let mut executor = Executor::new();

    // Test setting pipefail with -o
    let input = "set -o pipefail";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Check pipefail is set
    assert!(executor.runtime_mut().options.pipefail);
}

#[test]
fn test_set_plus_o_pipefail() {
    let mut executor = Executor::new();

    // First set, then unset pipefail
    let input = "set -o pipefail; set +o pipefail";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    executor.execute(statements).unwrap();

    // Check pipefail is unset
    assert!(!executor.runtime_mut().options.pipefail);
}

#[test]
fn test_pipefail_with_failing_command() {
    let mut executor = Executor::new();

    // Test pipefail: set -o pipefail; false | echo hello
    // Without pipefail, exit code would be 0 (from echo)
    // With pipefail, exit code should be non-zero (from false)
    let input = "set -o pipefail; false | echo hello";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should have non-zero exit code due to pipefail
    assert_ne!(result.exit_code, 0);
}

#[test]
fn test_pipefail_disabled() {
    let mut executor = Executor::new();

    // Test without pipefail: false | echo hello
    // Exit code should be 0 (from echo)
    let input = "false | echo hello";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should have zero exit code (last command in pipeline)
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_errexit_and_pipefail_combined() {
    let mut executor = Executor::new();

    // Test errexit with pipefail: set -e; set -o pipefail; false | echo hello; echo "should not print"
    let input = "set -e; set -o pipefail; false | echo hello; echo should_not_print";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should exit after pipeline due to errexit and pipefail
    assert_ne!(result.exit_code, 0);
    assert!(!result.stdout().contains("should_not_print"));
}

#[test]
fn test_set_invalid_option() {
    let mut executor = Executor::new();

    // Test setting an invalid option
    let input = "set -z";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);

    // Should error
    assert!(result.is_err());
}

#[test]
fn test_options_isolated_in_subshell() {
    let mut executor = Executor::new();

    // Test that options set in subshell don't affect parent
    let input = "set +e; (set -e; false); echo after_subshell";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should continue after subshell because errexit is not set in parent
    assert!(result.stdout().contains("after_subshell"));
}
