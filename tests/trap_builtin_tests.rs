use rush::builtins::trap::TrapSignal;
use rush::executor::Executor;
use rush::parser::ast::{Argument, Command, Statement};

#[test]
fn test_trap_basic_set_and_list() {
    let mut executor = Executor::new();

    // Set a trap for EXIT
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo cleanup".to_string()),
            Argument::Literal("EXIT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify the trap was set
    let trap = executor.runtime_mut().get_trap(TrapSignal::Exit);
    assert_eq!(trap, Some(&"echo cleanup".to_string()));

    // List all traps
    let list_cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![list_cmd]).unwrap();
    assert!(result.stdout().contains("EXIT"));
    assert!(result.stdout().contains("cleanup"));
}

#[test]
fn test_trap_set_multiple_signals() {
    let mut executor = Executor::new();

    // Set trap for INT, TERM, HUP with same handler
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo interrupted".to_string()),
            Argument::Literal("INT".to_string()),
            Argument::Literal("TERM".to_string()),
            Argument::Literal("HUP".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify all traps were set
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&"echo interrupted".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Term),
        Some(&"echo interrupted".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Hup),
        Some(&"echo interrupted".to_string())
    );
}

#[test]
fn test_trap_reset_to_default() {
    let mut executor = Executor::new();

    // Set a trap
    let set_cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo handler".to_string()),
            Argument::Literal("INT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    executor.execute(vec![set_cmd]).unwrap();
    assert!(executor.runtime_mut().get_trap(TrapSignal::Int).is_some());

    // Reset to default with -
    let reset_cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("-".to_string()),
            Argument::Literal("INT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![reset_cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify trap was removed
    assert_eq!(executor.runtime_mut().get_trap(TrapSignal::Int), None);
}

#[test]
fn test_trap_ignore_signal() {
    let mut executor = Executor::new();

    // Ignore signal with empty string
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("".to_string()),
            Argument::Literal("INT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify trap was set to empty (ignore)
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&String::new())
    );
}

#[test]
fn test_trap_list_signals() {
    let mut executor = Executor::new();

    // List available signals with -l flag
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![Argument::Literal("-l".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify output contains signal names
    assert!(result.stdout().contains("INT"));
    assert!(result.stdout().contains("TERM"));
    assert!(result.stdout().contains("HUP"));
    assert!(result.stdout().contains("EXIT"));
    assert!(result.stdout().contains("ERR"));
}

#[test]
fn test_trap_with_signal_numbers() {
    let mut executor = Executor::new();

    // Set trap using signal number instead of name
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo sigint".to_string()),
            Argument::Literal("2".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify trap was set (2 is SIGINT)
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&"echo sigint".to_string())
    );
}

#[test]
fn test_trap_exit_special_signal() {
    let mut executor = Executor::new();

    // Set EXIT trap (runs on shell exit)
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo exiting".to_string()),
            Argument::Literal("EXIT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify EXIT trap was set
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Exit),
        Some(&"echo exiting".to_string())
    );
}

#[test]
fn test_trap_err_special_signal() {
    let mut executor = Executor::new();

    // Set ERR trap (runs when command fails)
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo error occurred".to_string()),
            Argument::Literal("ERR".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify ERR trap was set
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Err),
        Some(&"echo error occurred".to_string())
    );
}

#[test]
fn test_trap_invalid_signal() {
    let mut executor = Executor::new();

    // Try to set trap with invalid signal name
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo test".to_string()),
            Argument::Literal("INVALID".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid signal"));
}

#[test]
fn test_trap_single_arg_error() {
    let mut executor = Executor::new();

    // trap with single arg (not -l or -p) should error
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![Argument::Literal("echo test".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("usage"));
}

#[test]
fn test_trap_list_with_p_flag() {
    let mut executor = Executor::new();

    // Set a trap first
    let set_cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo cleanup".to_string()),
            Argument::Literal("EXIT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });
    executor.execute(vec![set_cmd]).unwrap();

    // List with -p flag
    let list_cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![Argument::Literal("-p".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![list_cmd]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("EXIT"));
    assert!(result.stdout().contains("cleanup"));
}

#[test]
fn test_trap_override_existing() {
    let mut executor = Executor::new();

    // Set initial trap
    let cmd1 = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo first".to_string()),
            Argument::Literal("INT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });
    executor.execute(vec![cmd1]).unwrap();

    // Override with new trap
    let cmd2 = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo second".to_string()),
            Argument::Literal("INT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });
    executor.execute(vec![cmd2]).unwrap();

    // Verify new trap replaced old one
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&"echo second".to_string())
    );
}

#[test]
fn test_trap_case_insensitive_signal_names() {
    let mut executor = Executor::new();

    // Test lowercase signal name
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo test".to_string()),
            Argument::Literal("int".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify trap was set correctly
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&"echo test".to_string())
    );
}

#[test]
fn test_trap_with_sigprefix() {
    let mut executor = Executor::new();

    // Test with SIG prefix (SIGINT instead of INT)
    let cmd = Statement::Command(Command {
        name: "trap".to_string(),
        args: vec![
            Argument::Literal("echo sigint".to_string()),
            Argument::Literal("SIGINT".to_string()),
        ],
        redirects: vec![],
        prefix_env: vec![],
    });

    let result = executor.execute(vec![cmd]).unwrap();
    assert_eq!(result.exit_code, 0);

    // Verify trap was set correctly
    assert_eq!(
        executor.runtime_mut().get_trap(TrapSignal::Int),
        Some(&"echo sigint".to_string())
    );
}
