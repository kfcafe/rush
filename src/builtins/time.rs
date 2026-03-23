use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use libc;
use std::cell::RefCell;
use std::process::{Command as StdCommand, Stdio};
use std::time::Instant;

/// Timing data for a single pipeline stage
#[derive(Debug, Clone)]
pub struct StageTiming {
    pub name: String,
    pub is_builtin: bool,
    pub elapsed: std::time::Duration,
}

/// Global timing collection state
struct TimingState {
    collecting: bool,
    timings: Vec<StageTiming>,
}

thread_local! {
    static TIMING_STATE: RefCell<TimingState> = RefCell::new(TimingState {
        collecting: false,
        timings: Vec::new(),
    });
}

/// Timing result for a command executed in a specific shell
#[derive(Debug, Clone)]
struct ShellTiming {
    shell: String,
    elapsed: std::time::Duration,
    success: bool,
}

/// The time builtin command
///
/// Measures and reports execution time of a command.
/// Shows real (wall-clock), user (CPU in user mode), and sys (CPU in kernel mode) time.
///
/// For pipelines, shows per-stage timing breakdown.
///
/// Usage:
///   time command [args...]
///   time --compare command [args...]
///   time pipeline
///
/// Output format (POSIX-like):
///   real    0m0.123s
///   user    0m0.100s
///   sys     0m0.020s
///
/// For pipelines:
///   Stage    Time      Type
///   ----------------------------
///   find     123.5ms   builtin
///   grep     45.2ms    builtin
///   wc       12.1ms    builtin
///   overhead 5.0ms     -
///   total    185.8ms   -
///
/// For comparison mode:
///   Command: echo hello
///
///   Rush:  0.001s
///   Bash:  0.002s (2.0x faster in Rush)
///   Zsh:   0.0015s (1.5x faster in Rush)
///
/// Examples:
///   time echo hello
///   time sleep 0.1
///   time ls | wc -l
///   time --compare echo hello
pub fn builtin_time(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Need at least a command to time
    if args.is_empty() {
        return Err(anyhow!("time: usage: time [--compare] command [args...]"));
    }

    // Check for --compare flag
    let (use_compare, cmd_args) = if args[0] == "--compare" {
        if args.len() < 2 {
            return Err(anyhow!("time: --compare requires a command"));
        }
        (true, &args[1..])
    } else {
        (false, args)
    };

    if use_compare {
        return builtin_time_compare(cmd_args, runtime);
    }

    // Original single-shell timing logic
    let command_string = cmd_args.join(" ");

    // Parse and execute the command string
    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    // Tokenize the command string
    let tokens =
        Lexer::tokenize(&command_string).map_err(|e| anyhow!("time: tokenize error: {}", e))?;

    // Parse the tokens into statements
    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("time: parse error: {}", e))?;

    // Check if this is a pipeline
    let is_pipeline = statements.len() == 1
        && matches!(&statements[0], crate::parser::ast::Statement::Pipeline(_));

    // Record the start time (for wall-clock time)
    let start_time = Instant::now();

    // Get CPU time before execution (if available on this platform)
    let cpu_time_before = get_cpu_time();

    // Enable timing collection if pipeline
    if is_pipeline {
        TIMING_STATE.with(|ts| {
            let mut state = ts.borrow_mut();
            state.collecting = true;
            state.timings.clear();
        });
    }

    // Create a temporary executor with the current runtime state
    let mut executor = Executor::new_embedded();

    // Copy the current runtime state into the executor
    *executor.runtime_mut() = runtime.clone();

    // Execute the parsed statements
    let result = executor
        .execute(statements)
        .map_err(|e| anyhow!("time: execution error: {}", e))?;

    // Record the end time
    let elapsed_real = start_time.elapsed();

    // Get CPU time after execution
    let cpu_time_after = get_cpu_time();

    // Disable timing collection
    TIMING_STATE.with(|ts| {
        ts.borrow_mut().collecting = false;
    });

    // Copy back the runtime state to preserve variable changes
    *runtime = executor.runtime_mut().clone();

    // Calculate timing information
    let (user_time, sys_time) =
        if let (Some(before), Some(after)) = (cpu_time_before, cpu_time_after) {
            (after.user - before.user, after.sys - before.sys)
        } else {
            // Fallback: estimate from elapsed time (not accurate but better than nothing)
            (elapsed_real, std::time::Duration::ZERO)
        };

    // Format timing output
    let timing_output = if is_pipeline {
        TIMING_STATE.with(|ts| {
            let state = ts.borrow();
            format_pipeline_timing(&state.timings, elapsed_real)
        })
    } else {
        format_timing(elapsed_real, user_time, sys_time)
    };

    // Combine the command output with timing information
    let combined_output = format!("{}{}", result.stdout(), timing_output);

    // Return result with timing appended to output, but preserve stderr and exit code
    Ok(ExecutionResult {
        output: Output::Text(combined_output),
        stderr: result.stderr,
        exit_code: result.exit_code,
        error: result.error,
    })
}

/// Execute command in comparison mode across shells
fn builtin_time_compare(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let command_string = args.join(" ");

    // Run the command in Rush first
    let rush_timing = time_command_in_rush(&command_string, runtime)?;

    // Run in bash if available
    let bash_timing = time_command_in_shell("bash", &command_string);

    // Run in zsh if available
    let zsh_timing = time_command_in_shell("zsh", &command_string);

    // Format and display comparison results
    let output = format_comparison_results(
        &command_string,
        &rush_timing,
        bash_timing.as_ref(),
        zsh_timing.as_ref(),
    );

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

/// Time a command by executing it in Rush
fn time_command_in_rush(command: &str, runtime: &mut Runtime) -> Result<ShellTiming> {
    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    let start = Instant::now();

    // Tokenize and parse
    let tokens = Lexer::tokenize(command).map_err(|e| anyhow!("Rush timing error: {}", e))?;

    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("Rush timing error: {}", e))?;

    // Create executor and execute
    let mut executor = Executor::new_embedded();
    *executor.runtime_mut() = runtime.clone();

    match executor.execute(statements) {
        Ok(_result) => {
            let elapsed = start.elapsed();
            Ok(ShellTiming {
                shell: "Rush".to_string(),
                elapsed,
                success: true,
            })
        }
        Err(_) => {
            let elapsed = start.elapsed();
            Ok(ShellTiming {
                shell: "Rush".to_string(),
                elapsed,
                success: false,
            })
        }
    }
}

/// Time a command by executing it in an external shell (bash, zsh, etc.)
fn time_command_in_shell(shell_name: &str, command: &str) -> Option<ShellTiming> {
    // Check if shell exists
    let shell_check = StdCommand::new("which")
        .arg(shell_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !matches!(shell_check, Ok(status) if status.success()) {
        return None;
    }

    let start = Instant::now();

    // Execute command in the specified shell
    let status = StdCommand::new(shell_name)
        .arg("-c")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let elapsed = start.elapsed();

    match status {
        Ok(exit_status) => Some(ShellTiming {
            shell: capitalize_shell_name(shell_name),
            elapsed,
            success: exit_status.success(),
        }),
        Err(_) => None,
    }
}

/// Helper to capitalize shell names for display
fn capitalize_shell_name(shell: &str) -> String {
    if shell.is_empty() {
        String::new()
    } else {
        let mut chars = shell.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => {
                let rest: String = chars.collect();
                format!("{}{}", first.to_uppercase(), rest)
            }
        }
    }
}

/// Format comparison results across multiple shells
fn format_comparison_results(
    command: &str,
    rush: &ShellTiming,
    bash: Option<&ShellTiming>,
    zsh: Option<&ShellTiming>,
) -> String {
    let mut output = String::new();

    output.push_str(&format!("Command: {}\n\n", command));

    // Display Rush timing
    output.push_str(&format!("Rush:  {}\n", format_duration_short(rush.elapsed)));

    // Display Bash timing with speedup ratio if available
    if let Some(bash_timing) = bash {
        let speedup = if bash_timing.elapsed.as_secs_f64() > 0.0 {
            rush.elapsed.as_secs_f64() / bash_timing.elapsed.as_secs_f64()
        } else {
            1.0
        };

        let speedup_text = if speedup > 1.0 {
            format!(" ({:.1}x faster in Rush)", speedup)
        } else if speedup < 1.0 {
            format!(" ({:.1}x slower in Rush)", 1.0 / speedup)
        } else {
            String::new()
        };

        output.push_str(&format!(
            "Bash:  {}{}\n",
            format_duration_short(bash_timing.elapsed),
            speedup_text
        ));
    }

    // Display Zsh timing with speedup ratio if available
    if let Some(zsh_timing) = zsh {
        let speedup = if zsh_timing.elapsed.as_secs_f64() > 0.0 {
            rush.elapsed.as_secs_f64() / zsh_timing.elapsed.as_secs_f64()
        } else {
            1.0
        };

        let speedup_text = if speedup > 1.0 {
            format!(" ({:.1}x faster in Rush)", speedup)
        } else if speedup < 1.0 {
            format!(" ({:.1}x slower in Rush)", 1.0 / speedup)
        } else {
            String::new()
        };

        output.push_str(&format!(
            "Zsh:   {}{}\n",
            format_duration_short(zsh_timing.elapsed),
            speedup_text
        ));
    }

    output
}

/// Format a duration as a short string (seconds or milliseconds)
fn format_duration_short(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 0.001 {
        format!("{:.3}ms", secs * 1000.0)
    } else if secs < 1.0 {
        format!("{:.3}s", secs)
    } else {
        format!("{:.3}s", secs)
    }
}

/// Record timing for a pipeline stage
pub fn record_stage_timing(name: String, is_builtin: bool, elapsed: std::time::Duration) {
    TIMING_STATE.with(|ts| {
        let mut state = ts.borrow_mut();
        if state.collecting {
            state.timings.push(StageTiming {
                name,
                is_builtin,
                elapsed,
            });
        }
    });
}

/// Check if timing collection is enabled
pub fn is_collecting_timing() -> bool {
    TIMING_STATE.with(|ts| ts.borrow().collecting)
}

/// Format pipeline timing output as a table
fn format_pipeline_timing(timings: &[StageTiming], total_elapsed: std::time::Duration) -> String {
    if timings.is_empty() {
        // Fallback to POSIX format if no stage timings were collected
        return format_timing(total_elapsed, total_elapsed, std::time::Duration::ZERO);
    }

    let mut output = String::new();

    // Calculate overhead (total time - sum of all stages)
    let stages_total: std::time::Duration = timings.iter().map(|t| t.elapsed).sum();
    let overhead = if total_elapsed > stages_total {
        total_elapsed - stages_total
    } else {
        std::time::Duration::ZERO
    };

    // Header
    output.push_str("Stage\t\tTime\t\tType\n");
    output.push_str("─────────────────────────────────────\n");

    // Stage entries
    for timing in timings {
        let time_str = format_duration_ms(timing.elapsed);
        let type_str = if timing.is_builtin {
            "builtin"
        } else {
            "external"
        };
        output.push_str(&format!(
            "{}\t\t{}\t\t{}\n",
            timing.name, time_str, type_str
        ));
    }

    // Overhead line
    let overhead_str = format_duration_ms(overhead);
    output.push_str(&format!("overhead\t{}\t\t-\n", overhead_str));

    // Total line
    let total_str = format_duration_ms(total_elapsed);
    output.push_str(&format!("total\t\t{}\t\t-\n", total_str));

    output
}

/// Format a duration as milliseconds with appropriate precision
fn format_duration_ms(d: std::time::Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 1.0 {
        format!("{:.2}ms", ms)
    } else if ms < 1000.0 {
        format!("{:.1}ms", ms)
    } else {
        let secs = d.as_secs_f64();
        format!("{:.3}s", secs)
    }
}

/// CPU timing information
#[derive(Debug, Clone, Copy)]
struct CpuTime {
    user: std::time::Duration,
    sys: std::time::Duration,
}

/// Get current CPU time usage (user + system)
/// Returns None if the platform doesn't support this
#[cfg(unix)]
fn get_cpu_time() -> Option<CpuTime> {
    use std::time::Duration;

    // Try to use getrusage for accurate CPU timing
    #[cfg(not(target_os = "macos"))]
    {
        unsafe {
            let mut usage: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) == 0 {
                let user = Duration::new(
                    usage.ru_utime.tv_sec as u64,
                    (usage.ru_utime.tv_usec as u32) * 1000,
                );
                let sys = Duration::new(
                    usage.ru_stime.tv_sec as u64,
                    (usage.ru_stime.tv_usec as u32) * 1000,
                );
                return Some(CpuTime { user, sys });
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        unsafe {
            let mut usage: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) == 0 {
                let user = Duration::new(
                    usage.ru_utime.tv_sec as u64,
                    usage.ru_utime.tv_usec as u32 * 1000,
                );
                let sys = Duration::new(
                    usage.ru_stime.tv_sec as u64,
                    usage.ru_stime.tv_usec as u32 * 1000,
                );
                return Some(CpuTime { user, sys });
            }
        }
    }

    None
}

#[cfg(not(unix))]
fn get_cpu_time() -> Option<CpuTime> {
    None
}

/// Format timing output in POSIX style
/// real    0m0.123s
/// user    0m0.100s
/// sys     0m0.020s
fn format_timing(
    real: std::time::Duration,
    user: std::time::Duration,
    sys: std::time::Duration,
) -> String {
    fn duration_to_posix(d: std::time::Duration) -> String {
        let total_secs = d.as_secs_f64();
        let minutes = (total_secs / 60.0).floor() as u64;
        let seconds = total_secs - (minutes as f64 * 60.0);
        format!("{}m{:.3}s", minutes, seconds)
    }

    format!(
        "real\t{}\nuser\t{}\nsys\t{}\n",
        duration_to_posix(real),
        duration_to_posix(user),
        duration_to_posix(sys)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_echo() {
        let mut runtime = Runtime::new();
        let args = vec!["echo".to_string(), "hello".to_string()];
        let result = builtin_time(&args, &mut runtime).unwrap();

        // Check that output contains the command output
        assert!(result.stdout().contains("hello"));
        // Check that output contains timing information
        assert!(result.stdout().contains("real"));
        assert!(result.stdout().contains("user"));
        assert!(result.stdout().contains("sys"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_time_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_time(&[], &mut runtime);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_time_false_command() {
        let mut runtime = Runtime::new();
        let args = vec!["false".to_string()];
        let result = builtin_time(&args, &mut runtime).unwrap();

        // false returns exit code 1
        assert_eq!(result.exit_code, 1);
        // But still shows timing
        assert!(result.stdout().contains("real"));
    }

    #[test]
    fn test_format_timing() {
        let real = std::time::Duration::from_millis(123);
        let user = std::time::Duration::from_millis(100);
        let sys = std::time::Duration::from_millis(20);

        let output = format_timing(real, user, sys);

        // Should contain the time format markers
        assert!(output.contains("real"));
        assert!(output.contains("user"));
        assert!(output.contains("sys"));
        // Should contain time values with 's' suffix
        assert!(output.contains("s"));
    }

    #[test]
    fn test_format_timing_with_minutes() {
        let real = std::time::Duration::from_secs(65);
        let user = std::time::Duration::from_secs(60);
        let sys = std::time::Duration::from_secs(5);

        let output = format_timing(real, user, sys);

        // Should show minutes
        assert!(output.contains("1m"));
    }

    #[test]
    fn test_format_duration_ms_ms() {
        // Test milliseconds
        let d = std::time::Duration::from_millis(500);
        let output = format_duration_ms(d);
        assert!(output.contains("ms"));
        assert!(output.contains("500"));
    }

    #[test]
    fn test_format_duration_ms_seconds() {
        // Test seconds
        let d = std::time::Duration::from_secs_f64(1.5);
        let output = format_duration_ms(d);
        assert!(output.contains("s"));
    }

    #[test]
    fn test_format_duration_ms_sub_ms() {
        // Test sub-millisecond
        let d = std::time::Duration::from_micros(500);
        let output = format_duration_ms(d);
        assert!(output.contains("ms"));
    }

    #[test]
    fn test_pipeline_timing_formatter_empty() {
        let timings: Vec<StageTiming> = vec![];
        let total = std::time::Duration::from_millis(100);
        let output = format_pipeline_timing(&timings, total);

        // Should fall back to POSIX format for empty timings
        assert!(output.contains("real"));
        assert!(output.contains("user"));
        assert!(output.contains("sys"));
    }

    #[test]
    fn test_pipeline_timing_formatter_single_stage() {
        let timings = vec![StageTiming {
            name: "echo".to_string(),
            is_builtin: true,
            elapsed: std::time::Duration::from_millis(10),
        }];
        let total = std::time::Duration::from_millis(15);
        let output = format_pipeline_timing(&timings, total);

        // Should show stage name and type
        assert!(output.contains("echo"));
        assert!(output.contains("builtin"));
        assert!(output.contains("overhead"));
        assert!(output.contains("total"));
    }

    #[test]
    fn test_pipeline_timing_formatter_multi_stage() {
        let timings = vec![
            StageTiming {
                name: "find".to_string(),
                is_builtin: false,
                elapsed: std::time::Duration::from_millis(100),
            },
            StageTiming {
                name: "grep".to_string(),
                is_builtin: true,
                elapsed: std::time::Duration::from_millis(50),
            },
        ];
        let total = std::time::Duration::from_millis(160);
        let output = format_pipeline_timing(&timings, total);

        // Should show both stages
        assert!(output.contains("find"));
        assert!(output.contains("external"));
        assert!(output.contains("grep"));
        assert!(output.contains("builtin"));
        // Should show overhead calculation
        assert!(output.contains("overhead"));
        assert!(output.contains("total"));
    }

    #[test]
    fn test_is_collecting_timing_default() {
        // By default, timing collection should be disabled
        TIMING_STATE.with(|ts| {
            ts.borrow_mut().collecting = false;
        });
        assert!(!is_collecting_timing());
    }

    #[test]
    fn test_record_stage_timing_enabled() {
        // Enable timing collection
        TIMING_STATE.with(|ts| {
            let mut state = ts.borrow_mut();
            state.collecting = true;
            state.timings.clear();
        });

        record_stage_timing(
            "test".to_string(),
            true,
            std::time::Duration::from_millis(10),
        );

        // Check that timing was recorded
        TIMING_STATE.with(|ts| {
            let state = ts.borrow();
            assert_eq!(state.timings.len(), 1);
            assert_eq!(state.timings[0].name, "test");
            assert!(state.timings[0].is_builtin);
        });

        // Clean up
        TIMING_STATE.with(|ts| {
            let mut state = ts.borrow_mut();
            state.collecting = false;
            state.timings.clear();
        });
    }

    #[test]
    fn test_record_stage_timing_disabled() {
        // Disable timing collection
        TIMING_STATE.with(|ts| {
            let mut state = ts.borrow_mut();
            state.collecting = false;
            state.timings.clear();
        });

        record_stage_timing(
            "test".to_string(),
            true,
            std::time::Duration::from_millis(10),
        );

        // Check that timing was NOT recorded
        TIMING_STATE.with(|ts| {
            let state = ts.borrow();
            assert_eq!(state.timings.len(), 0);
        });
    }

    #[test]
    fn test_time_compare_flag() {
        let mut runtime = Runtime::new();
        let args = vec![
            "--compare".to_string(),
            "echo".to_string(),
            "hello".to_string(),
        ];
        let result = builtin_time(&args, &mut runtime).unwrap();

        // Should contain command in output
        assert!(result.stdout().contains("Command: echo hello"));
        // Should contain Rush timing
        assert!(result.stdout().contains("Rush:"));
    }

    #[test]
    fn test_time_compare_missing_command() {
        let mut runtime = Runtime::new();
        let args = vec!["--compare".to_string()];
        let result = builtin_time(&args, &mut runtime);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a command"));
    }

    #[test]
    fn test_format_comparison_results_basic() {
        let rush = ShellTiming {
            shell: "Rush".to_string(),
            elapsed: std::time::Duration::from_millis(1),
            success: true,
        };

        let output = format_comparison_results("echo hello", &rush, None, None);
        assert!(output.contains("Command: echo hello"));
        assert!(output.contains("Rush:"));
    }

    #[test]
    fn test_format_comparison_results_with_bash() {
        let rush = ShellTiming {
            shell: "Rush".to_string(),
            elapsed: std::time::Duration::from_millis(2),
            success: true,
        };

        let bash = ShellTiming {
            shell: "Bash".to_string(),
            elapsed: std::time::Duration::from_millis(4),
            success: true,
        };

        let output = format_comparison_results("echo hello", &rush, Some(&bash), None);
        assert!(output.contains("Command: echo hello"));
        assert!(output.contains("Rush:"));
        assert!(output.contains("Bash:"));
        // Should show 0.5x or slower in Rush
        assert!(output.contains("0.5") || output.contains("slower in Rush"));
    }

    #[test]
    fn test_capitalize_shell_name() {
        assert_eq!(capitalize_shell_name("bash"), "Bash");
        assert_eq!(capitalize_shell_name("zsh"), "Zsh");
        assert_eq!(capitalize_shell_name("sh"), "Sh");
        assert_eq!(capitalize_shell_name(""), "");
    }

    #[test]
    fn test_format_duration_short() {
        // Test milliseconds
        let d = std::time::Duration::from_millis(5);
        let output = format_duration_short(d);
        assert!(output.contains("s"));

        // Test seconds
        let d = std::time::Duration::from_secs_f64(1.5);
        let output = format_duration_short(d);
        assert!(output.contains("s"));
    }
}
