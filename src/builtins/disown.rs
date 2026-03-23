use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Remove jobs from the job table so they are no longer tracked by the shell.
///
/// Usage:
///   disown [JOBSPEC...]    — remove specified jobs (default: current job)
///   disown -a              — remove all jobs
///   disown -h [JOBSPEC...] — mark jobs so SIGHUP is not sent on shell exit
///                           (implemented as removal since no SIGHUP mechanism exists)
pub fn builtin_disown(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut remove_all = false;
    let mut no_hup = false; // -h flag (suppress SIGHUP)
    let mut jobspecs: Vec<&str> = Vec::new();

    // Parse flags and arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-a" => remove_all = true,
            "-h" => no_hup = true,
            arg if arg.starts_with('-') => {
                // Handle combined short flags like -ah
                let flags = &arg[1..];
                for ch in flags.chars() {
                    match ch {
                        'a' => remove_all = true,
                        'h' => no_hup = true,
                        _ => return Err(anyhow!("disown: -{}: invalid option", ch)),
                    }
                }
            }
            spec => jobspecs.push(spec),
        }
        i += 1;
    }

    // -h without -a and without jobspecs means current job
    let _ = no_hup; // acknowledged: no SIGHUP infrastructure; removal achieves the same effect

    if remove_all {
        // Remove every job from the table
        let all_jobs = runtime.job_manager().list_jobs();
        for job in all_jobs {
            runtime.job_manager().remove_job(job.id);
        }
        return Ok(ExecutionResult::success(String::new()));
    }

    if jobspecs.is_empty() {
        // Default: disown the current (most recent) job
        let job = runtime
            .job_manager()
            .get_current_job()
            .ok_or_else(|| anyhow!("disown: no current job"))?;
        runtime.job_manager().remove_job(job.id);
    } else {
        // Disown each specified job
        for spec in &jobspecs {
            let job = runtime
                .job_manager()
                .parse_job_spec(spec)
                .map_err(|e| anyhow!("disown: {}", e))?;
            runtime.job_manager().remove_job(job.id);
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn builtin_disown_removes_current_job() {
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 100".to_string());
        assert!(rt.job_manager().get_current_job().is_some());

        let result = builtin_disown(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(rt.job_manager().get_current_job().is_none());
    }

    #[test]
    fn builtin_disown_no_job_errors() {
        let mut rt = make_runtime();
        let result = builtin_disown(&[], &mut rt);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no current job"));
    }

    #[test]
    fn builtin_disown_all_removes_all_jobs() {
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 1".to_string());
        rt.job_manager().add_job(1002, "sleep 2".to_string());
        rt.job_manager().add_job(1003, "sleep 3".to_string());
        assert_eq!(rt.job_manager().list_jobs().len(), 3);

        let result = builtin_disown(&["-a".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(rt.job_manager().list_jobs().len(), 0);
    }

    #[test]
    fn builtin_disown_specific_jobspec() {
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 1".to_string());
        rt.job_manager().add_job(1002, "sleep 2".to_string());

        // Disown job %1 by spec
        let result = builtin_disown(&["%1".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(rt.job_manager().get_job(1).is_none());
        assert!(rt.job_manager().get_job(2).is_some());
    }

    #[test]
    fn builtin_disown_invalid_jobspec_errors() {
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 1".to_string());

        let result = builtin_disown(&["%99".to_string()], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn builtin_disown_h_flag_removes_job() {
        // -h has the same observable effect (job is disowned) since
        // no SIGHUP infrastructure is present in the runtime.
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 100".to_string());

        let result = builtin_disown(&["-h".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(rt.job_manager().get_current_job().is_none());
    }

    #[test]
    fn builtin_disown_combined_flags() {
        let mut rt = make_runtime();
        rt.job_manager().add_job(1001, "sleep 1".to_string());
        rt.job_manager().add_job(1002, "sleep 2".to_string());

        let result = builtin_disown(&["-ah".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(rt.job_manager().list_jobs().len(), 0);
    }

    #[test]
    fn builtin_disown_invalid_option_errors() {
        let mut rt = make_runtime();
        let result = builtin_disown(&["-z".to_string()], &mut rt);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid option"));
    }
}
