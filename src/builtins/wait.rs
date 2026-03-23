use crate::executor::{ExecutionResult, Output};
use crate::jobs::JobStatus;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;

/// Wait for background jobs to complete
///
/// Usage:
///   wait           - wait for all background jobs
///   wait %1        - wait for job 1
///   wait 1234      - wait for PID 1234
///   wait %1 %2     - wait for multiple jobs
///
/// Returns the exit status of the last job waited for.
/// If waiting for multiple jobs, returns the exit status of the last one.
/// Returns 127 if the job/PID doesn't exist.
pub fn builtin_wait(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Update job statuses before waiting
    runtime.job_manager().update_all_jobs();

    // If no arguments, wait for all background jobs
    if args.is_empty() {
        return wait_all_jobs(runtime);
    }

    // Wait for specific jobs/PIDs
    let mut last_exit_code = 0;

    for arg in args {
        let exit_code = if arg.starts_with('%') {
            // Job specification (e.g., %1, %%, %+, %-)
            wait_for_job_spec(arg, runtime)?
        } else {
            // Try to parse as PID
            match arg.parse::<u32>() {
                Ok(pid) => wait_for_pid(pid, runtime)?,
                Err(_) => {
                    return Err(anyhow!("wait: '{}': not a pid or valid job spec", arg));
                }
            }
        };

        last_exit_code = exit_code;
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: last_exit_code,
        error: None,
    })
}

/// Wait for all background jobs to complete
fn wait_all_jobs(runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut last_exit_code = 0;

    loop {
        // Get all jobs
        let jobs = runtime.job_manager().list_jobs();

        // If no jobs left, we're done
        if jobs.is_empty() {
            break;
        }

        // Wait for any job to complete
        for job in jobs {
            // Skip already completed jobs
            if job.status == JobStatus::Done || job.status == JobStatus::Terminated {
                runtime.job_manager().remove_job(job.id);
                continue;
            }

            let pid = Pid::from_raw(job.pid as i32);

            // Try non-blocking wait first
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(status) => {
                    use nix::sys::wait::WaitStatus;
                    match status {
                        WaitStatus::Exited(_, code) => {
                            last_exit_code = code;
                            runtime.job_manager().remove_job(job.id);
                        }
                        WaitStatus::Signaled(_, sig, _) => {
                            last_exit_code = 128 + sig as i32;
                            runtime.job_manager().remove_job(job.id);
                        }
                        WaitStatus::StillAlive => {
                            // Job still running, we'll check again
                        }
                        _ => {
                            // Other statuses, remove job
                            runtime.job_manager().remove_job(job.id);
                        }
                    }
                }
                Err(_) => {
                    // Process doesn't exist or error, remove job
                    runtime.job_manager().remove_job(job.id);
                    last_exit_code = 127;
                }
            }
        }

        // Update job statuses
        runtime.job_manager().update_all_jobs();

        // Small sleep to avoid busy waiting
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: last_exit_code,
        error: None,
    })
}

/// Wait for a specific job by job specification
fn wait_for_job_spec(spec: &str, runtime: &mut Runtime) -> Result<i32> {
    // Parse job spec (reuse logic from jobs.rs)
    let job = parse_job_spec(spec, runtime)?;
    let job_id = job.id;
    let pid = job.pid;

    // Wait for the job
    let exit_code = wait_for_pid_blocking(pid)?;

    // Remove job from job list
    runtime.job_manager().remove_job(job_id);

    Ok(exit_code)
}

/// Wait for a specific PID
fn wait_for_pid(pid: u32, runtime: &mut Runtime) -> Result<i32> {
    // Check if this PID corresponds to a job
    if let Some(job) = runtime.job_manager().get_job_by_pid(pid) {
        let job_id = job.id;
        let exit_code = wait_for_pid_blocking(pid)?;
        runtime.job_manager().remove_job(job_id);
        Ok(exit_code)
    } else {
        // PID is not a background job, try to wait for it anyway
        wait_for_pid_blocking(pid)
    }
}

/// Wait for a PID to complete (blocking)
fn wait_for_pid_blocking(pid: u32) -> Result<i32> {
    let pid = Pid::from_raw(pid as i32);

    match waitpid(pid, None) {
        Ok(status) => {
            use nix::sys::wait::WaitStatus;
            let exit_code = match status {
                WaitStatus::Exited(_, code) => code,
                WaitStatus::Signaled(_, sig, _) => 128 + sig as i32,
                _ => 0,
            };
            Ok(exit_code)
        }
        Err(nix::errno::Errno::ECHILD) => {
            // Process doesn't exist or is not a child
            Err(anyhow!(
                "wait: pid {} is not a child of this shell",
                pid.as_raw()
            ))
        }
        Err(e) => Err(anyhow!(
            "wait: failed to wait for pid {}: {}",
            pid.as_raw(),
            e
        )),
    }
}

/// Parse job specification (e.g., %1, %%, %+, %-, %sleep, %?string)
fn parse_job_spec(spec: &str, runtime: &Runtime) -> Result<crate::jobs::Job> {
    runtime
        .job_manager()
        .parse_job_spec(spec)
        .map_err(|e| anyhow!("wait: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_wait_no_jobs() {
        let mut runtime = Runtime::new();
        let result = builtin_wait(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_wait_nonexistent_job() {
        let mut runtime = Runtime::new();
        let result = builtin_wait(&["%1".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .to_lowercase()
            .contains("no such job"));
    }

    #[test]
    fn test_wait_invalid_pid() {
        let mut runtime = Runtime::new();
        let result = builtin_wait(&["invalid".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a pid"));
    }

    #[test]
    fn test_wait_current_job_empty() {
        let mut runtime = Runtime::new();
        let result = builtin_wait(&["%%".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .to_lowercase()
            .contains("no current job"));
    }

    #[test]
    fn test_wait_previous_job_empty() {
        let mut runtime = Runtime::new();
        let result = builtin_wait(&["%-".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .to_lowercase()
            .contains("no previous job"));
    }

    #[test]
    fn test_parse_job_spec_invalid() {
        let runtime = Runtime::new();
        // "1" is parsed as job ID 1, which doesn't exist
        let result = parse_job_spec("1", &runtime);
        assert!(result.is_err());
        // Use a truly invalid spec to test invalid specification
        let result2 = parse_job_spec("abc", &runtime);
        assert!(result2.is_err());
    }

    #[test]
    fn test_parse_job_spec_nonexistent() {
        let runtime = Runtime::new();
        let result = parse_job_spec("%999", &runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_wait_with_job() {
        let mut runtime = Runtime::new();

        // Add a job that doesn't actually exist
        runtime.job_manager().add_job(1, "sleep".to_string());

        // Try to wait for the job (should timeout or fail)
        // Just verify it doesn't crash
        let result = builtin_wait(&["%1".to_string()], &mut runtime);

        // The result depends on whether the process exists, but should be handled gracefully
        // We're just verifying it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_wait_with_pid() {
        let mut runtime = Runtime::new();

        // Wait for a non-existent PID (should fail gracefully)
        let result = builtin_wait(&["99999".to_string()], &mut runtime);

        // Should fail because 99999 is not a child process
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a child"));
    }

    #[test]
    #[ignore = "Spawns processes that are not children of the shell"]
    fn test_wait_multiple_jobs() {
        let mut runtime = Runtime::new();

        // Spawn two child processes
        let child1 = Command::new("true").spawn().unwrap();
        let pid1 = child1.id() as i32;
        let child2 = Command::new("false").spawn().unwrap();
        let pid2 = child2.id() as i32;

        // Add them as jobs
        let job_id1 = runtime
            .job_manager()
            .add_job(pid1 as u32, "true".to_string());
        let job_id2 = runtime
            .job_manager()
            .add_job(pid2 as u32, "false".to_string());

        // Wait for both jobs by PIDs (not by job IDs since they're not true shell-spawned jobs)
        let result = builtin_wait(&[pid1.to_string(), pid2.to_string()], &mut runtime).unwrap();

        // Should return exit code of last job (false = 1)
        assert_eq!(result.exit_code, 1);

        // Both jobs should be removed
        assert!(runtime.job_manager().get_job(job_id1).is_none());
        assert!(runtime.job_manager().get_job(job_id2).is_none());
    }
}
