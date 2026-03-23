//! Integration tests for the shift builtin
//!
//! Tests cover:
//! - Basic shift: shifts $2->$1, $3->$2, etc.
//! - shift N (shift by N positions)
//! - shift with no positional params (should error)
//! - shift beyond available params (should error)
//! - $# is decremented correctly
//! - shift in functions vs global scope

use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

fn execute_line(line: &str, executor: &mut Executor) -> Result<String, String> {
    let tokens = Lexer::tokenize(line).map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().map_err(|e| e.to_string())?;
    let result = executor.execute(statements).map_err(|e| e.to_string())?;
    Ok(result.stdout().trim().to_string())
}

fn execute_line_with_code(line: &str, executor: &mut Executor) -> Result<(String, i32), String> {
    let tokens = Lexer::tokenize(line).map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().map_err(|e| e.to_string())?;
    let result = executor.execute(statements).map_err(|e| e.to_string())?;
    Ok((result.stdout().trim().to_string(), result.exit_code))
}

// ============================================================================
// Basic shift tests
// ============================================================================

#[test]
fn test_shift_basic_by_one() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
    ]);

    // Before shift
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "a");
    assert_eq!(execute_line("echo $2", &mut executor).unwrap(), "b");
    assert_eq!(execute_line("echo $3", &mut executor).unwrap(), "c");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "3");

    // Shift by 1 (default)
    execute_line("shift", &mut executor).unwrap();

    // After shift: $2->$1, $3->$2, $3 is now empty
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "b");
    assert_eq!(execute_line("echo $2", &mut executor).unwrap(), "c");
    assert_eq!(execute_line("echo $3", &mut executor).unwrap(), "");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");
}

#[test]
fn test_shift_updates_at_and_star() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "x".to_string(),
        "y".to_string(),
        "z".to_string(),
    ]);

    assert_eq!(execute_line("echo $@", &mut executor).unwrap(), "x y z");
    assert_eq!(execute_line("echo $*", &mut executor).unwrap(), "x y z");

    execute_line("shift", &mut executor).unwrap();

    assert_eq!(execute_line("echo $@", &mut executor).unwrap(), "y z");
    assert_eq!(execute_line("echo $*", &mut executor).unwrap(), "y z");
}

// ============================================================================
// shift N tests
// ============================================================================

#[test]
fn test_shift_by_two() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
        "four".to_string(),
    ]);

    execute_line("shift 2", &mut executor).unwrap();

    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "three");
    assert_eq!(execute_line("echo $2", &mut executor).unwrap(), "four");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");
}

#[test]
fn test_shift_by_zero_is_noop() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["first".to_string(), "second".to_string()]);

    execute_line("shift 0", &mut executor).unwrap();

    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "first");
    assert_eq!(execute_line("echo $2", &mut executor).unwrap(), "second");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");
}

#[test]
fn test_shift_all_params() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
    ]);

    execute_line("shift 3", &mut executor).unwrap();

    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "0");
    assert_eq!(execute_line("echo $@", &mut executor).unwrap(), "");
}

// ============================================================================
// Error cases
// ============================================================================

#[test]
fn test_shift_empty_params_error() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![]);

    // shift with no params should fail
    let result = execute_line("shift", &mut executor);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds"));
}

#[test]
fn test_shift_beyond_available_params_error() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["a".to_string(), "b".to_string()]);

    // shift 3 when only 2 params exist should fail
    let result = execute_line("shift 3", &mut executor);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds"));
}

#[test]
fn test_shift_non_numeric_argument_error() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["a".to_string()]);

    let result = execute_line("shift abc", &mut executor);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("numeric argument required"));
}

#[test]
fn test_shift_too_many_arguments_error() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["a".to_string()]);

    let result = execute_line("shift 1 2", &mut executor);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too many arguments"));
}

// ============================================================================
// $# decrement tests
// ============================================================================

#[test]
fn test_param_count_decremented_correctly() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "1".to_string(),
        "2".to_string(),
        "3".to_string(),
        "4".to_string(),
        "5".to_string(),
    ]);

    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "5");

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "4");

    execute_line("shift 2", &mut executor).unwrap();
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");

    execute_line("shift 2", &mut executor).unwrap();
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "0");
}

// ============================================================================
// Function scope tests
// ============================================================================

#[test]
fn test_shift_in_function_scope() {
    let mut executor = Executor::new();

    // Set global positional params
    executor
        .runtime_mut()
        .set_positional_params(vec!["global1".to_string(), "global2".to_string()]);

    // Define a function that uses shift
    execute_line("shift_test() { echo $1; shift; echo $1; }", &mut executor).unwrap();

    // Call the function with its own args
    let output = execute_line("shift_test func1 func2 func3", &mut executor).unwrap();

    // Function should see its own positional params
    // First echo should print func1, after shift it should print func2
    assert_eq!(output, "func1\nfunc2");

    // Global params should be unaffected
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "global1");
    assert_eq!(execute_line("echo $2", &mut executor).unwrap(), "global2");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");
}

#[test]
fn test_shift_global_scope() {
    let mut executor = Executor::new();

    executor.runtime_mut().set_positional_params(vec![
        "arg1".to_string(),
        "arg2".to_string(),
        "arg3".to_string(),
    ]);

    // Shift at global scope
    execute_line("shift", &mut executor).unwrap();

    // Verify global params were shifted
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "arg2");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "2");
}

// ============================================================================
// Loop processing tests (common pattern)
// ============================================================================

#[test]
fn test_shift_in_while_loop() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
    ]);

    // A common shell pattern: process all args with while and shift
    // Use test builtin without brackets for simpler parsing
    let output = execute_line(
        r#"while test $# -gt 0; do echo $1; shift; done"#,
        &mut executor,
    )
    .unwrap();

    assert_eq!(output, "first\nsecond\nthird");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "0");
}

// ============================================================================
// Double-digit positional parameters
// ============================================================================

#[test]
fn test_shift_with_many_params() {
    let mut executor = Executor::new();
    let args: Vec<String> = (1..=12).map(|i| format!("arg{}", i)).collect();
    executor.runtime_mut().set_positional_params(args);

    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "12");
    assert_eq!(execute_line("echo ${10}", &mut executor).unwrap(), "arg10");

    // Shift by 5
    execute_line("shift 5", &mut executor).unwrap();

    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "7");
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "arg6");
    assert_eq!(execute_line("echo ${7}", &mut executor).unwrap(), "arg12");
}

// ============================================================================
// Shift multiple times
// ============================================================================

#[test]
fn test_multiple_shifts() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ]);

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "b");

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "c");

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "d");

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "e");

    execute_line("shift", &mut executor).unwrap();
    assert_eq!(execute_line("echo $1", &mut executor).unwrap(), "");
    assert_eq!(execute_line("echo $#", &mut executor).unwrap(), "0");
}

// ============================================================================
// Help text test
// ============================================================================

#[test]
fn test_shift_help_available() {
    let mut executor = Executor::new();
    let output = execute_line("help shift", &mut executor).unwrap();

    assert!(output.contains("shift"));
    assert!(output.contains("Shift positional parameters"));
    assert!(output.contains("DESCRIPTION"));
    assert!(output.contains("USAGE"));
    assert!(output.contains("EXAMPLES"));
}
