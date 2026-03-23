use crate::executor::{ExecutionResult, Output};
use crate::jobs::{Job, JobStatus};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// List all background jobs
pub fn builtin_jobs(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut output = String::new();

    // Update job statuses before listing
    runtime.job_manager().update_all_jobs();

    // Parse options
    let mut show_pids = false;
    let mut show_running = false;
    let mut show_stopped = false;

    for arg in args {
        match arg.as_str() {
            "-l" => show_pids = true,
            "-r" => show_running = true,
            "-s" => show_stopped = true,
            _ => {
                return Err(anyhow!("jobs: invalid option: {}", arg));
            }
        }
    }

    // Get all jobs sorted by ID
    let mut jobs = runtime.job_manager().list_jobs();
    jobs.sort_by_key(|j| j.id);

    if jobs.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    // Determine current and previous job for +/- indicators
    let current_id = runtime.job_manager().get_current_job().map(|j| j.id);
    let previous_id = runtime.job_manager().get_previous_job().map(|j| j.id);

    for job in jobs {
        // Filter by status if requested
        if show_running && job.status != JobStatus::Running {
            continue;
        }
        if show_stopped && job.status != JobStatus::Stopped {
            continue;
        }

        // POSIX +/- indicators
        let indicator = if Some(job.id) == current_id {
            "+"
        } else if Some(job.id) == previous_id {
            "-"
        } else {
            " "
        };

        // Running jobs show trailing &
        let cmd_suffix = if job.status == JobStatus::Running {
            " &"
        } else {
            ""
        };

        // Format output
        if show_pids {
            output.push_str(&format!(
                "[{}]{}  {} {}\t{}{}\n",
                job.id,
                indicator,
                job.pid,
                job.status.as_str(),
                job.command,
                cmd_suffix,
            ));
        } else {
            output.push_str(&format!(
                "[{}]{}  {}\t{}{}\n",
                job.id,
                indicator,
                job.status.as_str(),
                job.command,
                cmd_suffix,
            ));
        }
    }

    Ok(ExecutionResult::success(output))
}

/// Bring a job to foreground
pub fn builtin_fg(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    use nix::unistd::Pid;

    // Update job statuses
    runtime.job_manager().update_all_jobs();

    // Determine which job to foreground
    let job = if args.is_empty() {
        // Default to current job (most recent)
        runtime
            .job_manager()
            .get_current_job()
            .ok_or_else(|| anyhow!("fg: no current job"))?
    } else {
        let job_spec = &args[0];
        parse_job_spec(job_spec, runtime)?
    };

    let job_id = job.id;
    let pid = job.pid;
    let pgid = Pid::from_raw(job.pgid as i32);

    // Give the job's process group the terminal
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::tcsetpgrp(std::io::stdin().as_raw_fd(), pgid.as_raw());
        }
    }

    // If the job is stopped, continue it
    if job.status == JobStatus::Stopped {
        runtime
            .job_manager()
            .continue_job(job_id)
            .map_err(|e: String| anyhow!(e))?;
    }

    // Remove from job list
    runtime.job_manager().remove_job(job_id);

    // Wait for the process to complete in foreground
    let pid_nix = Pid::from_raw(pid as i32);

    // Print the command being foregrounded
    eprintln!("{}", job.command);

    let result = match waitpid(pid_nix, Some(WaitPidFlag::WUNTRACED)) {
        Ok(status) => {
            let exit_code = match status {
                WaitStatus::Exited(_, code) => code,
                WaitStatus::Signaled(_, sig, _) => 128 + sig as i32,
                WaitStatus::Stopped(_, sig) => {
                    // Job was stopped again (Ctrl+Z), add it back
                    let new_id = runtime.job_manager().add_job(pid, job.command.clone());
                    runtime
                        .job_manager()
                        .set_job_status(new_id, JobStatus::Stopped);
                    eprintln!("\n[{}]+  Stopped\t{}", new_id, job.command);
                    128 + sig as i32
                }
                _ => 0,
            };
            Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: String::new(),
                exit_code,
                error: None,
            })
        }
        Err(e) => Err(anyhow!("fg: failed to wait for job: {}", e)),
    };

    // Restore the shell's terminal control
    #[cfg(unix)]
    {
        use nix::unistd::getpgrp;
        use std::os::unix::io::AsRawFd;
        let shell_pgid = getpgrp();
        unsafe {
            libc::tcsetpgrp(std::io::stdin().as_raw_fd(), shell_pgid.as_raw());
        }
    }

    result
}

/// Continue a stopped job in background
pub fn builtin_bg(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Update job statuses
    runtime.job_manager().update_all_jobs();

    // Determine which job to background
    let job = if args.is_empty() {
        // Default to current job (most recent)
        runtime
            .job_manager()
            .get_current_job()
            .ok_or_else(|| anyhow!("bg: no current job"))?
    } else {
        let job_spec = &args[0];
        parse_job_spec(job_spec, runtime)?
    };

    // Only continue if stopped
    if job.status != JobStatus::Stopped {
        return Err(anyhow!("bg: job {} is not stopped", job.id));
    }

    // Continue the job
    runtime
        .job_manager()
        .continue_job(job.id)
        .map_err(|e: String| anyhow!(e))?;

    Ok(ExecutionResult::success(format!(
        "[{}]+  {} &\n",
        job.id, job.command
    )))
}

/// Parse job specification (e.g., %1, %%, %+, %-, %sleep, %?string)
fn parse_job_spec(spec: &str, runtime: &Runtime) -> Result<Job> {
    runtime
        .job_manager()
        .parse_job_spec(spec)
        .map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jobs_empty() {
        let mut runtime = Runtime::new();
        let result = builtin_jobs(&[], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_jobs_with_job() {
        let mut runtime = Runtime::new();
        runtime
            .job_manager()
            .add_job(12345, "sleep 100".to_string());

        let result = builtin_jobs(&[], &mut runtime).unwrap();
        assert!(result.stdout().contains("[1]"));
        assert!(result.stdout().contains("sleep 100"));
    }

    #[test]
    fn test_jobs_with_pid_flag() {
        let mut runtime = Runtime::new();
        runtime
            .job_manager()
            .add_job(12345, "sleep 100".to_string());

        let result = builtin_jobs(&["-l".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("12345"));
    }

    #[test]
    fn test_jobs_current_indicator() {
        let mut runtime = Runtime::new();
        runtime
            .job_manager()
            .add_job(12345, "sleep 100".to_string());
        runtime
            .job_manager()
            .add_job(12346, "sleep 200".to_string());

        let result = builtin_jobs(&[], &mut runtime).unwrap();
        let out = result.stdout();
        // Most recent job (id=2) should have + indicator
        assert!(out.contains("[2]+"));
        // Previous job (id=1) should have - indicator
        assert!(out.contains("[1]-"));
    }

    #[test]
    fn test_jobs_running_suffix() {
        let mut runtime = Runtime::new();
        // Spawn a real long-lived process so update_all_jobs sees it as Running
        use std::process::Command as StdCommand;
        let child = StdCommand::new("sleep").arg("100").spawn().unwrap();
        let pid = child.id();
        runtime.job_manager().add_job(pid, "sleep 100".to_string());

        let result = builtin_jobs(&[], &mut runtime).unwrap();
        // Running jobs should show & suffix
        assert!(
            result.stdout().contains(" &"),
            "Output was: {}",
            result.stdout()
        );

        // Clean up the spawned process
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
    }
}
