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
fn test_shell_pid() {
    // Test $$ - should return the current process ID
    let mut executor = Executor::new();

    let output = execute_line("echo $$", &mut executor).unwrap();

    // Should be a valid PID (positive integer)
    let pid: u32 = output.parse().expect("$$ should be a valid number");
    assert!(pid > 0, "$$ should be a positive process ID");

    // Should be consistent across multiple calls
    let output2 = execute_line("echo $$", &mut executor).unwrap();
    assert_eq!(output, output2, "$$ should be consistent");
}

#[test]
fn test_shell_pid_braced() {
    // Test ${$} - should work the same as $$
    let mut executor = Executor::new();

    let output = execute_line("echo ${$}", &mut executor).unwrap();

    let pid: u32 = output.parse().expect("${{$}} should be a valid number");
    assert!(pid > 0, "${{$}} should be a positive process ID");
}

#[test]
fn test_last_bg_pid_empty() {
    // Test $! when no background jobs have been started
    let mut executor = Executor::new();

    // $! should be empty - test by checking if it's empty via assignment and default operator
    let output = execute_line("let X = $!; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(
        output, "EMPTY",
        "$! should be empty when no background jobs"
    );
}

#[test]
fn test_last_bg_pid_after_background_job() {
    // Test $! after starting a background job
    let mut executor = Executor::new();

    // Start a background job (sleep for a short time)
    let bg_output = execute_line("sleep 0.1 &", &mut executor).unwrap();

    // Extract PID from output like "[1] 12345"
    let parts: Vec<&str> = bg_output.split_whitespace().collect();
    assert!(parts.len() >= 2, "Should have job ID and PID");
    let expected_pid: u32 = parts[1].parse().expect("PID should be a number");

    // Now check $!
    let bang_output = execute_line("echo $!", &mut executor).unwrap();
    let bang_pid: u32 = bang_output.parse().expect("$! should be a number");

    assert_eq!(
        bang_pid, expected_pid,
        "$! should match the last background job PID"
    );
}

#[test]
fn test_last_bg_pid_updates() {
    // Test that $! updates with each new background job
    let mut executor = Executor::new();

    // Start first background job
    execute_line("sleep 0.1 &", &mut executor).unwrap();
    let pid1 = execute_line("echo $!", &mut executor).unwrap();

    // Start second background job
    execute_line("sleep 0.1 &", &mut executor).unwrap();
    let pid2 = execute_line("echo $!", &mut executor).unwrap();

    // PIDs should be different (different processes)
    assert_ne!(pid1, pid2, "$! should update to new background job PID");
}

#[test]
fn test_option_flags_empty() {
    // Test $- when no options are set
    let mut executor = Executor::new();

    // $- should be empty when no options set
    let output = execute_line("let X = $-; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(output, "EMPTY", "$- should be empty when no options set");
}

#[test]
fn test_option_flags_errexit() {
    // Test $- with errexit option
    let mut executor = Executor::new();

    let output = execute_line("set -e; echo $-", &mut executor).unwrap();

    assert!(
        output.contains('e'),
        "$- should contain 'e' when set -e is active"
    );
}

#[test]
fn test_option_flags_multiple() {
    // Test $- with multiple options (skip xtrace to avoid extra output)
    let mut executor = Executor::new();

    let output = execute_line("set -eu; echo $-", &mut executor).unwrap();

    assert!(output.contains('e'), "$- should contain 'e'");
    assert!(output.contains('u'), "$- should contain 'u'");
}

#[test]
fn test_last_arg_simple() {
    // Test $_ captures last argument
    let mut executor = Executor::new();

    execute_line("echo foo bar", &mut executor).unwrap();
    let output = execute_line("echo $_", &mut executor).unwrap();

    assert_eq!(output, "bar", "$_ should be 'bar' (last argument)");
}

#[test]
fn test_last_arg_single_arg() {
    // Test $_ with single argument
    let mut executor = Executor::new();

    execute_line("echo hello", &mut executor).unwrap();
    let output = execute_line("echo $_", &mut executor).unwrap();

    assert_eq!(output, "hello", "$_ should be 'hello'");
}

#[test]
fn test_last_arg_updates() {
    // Test that $_ updates with each command
    let mut executor = Executor::new();

    execute_line("echo first", &mut executor).unwrap();
    let output1 = execute_line("echo $_", &mut executor).unwrap();
    assert_eq!(output1, "first", "First $_ should be 'first'");

    execute_line("echo second", &mut executor).unwrap();
    let output2 = execute_line("echo $_", &mut executor).unwrap();
    assert_eq!(output2, "second", "Second $_ should be 'second'");
}

#[test]
fn test_last_arg_with_multiple_args() {
    // Test $_ captures the actual last argument
    let mut executor = Executor::new();

    execute_line("echo one two three", &mut executor).unwrap();
    let output = execute_line("echo $_", &mut executor).unwrap();

    assert_eq!(output, "three", "$_ should be 'three' (the last arg)");
}

#[test]
fn test_last_arg_empty_initial() {
    // Test $_ when no command has been run yet
    let mut executor = Executor::new();

    // Should be empty initially
    let output = execute_line("let X = $_; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(output, "EMPTY", "$_ should be empty initially");
}

#[test]
fn test_special_vars_in_expressions() {
    // Test that special variables work in various contexts
    let mut executor = Executor::new();

    // Test $$ in assignment
    let output = execute_line("let PID = $$; echo $PID", &mut executor).unwrap();
    let pid: u32 = output.parse().expect("Should be valid PID");
    assert!(pid > 0);

    // Test $- in assignment
    execute_line("set -e", &mut executor).unwrap();
    let output = execute_line("let FLAGS = $-; echo $FLAGS", &mut executor).unwrap();
    assert!(output.contains('e'));
}

#[test]
fn test_exit_code_still_works() {
    // Ensure $? still works (regression test)
    let mut executor = Executor::new();

    execute_line("echo test", &mut executor).unwrap();
    let output = execute_line("echo $?", &mut executor).unwrap();

    assert_eq!(output, "0", "$? should still work and be 0");
}

#[test]
fn test_braced_special_vars() {
    // Test braced versions of special variables
    let mut executor = Executor::new();

    // Test ${!}
    let output = execute_line("let X = ${!}; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(
        output, "EMPTY",
        "${{!}} should be empty when no background jobs"
    );

    // Test ${-}
    let output = execute_line("let X = ${-}; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(
        output, "EMPTY",
        "${{-}} should be empty when no options set"
    );

    // Test ${_}
    let output = execute_line("let X = ${_}; echo ${X:-EMPTY}", &mut executor).unwrap();
    assert_eq!(output, "EMPTY", "${{_}} should be empty initially");
}

#[test]
fn test_last_arg_with_builtins() {
    // Test $_ works with builtin commands
    let mut executor = Executor::new();

    execute_line("cd /tmp", &mut executor).unwrap();
    let output = execute_line("echo $_", &mut executor).unwrap();
    assert_eq!(output, "/tmp", "$_ should capture builtin arguments");
}
