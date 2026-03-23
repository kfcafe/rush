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

#[test]
fn test_positional_param_count() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "arg1".to_string(),
        "arg2".to_string(),
        "arg3".to_string(),
    ]);

    let output = execute_line("echo $#", &mut executor).unwrap();
    assert_eq!(output, "3");
}

#[test]
fn test_positional_param_at() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "arg1".to_string(),
        "arg2".to_string(),
        "arg3".to_string(),
    ]);

    let output = execute_line("echo $@", &mut executor).unwrap();
    assert_eq!(output, "arg1 arg2 arg3");
}

#[test]
fn test_positional_param_star() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "arg1".to_string(),
        "arg2".to_string(),
        "arg3".to_string(),
    ]);

    let output = execute_line("echo $*", &mut executor).unwrap();
    assert_eq!(output, "arg1 arg2 arg3");
}

#[test]
fn test_positional_param_numbered() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
    ]);

    let output = execute_line("echo $1", &mut executor).unwrap();
    assert_eq!(output, "first");

    let output = execute_line("echo $2", &mut executor).unwrap();
    assert_eq!(output, "second");

    let output = execute_line("echo $3", &mut executor).unwrap();
    assert_eq!(output, "third");
}

#[test]
fn test_positional_param_zero() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("0".to_string(), "myscript.sh".to_string());

    let output = execute_line("echo $0", &mut executor).unwrap();
    assert_eq!(output, "myscript.sh");
}

#[test]
fn test_positional_param_double_digit() {
    let mut executor = Executor::new();
    let mut args = Vec::new();
    for i in 1..=15 {
        args.push(format!("arg{}", i));
    }
    executor.runtime_mut().set_positional_params(args);

    // ${10} should work
    let output = execute_line("echo ${10}", &mut executor).unwrap();
    assert_eq!(output, "arg10");

    // ${15} should work
    let output = execute_line("echo ${15}", &mut executor).unwrap();
    assert_eq!(output, "arg15");
}

#[test]
fn test_positional_param_shift() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
    ]);

    // Before shift
    let output = execute_line("echo $# $1", &mut executor).unwrap();
    assert_eq!(output, "4 a");

    // Shift once
    execute_line("shift", &mut executor).unwrap();
    let output = execute_line("echo $# $1", &mut executor).unwrap();
    assert_eq!(output, "3 b");

    // Shift twice
    execute_line("shift 2", &mut executor).unwrap();
    let output = execute_line("echo $# $1", &mut executor).unwrap();
    assert_eq!(output, "1 d");
}

#[test]
fn test_positional_param_empty() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![]);

    let output = execute_line("echo $#", &mut executor).unwrap();
    assert_eq!(output, "0");

    let output = execute_line("echo $@", &mut executor).unwrap();
    assert_eq!(output, "");

    let output = execute_line("echo $*", &mut executor).unwrap();
    assert_eq!(output, "");

    let output = execute_line("echo $1", &mut executor).unwrap();
    assert_eq!(output, "");
}

#[test]
fn test_positional_param_out_of_range() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["arg1".to_string(), "arg2".to_string()]);

    // $3 should be empty when we only have 2 args
    let output = execute_line("echo $3", &mut executor).unwrap();
    assert_eq!(output, "");

    // ${10} should be empty when we only have 2 args
    let output = execute_line("echo ${10}", &mut executor).unwrap();
    assert_eq!(output, "");
}

#[test]
fn test_positional_param_with_default() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_positional_params(vec!["arg1".to_string()]);

    // $2 doesn't exist, use default
    let output = execute_line("echo ${2:-default}", &mut executor).unwrap();
    assert_eq!(output, "default");

    // $1 exists, don't use default
    let output = execute_line("echo ${1:-default}", &mut executor).unwrap();
    assert_eq!(output, "arg1");
}

#[test]
fn test_positional_param_all_together() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("0".to_string(), "script.sh".to_string());
    executor.runtime_mut().set_positional_params(vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
    ]);

    // Test combining multiple positional params
    let output = execute_line("echo $# $1 $2 $3", &mut executor).unwrap();
    assert_eq!(output, "3 alpha beta gamma");
}

#[test]
#[ignore] // TODO: Function syntax not yet fully implemented
fn test_positional_param_function_scope() {
    let mut executor = Executor::new();

    // Set global positional params
    executor
        .runtime_mut()
        .set_positional_params(vec!["global1".to_string(), "global2".to_string()]);

    // Define a function
    execute_line("fn test_func() { echo $1 $2; }", &mut executor).unwrap();

    // Call function with different args - this should use function's positional params
    // Note: This test might not work until function call syntax is fully implemented
    // For now, we'll skip this test or mark it as TODO
}

#[test]
#[ignore] // TODO: ${#} syntax not working - lexer issue with # inside braces
fn test_positional_param_count_special() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ]);

    // ${#} should also work
    let output = execute_line("echo ${#}", &mut executor).unwrap();
    assert_eq!(output, "5");
}

#[test]
fn test_braced_positional_params() {
    let mut executor = Executor::new();
    executor.runtime_mut().set_positional_params(vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
    ]);

    // ${1}, ${2}, ${3} should work
    let output = execute_line("echo ${1}", &mut executor).unwrap();
    assert_eq!(output, "one");

    let output = execute_line("echo ${2}", &mut executor).unwrap();
    assert_eq!(output, "two");

    let output = execute_line("echo ${3}", &mut executor).unwrap();
    assert_eq!(output, "three");
}
