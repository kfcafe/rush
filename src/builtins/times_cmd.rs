use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::Result;

/// Print accumulated user and system CPU times for the shell process and its children.
///
/// POSIX output format (two lines):
///   <shell_user> <shell_sys>
///   <child_user> <child_sys>
///
/// Each time is formatted as: Xm Y.ZZZs
///
/// Example:
///   0m0.050s 0m0.010s
///   0m1.200s 0m0.300s
pub fn builtin_times(_args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let (shell_user, shell_sys, child_user, child_sys) = get_process_times();

    let output = format!(
        "{} {}\n{} {}\n",
        format_posix_time(shell_user),
        format_posix_time(shell_sys),
        format_posix_time(child_user),
        format_posix_time(child_sys),
    );

    Ok(ExecutionResult::success(output))
}

/// Format a duration in seconds as POSIX minutes+seconds: Xm Y.ZZZs
fn format_posix_time(secs: f64) -> String {
    let secs = secs.max(0.0);
    let minutes = (secs / 60.0).floor() as u64;
    let remaining = secs - (minutes as f64 * 60.0);
    format!("{}m{:.3}s", minutes, remaining)
}

/// Read shell and child CPU times via libc::times().
/// Returns (shell_user, shell_sys, child_user, child_sys) in seconds.
#[cfg(unix)]
fn get_process_times() -> (f64, f64, f64, f64) {
    unsafe {
        let mut tms: libc::tms = std::mem::zeroed();
        let clk_tck = libc::sysconf(libc::_SC_CLK_TCK) as f64;

        if libc::times(&mut tms) != !0 && clk_tck > 0.0 {
            let shell_user = tms.tms_utime as f64 / clk_tck;
            let shell_sys = tms.tms_stime as f64 / clk_tck;
            let child_user = tms.tms_cutime as f64 / clk_tck;
            let child_sys = tms.tms_cstime as f64 / clk_tck;
            (shell_user, shell_sys, child_user, child_sys)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }
}

#[cfg(not(unix))]
fn get_process_times() -> (f64, f64, f64, f64) {
    (0.0, 0.0, 0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_posix_time_zero() {
        assert_eq!(format_posix_time(0.0), "0m0.000s");
    }

    #[test]
    fn test_format_posix_time_seconds() {
        assert_eq!(format_posix_time(1.5), "0m1.500s");
    }

    #[test]
    fn test_format_posix_time_minutes() {
        assert_eq!(format_posix_time(90.0), "1m30.000s");
    }

    #[test]
    fn test_builtin_times_output_format() {
        let mut runtime = Runtime::new();
        let result = builtin_times(&[], &mut runtime).unwrap();
        let output = result.stdout();

        // Should have exactly two lines (plus trailing newline)
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "times should print exactly two lines");

        for line in lines {
            // Each line should have two time tokens: Xm Y.ZZZs
            let tokens: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(tokens.len(), 2, "each times line should have two tokens");
            for token in tokens {
                assert!(token.contains('m'), "time token should contain 'm'");
                assert!(token.ends_with('s'), "time token should end with 's'");
            }
        }
    }

    #[test]
    fn test_builtin_times_exit_code() {
        let mut runtime = Runtime::new();
        let result = builtin_times(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_builtin_times_ignores_args() {
        let mut runtime = Runtime::new();
        let args = vec!["ignored".to_string()];
        let result = builtin_times(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }
}
