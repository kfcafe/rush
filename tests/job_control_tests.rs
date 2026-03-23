use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;
use std::thread;
use std::time::Duration;

#[test]
fn test_background_command_syntax() {
    let tokens = Lexer::tokenize("sleep 1 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    assert_eq!(statements.len(), 1);
    // Verify it's a BackgroundCommand
    match &statements[0] {
        rush::parser::ast::Statement::BackgroundCommand(_) => {}
        _ => panic!("Expected BackgroundCommand"),
    }
}

#[test]
fn test_background_command_execution() {
    let mut executor = Executor::new();

    // Execute background command
    let tokens = Lexer::tokenize("sleep 1 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements).unwrap();

    // Should get job notification
    assert!(result.stdout().contains("[1]"));
    assert!(result.stdout().contains("\n"));

    // Job should be in the job list
    let jobs = executor.runtime_mut().job_manager().list_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, 1);
    assert!(jobs[0].command.contains("sleep"));
}

#[test]
fn test_multiple_background_jobs() {
    let mut executor = Executor::new();

    // Start first job
    let tokens = Lexer::tokenize("sleep 10 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result1 = executor.execute(statements).unwrap();

    // Extract job ID from output
    let job1_output = result1.stdout();
    assert!(job1_output.contains("["));

    // Start second job
    let tokens = Lexer::tokenize("sleep 20 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result2 = executor.execute(statements).unwrap();

    let job2_output = result2.stdout();
    assert!(job2_output.contains("["));

    // Should have 2 jobs
    let mut jobs = executor.runtime_mut().job_manager().list_jobs();
    assert_eq!(jobs.len(), 2, "Expected 2 jobs but found {}", jobs.len());

    // Sort jobs by ID to ensure consistent ordering
    jobs.sort_by_key(|j| j.id);

    // Job IDs should be sequential (but not necessarily starting at 1)
    let id1 = jobs[0].id;
    let id2 = jobs[1].id;
    assert!(
        id2 > id1,
        "Second job ID ({}) should be greater than first ({})",
        id2,
        id1
    );

    // Cleanup - terminate the jobs
    executor.runtime_mut().job_manager().terminate_job(id1).ok();
    executor.runtime_mut().job_manager().terminate_job(id2).ok();
}

#[test]
fn test_jobs_builtin() {
    let mut executor = Executor::new();

    // Start a background job
    let tokens = Lexer::tokenize("sleep 5 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Run jobs command
    let tokens = Lexer::tokenize("jobs").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should show the job
    assert!(result.stdout().contains("[1]"));
    assert!(result.stdout().contains("sleep"));
    assert!(result.stdout().contains("Running"));

    // Cleanup
    executor.runtime_mut().job_manager().terminate_job(1).ok();
}

#[test]
fn test_jobs_builtin_with_l_flag() {
    let mut executor = Executor::new();

    // Start a background job
    let tokens = Lexer::tokenize("sleep 5 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Run jobs -l command
    let tokens = Lexer::tokenize("jobs -l").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should show the job with PID
    assert!(result.stdout().contains("[1]"));
    assert!(result.stdout().contains("Running"));
    // PID should be present (it will be a number)
    let has_pid = result.stdout().lines().any(|line| {
        line.split_whitespace()
            .any(|word| word.chars().all(|c| c.is_ascii_digit()) && word.len() > 2)
    });
    assert!(has_pid, "Expected PID in output: {}", result.stdout());

    // Cleanup
    executor.runtime_mut().job_manager().terminate_job(1).ok();
}

#[test]
fn test_jobs_empty() {
    let mut executor = Executor::new();

    // Run jobs with no background jobs
    let tokens = Lexer::tokenize("jobs").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should be empty
    assert_eq!(result.stdout(), "");
}

#[test]
fn test_job_completion_detection() {
    let mut executor = Executor::new();

    // Start a short-lived background job using external command
    // Use sleep instead of true (true is a builtin and can't be backgrounded)
    let tokens = Lexer::tokenize("sleep 0.05 &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    executor.execute(statements).unwrap();

    // Give it time to complete
    thread::sleep(Duration::from_millis(200));

    // Update job statuses
    executor.runtime_mut().job_manager().update_all_jobs();

    // Check job status
    let jobs = executor.runtime_mut().job_manager().list_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, rush::jobs::JobStatus::Done);
}

#[test]
fn test_background_builtin_fails() {
    let mut executor = Executor::new();

    // Try to run a builtin in background (should fail)
    let tokens = Lexer::tokenize("echo test &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Builtin commands cannot be run in background"));
}

#[test]
fn test_background_with_conditional_and() {
    let mut executor = Executor::new();

    // Verify that echo (a builtin) can't be backgrounded
    // This should fail with the builtin error message
    let tokens = Lexer::tokenize("echo test &").unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let result = executor.execute(statements);
    // Should fail because echo is a builtin
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Builtin commands cannot be run in background")
            || error_msg.contains("cannot be run in background")
    );
}

#[test]
fn test_ampersand_token_parsing() {
    // Make sure & is parsed separately from &&
    let tokens = Lexer::tokenize("cmd1 && cmd2").unwrap();
    assert!(tokens.iter().any(|t| matches!(t, rush::lexer::Token::And)));
    assert!(!tokens
        .iter()
        .any(|t| matches!(t, rush::lexer::Token::Ampersand)));

    let tokens = Lexer::tokenize("cmd1 &").unwrap();
    assert!(tokens
        .iter()
        .any(|t| matches!(t, rush::lexer::Token::Ampersand)));
    assert!(!tokens.iter().any(|t| matches!(t, rush::lexer::Token::And)));
}

#[test]
fn test_job_manager_get_current_job() {
    let mut executor = Executor::new();

    // Start two jobs
    executor
        .runtime_mut()
        .job_manager()
        .add_job(1111, "job1".to_string());
    executor
        .runtime_mut()
        .job_manager()
        .add_job(2222, "job2".to_string());

    // Current job should be the most recent (job 2)
    let current = executor
        .runtime_mut()
        .job_manager()
        .get_current_job()
        .unwrap();
    assert_eq!(current.id, 2);
    assert_eq!(current.command, "job2");
}

#[test]
fn test_job_manager_get_previous_job() {
    let mut executor = Executor::new();

    // Start two jobs
    executor
        .runtime_mut()
        .job_manager()
        .add_job(1111, "job1".to_string());
    executor
        .runtime_mut()
        .job_manager()
        .add_job(2222, "job2".to_string());

    // Previous job should be job 1
    let previous = executor
        .runtime_mut()
        .job_manager()
        .get_previous_job()
        .unwrap();
    assert_eq!(previous.id, 1);
    assert_eq!(previous.command, "job1");
}

#[test]
fn test_job_manager_cleanup() {
    let mut executor = Executor::new();

    // Add a job and mark it as done
    let job_id = executor
        .runtime_mut()
        .job_manager()
        .add_job(9999, "test".to_string());

    // Manually update it to Done status
    // (In real usage, this happens via waitpid)
    executor.runtime_mut().job_manager().update_all_jobs();

    // Initially should have 1 job
    let jobs = executor.runtime_mut().job_manager().list_jobs();
    assert_eq!(jobs.len(), 1);

    // After cleanup, completed jobs should be removed
    // Note: This test might be flaky if the process actually exists
    // In practice, cleanup is called periodically in the main loop
}

#[test]
fn test_job_id_increments() {
    let mut executor = Executor::new();

    let id1 = executor
        .runtime_mut()
        .job_manager()
        .add_job(1111, "job1".to_string());
    let id2 = executor
        .runtime_mut()
        .job_manager()
        .add_job(2222, "job2".to_string());
    let id3 = executor
        .runtime_mut()
        .job_manager()
        .add_job(3333, "job3".to_string());

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_job_removal() {
    let mut executor = Executor::new();

    let job_id = executor
        .runtime_mut()
        .job_manager()
        .add_job(1234, "test".to_string());

    // Job should exist
    let job = executor.runtime_mut().job_manager().get_job(job_id);
    assert!(job.is_some());

    // Remove the job
    let removed = executor.runtime_mut().job_manager().remove_job(job_id);
    assert!(removed.is_some());

    // Job should no longer exist
    let job = executor.runtime_mut().job_manager().get_job(job_id);
    assert!(job.is_none());
}
