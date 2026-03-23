use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed options for the touch command
#[derive(Debug, Default)]
struct TouchOptions {
    /// Don't create file if it doesn't exist (-c / --no-create)
    no_create: bool,
    /// Change only atime (-a)
    atime_only: bool,
    /// Change only mtime (-m)
    mtime_only: bool,
    /// Specific timestamp to set, parsed from -t STAMP
    /// Stored as (seconds_since_epoch, nanoseconds)
    timestamp: Option<(i64, i64)>,
    /// Files to touch
    files: Vec<String>,
}

impl TouchOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = TouchOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--no-create" {
                opts.no_create = true;
            } else if arg == "--help" {
                return Err(anyhow!("HELP"));
            } else if arg.starts_with("--") {
                return Err(anyhow!("touch: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                let mut chars = arg[1..].chars().peekable();
                while let Some(ch) = chars.next() {
                    match ch {
                        'c' => opts.no_create = true,
                        'a' => opts.atime_only = true,
                        'm' => opts.mtime_only = true,
                        't' => {
                            // -t STAMP — may be joined or next arg
                            let rest: String = chars.collect();
                            let stamp_str = if rest.is_empty() {
                                i += 1;
                                args.get(i)
                                    .ok_or_else(|| anyhow!("touch: option '-t' requires an argument"))?
                                    .clone()
                            } else {
                                rest.clone()
                            };
                            opts.timestamp = Some(parse_timestamp(&stamp_str)?);
                            break; // consumed the rest of this flag group
                        }
                        _ => return Err(anyhow!("touch: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }

            i += 1;
        }

        if opts.files.is_empty() {
            return Err(anyhow!(
                "touch: missing file operand\nTry 'touch --help' for more information."
            ));
        }

        Ok(opts)
    }
}

/// Parse a touch -t timestamp string: [[CC]YY]MMDDhhmm[.ss]
/// Returns (seconds_since_epoch, nanoseconds).
fn parse_timestamp(s: &str) -> Result<(i64, i64)> {
    // Split off optional .ss suffix
    let (main, ss) = if let Some(dot_pos) = s.rfind('.') {
        let (m, sec_part) = s.split_at(dot_pos);
        let sec_str = &sec_part[1..]; // drop the '.'
        let ss: u32 = sec_str
            .parse()
            .map_err(|_| anyhow!("touch: invalid timestamp '{}'", s))?;
        if ss > 59 {
            return Err(anyhow!("touch: invalid timestamp '{}': seconds out of range", s));
        }
        (m, ss)
    } else {
        (s, 0)
    };

    // main is [[CC]YY]MMDDhhmm — 8, 10, or 12 digits
    if !main.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("touch: invalid timestamp '{}'", s));
    }

    let len = main.len();
    if len != 8 && len != 10 && len != 12 {
        return Err(anyhow!(
            "touch: invalid timestamp '{}': expected [[CC]YY]MMDDhhmm[.ss]",
            s
        ));
    }

    let (year, rest) = match len {
        12 => {
            // CCYYMMDDhhmm
            let cc: i32 = main[..2].parse().unwrap();
            let yy: i32 = main[2..4].parse().unwrap();
            (cc * 100 + yy, &main[4..])
        }
        10 => {
            // YYMMDDhhmm
            let yy: i32 = main[..2].parse().unwrap();
            let year = if yy >= 69 { 1900 + yy } else { 2000 + yy };
            (year, &main[2..])
        }
        8 => {
            // MMDDhhmm — current year
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Rough current year from unix time
            let approx_year = 1970 + (now / 31_557_600) as i32;
            (approx_year, main)
        }
        _ => unreachable!(),
    };

    let month: u32 = rest[..2].parse().map_err(|_| anyhow!("touch: invalid timestamp '{}'", s))?;
    let day: u32 = rest[2..4].parse().map_err(|_| anyhow!("touch: invalid timestamp '{}'", s))?;
    let hour: u32 = rest[4..6].parse().map_err(|_| anyhow!("touch: invalid timestamp '{}'", s))?;
    let minute: u32 = rest[6..8].parse().map_err(|_| anyhow!("touch: invalid timestamp '{}'", s))?;

    if month < 1 || month > 12 {
        return Err(anyhow!("touch: invalid timestamp '{}': month out of range", s));
    }
    if day < 1 || day > 31 {
        return Err(anyhow!("touch: invalid timestamp '{}': day out of range", s));
    }
    if hour > 23 {
        return Err(anyhow!("touch: invalid timestamp '{}': hour out of range", s));
    }
    if minute > 59 {
        return Err(anyhow!("touch: invalid timestamp '{}': minute out of range", s));
    }

    // Convert to unix timestamp using a simple formula (ignores leap seconds)
    let epoch_secs = days_since_epoch(year, month, day) * 86400
        + hour as i64 * 3600
        + minute as i64 * 60
        + ss as i64;

    Ok((epoch_secs, 0))
}

/// Compute days since Unix epoch (1970-01-01) for a given date.
fn days_since_epoch(year: i32, month: u32, day: u32) -> i64 {
    // Days in each month (non-leap)
    let days_before_month: [u32; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    let is_leap = |y: i32| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;

    let leap_days_before = |y: i32| -> i64 {
        let y = y as i64;
        (y - 1) / 4 - (y - 1) / 100 + (y - 1) / 400
    };

    let epoch_leap = leap_days_before(1970);
    let year_leap = leap_days_before(year);

    let year_days = (year as i64 - 1970) * 365 + (year_leap - epoch_leap);
    let month_days = days_before_month[month as usize] as i64
        + if month > 2 && is_leap(year) { 1 } else { 0 };

    year_days + month_days + (day as i64 - 1)
}

/// Resolve path against CWD, expanding leading `~`.
fn resolve_path(path_str: &str, cwd: &Path) -> PathBuf {
    let path = if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(path_str.trim_start_matches("~/"))
        } else {
            PathBuf::from(path_str)
        }
    } else {
        PathBuf::from(path_str)
    };

    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

/// Use libc::utimensat to set atime and/or mtime on a file.
///
/// Pass `None` for a timestamp to leave it unchanged (UTIME_OMIT).
/// Pass `Some((0, libc::UTIME_NOW))` to set to current time via kernel.
fn set_times(path: &Path, atime: Option<(i64, i64)>, mtime: Option<(i64, i64)>) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let to_timespec = |opt: Option<(i64, i64)>| -> libc::timespec {
        match opt {
            Some((sec, nsec)) => libc::timespec {
                tv_sec: sec as libc::time_t,
                tv_nsec: nsec as libc::c_long,
            },
            None => libc::timespec {
                tv_sec: 0,
                tv_nsec: libc::UTIME_OMIT,
            },
        }
    };

    let times = [to_timespec(atime), to_timespec(mtime)];

    // Build a nul-terminated path
    let mut path_bytes = path.as_os_str().as_bytes().to_vec();
    path_bytes.push(0);

    // SAFETY: path_bytes is nul-terminated; times is a valid 2-element array
    let ret = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            path_bytes.as_ptr() as *const libc::c_char,
            times.as_ptr(),
            0,
        )
    };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        return Err(anyhow!("cannot touch '{}': {}", path.display(), err));
    }

    Ok(())
}

pub fn builtin_touch(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match TouchOptions::parse(args) {
        Ok(o) => o,
        Err(e) if e.to_string() == "HELP" => {
            return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
        }
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut stderr_output = String::new();
    let mut exit_code = 0;

    for file_arg in &opts.files {
        let path = resolve_path(file_arg, runtime.get_cwd());

        if !path.exists() {
            if opts.no_create {
                // -c: silently skip non-existent files
                continue;
            }
            // Create the file
            match fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&path)
            {
                Ok(_) => {
                    let description = format!("touch {}", path.display());
                    runtime
                        .undo_manager_mut()
                        .track_create(path.clone(), description);
                }
                Err(e) => {
                    stderr_output.push_str(&format!(
                        "touch: cannot touch '{}': {}\n",
                        file_arg, e
                    ));
                    exit_code = 1;
                    continue;
                }
            }
        }

        // Update timestamps.
        // If a specific -t timestamp was given, use it for whichever times apply.
        // If neither -a nor -m is set, update both.
        let update_atime = !opts.mtime_only; // true unless -m only
        let update_mtime = !opts.atime_only; // true unless -a only

        let now_ts: Option<(i64, i64)> = if opts.timestamp.is_some() {
            opts.timestamp
        } else {
            // Use UTIME_NOW so the kernel fills in the current time.
            // The value differs by platform; use the libc constant for correctness.
            Some((0, libc::UTIME_NOW as i64))
        };

        let atime = if update_atime { now_ts } else { None };
        let mtime = if update_mtime { now_ts } else { None };

        if let Err(e) = set_times(&path, atime, mtime) {
            stderr_output.push_str(&format!("{}\n", e));
            exit_code = 1;
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: touch [OPTION]... FILE...
Update the access and modification times of each FILE to the current time.
A FILE argument that does not exist is created empty, unless -c is supplied.

Options:
  -a            change only the access time
  -c, --no-create  do not create any files
  -m            change only the modification time
  -t STAMP      use [[CC]YY]MMDDhhmm[.ss] instead of current time
  --help        display this help and exit

Examples:
  touch file.txt           Create file.txt or update its timestamps
  touch -c file.txt        Update timestamps (no-op if file missing)
  touch -t 202506011200 f  Set time to 2025-06-01 12:00
  touch -a file.txt        Update access time only
  touch -m file.txt        Update modification time only
";

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn make_runtime(dir: &TempDir) -> Runtime {
        let mut rt = Runtime::new();
        rt.set_cwd(dir.path().to_path_buf());
        rt
    }

    // ---- create new file ----

    #[test]
    fn test_builtin_touch_creates_new_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(&["newfile.txt".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(tmp.path().join("newfile.txt").exists());
    }

    #[test]
    fn test_builtin_touch_creates_multiple_files() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(
            &["a.txt".to_string(), "b.txt".to_string(), "c.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(tmp.path().join("a.txt").exists());
        assert!(tmp.path().join("b.txt").exists());
        assert!(tmp.path().join("c.txt").exists());
    }

    // ---- update existing file ----

    #[test]
    fn test_builtin_touch_updates_existing_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let file = tmp.path().join("existing.txt");
        fs::write(&file, "hello").unwrap();

        // Set mtime to 10 seconds in the past via set_times
        let past = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 10;
        set_times(&file, Some((past, 0)), Some((past, 0))).unwrap();

        std::thread::sleep(Duration::from_millis(50));

        let result = builtin_touch(&["existing.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mtime_after = fs::metadata(&file).unwrap().modified().unwrap();
        let after_secs = mtime_after
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(
            after_secs > past,
            "mtime should be updated beyond past: got {after_secs}, was {past}"
        );
        // File content should be unchanged
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello");
    }

    // ---- -c flag ----

    #[test]
    fn test_builtin_touch_no_create_flag() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(
            &["-c".to_string(), "ghost.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!tmp.path().join("ghost.txt").exists());
    }

    #[test]
    fn test_builtin_touch_no_create_existing_file_succeeds() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let file = tmp.path().join("present.txt");
        fs::write(&file, "data").unwrap();

        let result = builtin_touch(
            &["-c".to_string(), "present.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(file.exists());
    }

    // ---- -a / -m flags ----

    #[test]
    fn test_builtin_touch_atime_only() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let file = tmp.path().join("atime.txt");
        fs::write(&file, "x").unwrap();

        let result = builtin_touch(&["-a".to_string(), "atime.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
    }

    #[test]
    fn test_builtin_touch_mtime_only() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let file = tmp.path().join("mtime.txt");
        fs::write(&file, "x").unwrap();

        let result = builtin_touch(&["-m".to_string(), "mtime.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
    }

    // ---- undo tracking ----

    #[test]
    fn test_builtin_touch_undo_tracked_for_new_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(&["tracked.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);

        let ops = rt.undo_manager_mut().list_operations(10);
        assert!(!ops.is_empty(), "undo operation should be recorded");
        assert!(
            ops[0].description.contains("touch"),
            "description should mention touch"
        );
    }

    #[test]
    fn test_builtin_touch_undo_not_tracked_for_existing_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let file = tmp.path().join("existing.txt");
        fs::write(&file, "data").unwrap();

        let result = builtin_touch(&["existing.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);

        // No undo entry for simple timestamp update
        let ops = rt.undo_manager_mut().list_operations(10);
        assert!(ops.is_empty(), "no undo entry for timestamp-only update");
    }

    // ---- error cases ----

    #[test]
    fn test_builtin_touch_missing_operand() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing file operand"));
    }

    #[test]
    fn test_builtin_touch_invalid_option() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_touch(&["-z".to_string(), "f.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    // ---- -t timestamp ----

    #[test]
    fn test_builtin_touch_timestamp_flag() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        // Create file first
        let file = tmp.path().join("ts.txt");
        fs::write(&file, "").unwrap();

        let result = builtin_touch(
            &["-t".to_string(), "202506011200".to_string(), "ts.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        // Verify the mtime is approximately 2025-06-01 12:00 UTC
        let meta = fs::metadata(&file).unwrap();
        let mtime = meta.modified().unwrap();
        let secs = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 2025-06-01 12:00:00 UTC = 1748779200
        assert!(
            (secs as i64 - 1748779200).abs() < 2,
            "mtime should be ~2025-06-01 12:00 UTC, got secs={secs}"
        );
    }

    // ---- timestamp parser unit tests ----

    #[test]
    fn test_parse_timestamp_12digit() {
        // 202506011200 → 2025-06-01 12:00:00 UTC
        let (sec, nsec) = parse_timestamp("202506011200").unwrap();
        assert_eq!(nsec, 0);
        // Allow ±1 second for rounding
        assert!((sec - 1748779200).abs() < 2, "got sec={sec}");
    }

    #[test]
    fn test_parse_timestamp_with_seconds() {
        // 202506011200.30 → +30 seconds
        let (sec, _) = parse_timestamp("202506011200.30").unwrap();
        assert!((sec - 1748779230).abs() < 2, "got sec={sec}");
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp("not-a-date").is_err());
        assert!(parse_timestamp("12345").is_err()); // wrong length
        assert!(parse_timestamp("202506011200.99").is_err()); // seconds > 59 is invalid
    }

    #[test]
    fn test_days_since_epoch_known_dates() {
        // 1970-01-01 = day 0
        assert_eq!(days_since_epoch(1970, 1, 1), 0);
        // 1970-01-02 = day 1
        assert_eq!(days_since_epoch(1970, 1, 2), 1);
        // 2000-01-01 = 10957 days
        assert_eq!(days_since_epoch(2000, 1, 1), 10957);
    }
}
