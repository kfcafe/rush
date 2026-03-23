use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::time::Duration;

pub fn builtin_sleep(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "sleep: missing operand\nTry 'sleep --help' for more information.\n"
                .to_string(),
            exit_code: 1,
            error: None,
        });
    }

    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    // Accumulate total sleep duration across all arguments (GNU sleep accepts multiple).
    let mut total = Duration::ZERO;
    for arg in args {
        match parse_duration(arg) {
            Ok(d) => total += d,
            Err(e) => {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: format!("{}\n", e),
                    exit_code: 1,
                    error: None,
                });
            }
        }
    }

    std::thread::sleep(total);

    Ok(ExecutionResult::success(String::new()))
}

/// Parse a duration string like `1`, `0.5`, `2s`, `1m`, `1h`, `1d`.
fn parse_duration(s: &str) -> Result<Duration> {
    if s.is_empty() {
        return Err(anyhow!("sleep: invalid time interval ''"));
    }

    // Determine suffix and numeric part.
    let (num_str, multiplier_secs) = if s.ends_with('s') {
        (&s[..s.len() - 1], 1.0f64)
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], 60.0f64)
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], 3600.0f64)
    } else if s.ends_with('d') {
        (&s[..s.len() - 1], 86400.0f64)
    } else {
        (s, 1.0f64)
    };

    let value: f64 = num_str
        .parse()
        .map_err(|_| anyhow!("sleep: invalid time interval '{}'", s))?;

    if value < 0.0 {
        return Err(anyhow!(
            "sleep: invalid time interval '{}': negative values not allowed",
            s
        ));
    }

    let total_secs = value * multiplier_secs;
    let whole = total_secs.floor() as u64;
    let nanos = ((total_secs - total_secs.floor()) * 1_000_000_000.0) as u32;

    Ok(Duration::new(whole, nanos))
}

const HELP_TEXT: &str = "Usage: sleep NUMBER[SUFFIX]...
Pause for NUMBER seconds. SUFFIX may be:
  s  seconds (default)
  m  minutes
  h  hours
  d  days

Fractional seconds are supported (e.g., sleep 0.5).
Multiple arguments sum their durations (e.g., sleep 1m 30s).

Examples:
  sleep 5        sleep 5 seconds
  sleep 0.1      sleep 100 milliseconds
  sleep 1m       sleep 1 minute
  sleep 1h 30m   sleep 1.5 hours
";

#[cfg(test)]
mod tests {
    use super::*;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn test_sleep_zero() {
        let mut rt = make_runtime();
        let result = builtin_sleep(&["0".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_sleep_fractional() {
        let mut rt = make_runtime();
        let result = builtin_sleep(&["0.01".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_sleep_suffix_s() {
        let mut rt = make_runtime();
        let result = builtin_sleep(&["0s".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_sleep_suffix_m() {
        let d = parse_duration("2m").unwrap();
        assert_eq!(d, Duration::from_secs(120));
    }

    #[test]
    fn test_sleep_suffix_h() {
        let d = parse_duration("1h").unwrap();
        assert_eq!(d, Duration::from_secs(3600));
    }

    #[test]
    fn test_sleep_suffix_d() {
        let d = parse_duration("1d").unwrap();
        assert_eq!(d, Duration::from_secs(86400));
    }

    #[test]
    fn test_sleep_fractional_minutes() {
        let d = parse_duration("0.5m").unwrap();
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn test_sleep_missing_operand() {
        let mut rt = make_runtime();
        let result = builtin_sleep(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_sleep_invalid_value() {
        let mut rt = make_runtime();
        let result = builtin_sleep(&["abc".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid time interval"));
    }

    #[test]
    fn test_sleep_negative_value() {
        let result = parse_duration("-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_sleep_multiple_args() {
        // 1s + 2s = 3s total — just check the parse, don't actually sleep
        let d1 = parse_duration("1").unwrap();
        let d2 = parse_duration("2").unwrap();
        assert_eq!(d1 + d2, Duration::from_secs(3));
    }
}
