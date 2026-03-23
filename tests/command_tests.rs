use rush::executor::Executor;
use rush::parser::ast::{Argument, Command, FunctionDef, Statement};

#[test]
fn test_command_basic_echo() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("echo".to_string()),
            Argument::Literal("hello".to_string()),
            Argument::Literal("world".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.stdout(), "hello world\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_bypasses_function() {
    let mut executor = Executor::new();

    // Define a function named "echo" that uses builtin to avoid recursion
    let func_def = Statement::FunctionDef(FunctionDef {
        name: "echo".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "builtin".to_string(),
            args: vec![
                Argument::Literal("echo".to_string()),
                Argument::Literal("FUNCTION ECHO".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        })],
    });
    executor.execute(vec![func_def]).unwrap();

    // Verify the function exists and would be called normally
    let normal_call = Statement::Command(Command {
        name: "echo".to_string(),
        args: vec![Argument::Literal("test".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });
    let normal_result = executor.execute(vec![normal_call]).unwrap();
    assert_eq!(normal_result.stdout(), "FUNCTION ECHO\n");

    // Now use command to bypass the function
    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("echo".to_string()),
            Argument::Literal("test".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Should call the builtin echo, not the function
    assert_eq!(result.stdout(), "test\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_v_flag_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-v".to_string()),
            Argument::Literal("echo".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.stdout(), "echo\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_V_flag_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-V".to_string()),
            Argument::Literal("pwd".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.stdout(), "pwd is a shell builtin\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_v_flag_external() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-v".to_string()),
            Argument::Literal("sh".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Should print the path to sh
    assert!(result.stdout().starts_with('/'));
    assert!(result.stdout().contains("sh"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_V_flag_external() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-V".to_string()),
            Argument::Literal("sh".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Should print "sh is /path/to/sh"
    assert!(result.stdout().contains("sh is /"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_v_flag_not_found() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-v".to_string()),
            Argument::Literal("nonexistent_cmd_xyz".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 1);
    assert!(result.stderr.contains("not found"));
}

#[test]
fn test_command_no_arguments() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("usage"));
}

#[test]
fn test_command_invalid_flag() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-x".to_string()),
            Argument::Literal("echo".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid option"));
}

#[test]
fn test_command_combined_flags() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-pv".to_string()),
            Argument::Literal("sh".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Should use default path and print short description
    assert!(result.stdout().contains("sh"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_double_dash() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("--".to_string()),
            Argument::Literal("echo".to_string()),
            Argument::Literal("test".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.stdout(), "test\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_pwd_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![Argument::Literal("pwd".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert!(!result.stdout().is_empty());
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_true_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![Argument::Literal("true".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_false_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![Argument::Literal("false".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 1);
}

#[test]
fn test_command_nonexistent() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![Argument::Literal("nonexistent_cmd_xyz".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_command_with_function_shadowing_builtin() {
    let mut executor = Executor::new();

    // Define a function named "pwd" that would normally override the builtin
    let func_def = Statement::FunctionDef(FunctionDef {
        name: "pwd".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Literal("/fake/path".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    });
    executor.execute(vec![func_def]).unwrap();

    // Verify the function is called normally
    let normal_call = Statement::Command(Command {
        name: "pwd".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    });
    let normal_result = executor.execute(vec![normal_call]).unwrap();
    assert_eq!(normal_result.stdout(), "/fake/path\n");

    // Use command to bypass the function
    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![Argument::Literal("pwd".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Should call the builtin pwd, not the function
    assert_ne!(result.stdout(), "/fake/path\n");
    assert!(result.stdout().contains('/'));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_v_flag_with_function() {
    let mut executor = Executor::new();

    // Define a function named "myfunc"
    let func_def = Statement::FunctionDef(FunctionDef {
        name: "myfunc".to_string(),
        params: vec![],
        body: vec![],
    });
    executor.execute(vec![func_def]).unwrap();

    // command -v should report the builtin if it exists, not the function
    // Since myfunc is not a builtin, command -v should report not found
    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-v".to_string()),
            Argument::Literal("myfunc".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // Functions are bypassed, so it should not be found
    assert_eq!(result.exit_code, 1);
    assert!(result.stderr.contains("not found"));
}

#[test]
fn test_command_p_flag_uses_default_path() {
    let mut executor = Executor::new();

    // Test with -p flag (should use default PATH)
    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("-pV".to_string()),
            Argument::Literal("ls".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    // With -p, it might find ls in default PATH if it's a builtin or external
    // Since ls is a builtin in rush, it should report as builtin
    assert!(result.stdout().contains("ls") || result.stdout().contains("builtin"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_command_multiple_args_to_builtin() {
    let mut executor = Executor::new();

    let cmd = Statement::Command(Command {
        name: "command".to_string(),
        args: vec![
            Argument::Literal("echo".to_string()),
            Argument::Literal("one".to_string()),
            Argument::Literal("two".to_string()),
            Argument::Literal("three".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.stdout(), "one two three\n");
    assert_eq!(result.exit_code, 0);
}
