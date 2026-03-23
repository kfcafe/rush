use rush::executor::Executor;
use rush::parser::ast::{Argument, Command, Statement};

#[test]
fn test_eval_basic_echo() {
    let mut executor = Executor::new();

    // Execute: eval echo hello world
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![
            Argument::Literal("echo".to_string()),
            Argument::Literal("hello".to_string()),
            Argument::Literal("world".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "hello world\n");
}

#[test]
fn test_eval_variable_expansion() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("cmd".to_string(), "echo".to_string());
    executor
        .runtime_mut()
        .set_variable("msg".to_string(), "hello".to_string());

    // Execute: eval $cmd $msg
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![
            Argument::Variable("cmd".to_string()),
            Argument::Variable("msg".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "hello\n");
}

#[test]
fn test_eval_command_substitution() {
    let mut executor = Executor::new();

    // Execute: eval "echo $(echo nested)"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("echo $(echo nested)".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "nested\n");
}

#[test]
fn test_eval_multiple_statements() {
    let mut executor = Executor::new();

    // Execute: eval "echo first ; echo second"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("echo first ; echo second".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "first\nsecond\n");
}

#[test]
fn test_eval_exit_code_propagation() {
    let mut executor = Executor::new();

    // Execute: eval false
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("false".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 1);
}

#[test]
fn test_eval_with_pipes() {
    let mut executor = Executor::new();

    // Execute: eval "echo hello | cat"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("echo hello | cat".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "hello\n");
}

#[test]
fn test_eval_with_and_operator() {
    let mut executor = Executor::new();

    // Execute: eval "true && echo success"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("true && echo success".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "success\n");
}

#[test]
fn test_eval_with_or_operator() {
    let mut executor = Executor::new();

    // Execute: eval "false || echo fallback"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("false || echo fallback".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "fallback\n");
}

#[test]
fn test_eval_no_arguments() {
    let mut executor = Executor::new();

    // Execute: eval (with no arguments)
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "");
}

#[test]
fn test_eval_with_test_builtin() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("VAR".to_string(), "value".to_string());

    // Execute: eval "test -n $VAR && echo yes"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("test -n $VAR && echo yes".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "yes\n");
}

#[test]
fn test_eval_concatenates_args() {
    let mut executor = Executor::new();

    // Execute: eval echo hello world (multiple args get concatenated)
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![
            Argument::Literal("echo".to_string()),
            Argument::Literal("hello".to_string()),
            Argument::Literal("world".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "hello world\n");
}

#[test]
fn test_eval_double_expansion() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("x".to_string(), "5".to_string());
    executor
        .runtime_mut()
        .set_variable("y".to_string(), "10".to_string());

    // Execute: eval "echo $x $y"
    // Variables are expanded by shell before eval, then eval expands again
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("echo $x $y".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "5 10\n");
}

#[test]
fn test_eval_with_pwd() {
    let mut executor = Executor::new();

    // Execute: eval pwd
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("pwd".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(!result.stdout().is_empty());
    assert!(result.stdout().ends_with('\n'));
}

#[test]
fn test_eval_sequential_commands() {
    let mut executor = Executor::new();

    // Execute: eval "echo a ; echo b ; echo c"
    let command = Statement::Command(Command {
        name: "eval".to_string(),
        args: vec![Argument::Literal("echo a ; echo b ; echo c".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![command]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "a\nb\nc\n");
}
