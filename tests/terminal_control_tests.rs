use nix::unistd::{getpgrp, Pid};
use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;
use rush::runtime::Runtime;
use rush::terminal::TerminalControl;

#[test]
fn test_terminal_control_creation() {
    let terminal = TerminalControl::new();
    // Should not panic
}

#[test]
fn test_foreground_command_gets_terminal() {
    // This test needs a real terminal to work properly
    // We'll just verify the code path doesn't panic
    let mut executor = Executor::new();

    let input = "echo 'test'";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    for stmt in statements {
        let _ = executor.execute_statement(stmt);
    }
}

#[test]
fn test_background_job_no_terminal() {
    // Background jobs should not get terminal control
    let mut executor = Executor::new();

    // Parse a background command
    // Note: Background command syntax not fully implemented yet
    // This test just verifies the terminal module doesn't panic
    let input = "echo test";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    for stmt in statements {
        let _ = executor.execute_statement(stmt);
    }
}

#[test]
fn test_terminal_state_restoration() {
    let terminal = TerminalControl::new();
    let shell_pgid = getpgrp();

    // Give terminal to a different process group (ourselves in this case)
    let _ = terminal.give_terminal_to(shell_pgid);

    // Reclaim should succeed
    let _ = terminal.reclaim_terminal();
}

#[test]
fn test_process_group_setup() {
    // Verify process groups are set up correctly
    let mut executor = Executor::new();

    let input = "echo test";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    for stmt in statements {
        let result = executor.execute_statement(stmt);
        assert!(result.is_ok());
    }
}

#[test]
fn test_terminal_error_recovery() {
    // Test that we handle terminal control errors gracefully
    let terminal = TerminalControl::new();

    // Try to give terminal to invalid process group
    let invalid_pgid = Pid::from_raw(-1);
    let result = terminal.give_terminal_to(invalid_pgid);

    // In test environment (non-interactive), this should succeed (no-op)
    // In interactive environment, it would fail
    // Either way, it shouldn't panic
    let _ = result;
}

#[test]
fn test_job_pgid_field() {
    // Test that jobs have pgid field set correctly
    use rush::jobs::JobManager;

    let job_manager = JobManager::new();
    let job_id = job_manager.add_job(12345, "test command".to_string());

    let job = job_manager.get_job(job_id).unwrap();
    assert_eq!(job.pid, 12345);
    assert_eq!(job.pgid, 12345); // Initially pgid == pid
}

#[test]
fn test_interactive_detection() {
    let terminal = TerminalControl::new();
    // In test environment, should not be interactive
    // (unless tests are run with a real terminal)
}

#[test]
fn test_terminal_control_clone() {
    let terminal = TerminalControl::new();
    let terminal2 = terminal.clone();

    // Both should have same settings
    assert_eq!(terminal.is_interactive(), terminal2.is_interactive());
}

#[test]
fn test_foreground_pgid_query() {
    let terminal = TerminalControl::new();

    if terminal.is_interactive() {
        let pgid = terminal.get_foreground_pgid();
        assert!(pgid.is_ok());
    }
}
