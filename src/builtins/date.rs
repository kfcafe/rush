use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, TimeZone, Utc};

pub fn builtin_date(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let mut use_utc = false;
    let mut format: Option<String> = None;
    let mut date_string: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-u" | "--utc" | "--universal" => {
                use_utc = true;
            }
            "-d" | "--date" => {
                i += 1;
                date_string = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow!("date: option '-d' requires an argument"))?
                        .clone(),
                );
            }
            _ if arg.starts_with("-d") => {
                date_string = Some(arg[2..].to_string());
            }
            _ if arg.starts_with('+') => {
                format = Some(arg[1..].to_string());
            }
            _ => {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: format!("date: invalid option or argument '{}'\n", arg),
                    exit_code: 1,
                    error: None,
                });
            }
        }
        i += 1;
    }

    let output = if let Some(ds) = &date_string {
        format_date_string(ds, use_utc, format.as_deref())?
    } else {
        format_now(use_utc, format.as_deref())
    };

    Ok(ExecutionResult::success(format!("{}\n", output)))
}

/// Format the current date/time.
fn format_now(use_utc: bool, format: Option<&str>) -> String {
    let fmt = format.unwrap_or(DEFAULT_FORMAT);
    if use_utc {
        Utc::now().format(fmt).to_string()
    } else {
        Local::now().format(fmt).to_string()
    }
}

/// Parse a date string and format it.
///
/// Supports:
///   - Unix timestamps prefixed with `@` (e.g., `@1748779200`)
///   - ISO 8601 / RFC 3339 strings (e.g., `2025-06-01T12:00:00Z`)
///   - Date-only strings (e.g., `2025-06-01`)
fn format_date_string(s: &str, use_utc: bool, format: Option<&str>) -> Result<String> {
    let fmt = format.unwrap_or(DEFAULT_FORMAT);

    // @UNIX_TIMESTAMP
    if let Some(ts_str) = s.strip_prefix('@') {
        let secs: i64 = ts_str
            .parse()
            .map_err(|_| anyhow!("date: invalid date '@{}'", ts_str))?;
        let dt_utc = DateTime::from_timestamp(secs, 0)
            .ok_or_else(|| anyhow!("date: timestamp out of range: {}", secs))?;
        return Ok(if use_utc {
            dt_utc.format(fmt).to_string()
        } else {
            DateTime::<Local>::from(dt_utc).format(fmt).to_string()
        });
    }

    // Try RFC 3339 / ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(if use_utc {
            dt.with_timezone(&Utc).format(fmt).to_string()
        } else {
            dt.with_timezone(&Local).format(fmt).to_string()
        });
    }

    // Try date-only YYYY-MM-DD
    if let Ok(nd) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(if use_utc {
            Utc.from_utc_datetime(&ndt).format(fmt).to_string()
        } else {
            Local
                .from_local_datetime(&ndt)
                .single()
                .ok_or_else(|| anyhow!("date: ambiguous local time for '{}'", s))?
                .format(fmt)
                .to_string()
        });
    }

    // Try common formats
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%d %b %Y",
        "%b %d %Y",
    ];
    for f in &formats {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return Ok(if use_utc {
                Utc.from_utc_datetime(&ndt).format(fmt).to_string()
            } else {
                Local
                    .from_local_datetime(&ndt)
                    .single()
                    .ok_or_else(|| anyhow!("date: ambiguous local time for '{}'", s))?
                    .format(fmt)
                    .to_string()
            });
        }
    }

    Err(anyhow!("date: invalid date '{}'\n", s))
}

/// Default output format — matches GNU date's default on Linux.
const DEFAULT_FORMAT: &str = "%a %b %e %H:%M:%S %Z %Y";

const HELP_TEXT: &str = "Usage: date [OPTION]... [+FORMAT]
Display the current date and time, or a specified date.

Options:
  -d STRING, --date=STRING  display time described by STRING, not 'now'
  -u, --utc                 print or set Coordinated Universal Time (UTC)
  --help                    display this help and exit

FORMAT directives (strftime-style):
  %Y  four-digit year        %m  month (01–12)    %d  day (01–31)
  %H  hour (00–23)           %M  minute (00–59)   %S  second (00–60)
  %F  equivalent to %Y-%m-%d
  %T  equivalent to %H:%M:%S
  %s  seconds since Unix epoch
  %Z  timezone abbreviation

Examples:
  date                    current date and time
  date +%Y-%m-%d          2025-06-01
  date +%s                Unix timestamp
  date -u +%T             UTC time
  date -d @1748779200     convert Unix timestamp
  date -d 2025-06-01      display a specific date
";

#[cfg(test)]
mod tests {
    use super::*;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn test_date_no_args_succeeds() {
        let mut rt = make_runtime();
        let result = builtin_date(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.output.as_text().trim().is_empty());
    }

    #[test]
    fn test_date_format_year() {
        let mut rt = make_runtime();
        let result = builtin_date(&["+%Y".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        let out = result.output.as_text();
        let year: i32 = out.trim().parse().unwrap();
        assert!(year >= 2025, "year={year}");
    }

    #[test]
    fn test_date_utc_flag() {
        let mut rt = make_runtime();
        let result = builtin_date(&["-u".to_string(), "+%Y".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        let out = result.output.as_text();
        let year: i32 = out.trim().parse().unwrap();
        assert!(year >= 2025, "year={year}");
    }

    #[test]
    fn test_date_unix_timestamp() {
        let mut rt = make_runtime();
        let result = builtin_date(
            &["-d".to_string(), "@0".to_string(), "+%Y-%m-%d".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        // @0 is 1970-01-01 in UTC; in local time it might still be 1970-01-01 or 1969-12-31
        let out = result.output.as_text();
        assert!(out.contains("1970") || out.contains("1969"), "got: {out}");
    }

    #[test]
    fn test_date_unix_timestamp_utc() {
        let mut rt = make_runtime();
        let result = builtin_date(
            &[
                "-u".to_string(),
                "-d".to_string(),
                "@0".to_string(),
                "+%Y-%m-%d".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.as_text().trim(), "1970-01-01");
    }

    #[test]
    fn test_date_iso_string() {
        let mut rt = make_runtime();
        let result = builtin_date(
            &[
                "-u".to_string(),
                "-d".to_string(),
                "2025-06-01T12:00:00Z".to_string(),
                "+%Y-%m-%d".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.as_text().trim(), "2025-06-01");
    }

    #[test]
    fn test_date_invalid_arg() {
        let mut rt = make_runtime();
        let result = builtin_date(&["--badopt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }
}
