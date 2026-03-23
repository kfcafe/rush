use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Resource limit descriptor: maps a flag character to a libc resource type and display label.
struct Resource {
    flag: char,
    resource: libc::c_int,
    label: &'static str,
    unit: &'static str,
}

const RESOURCES: &[Resource] = &[
    Resource { flag: 'c', resource: libc::RLIMIT_CORE,   label: "core file size",          unit: "blocks" },
    Resource { flag: 'n', resource: libc::RLIMIT_NOFILE, label: "open files",               unit: ""       },
    Resource { flag: 's', resource: libc::RLIMIT_STACK,  label: "stack size",               unit: "kbytes" },
    Resource { flag: 'u', resource: libc::RLIMIT_NPROC,  label: "max user processes",       unit: ""       },
    Resource { flag: 'v', resource: libc::RLIMIT_AS,     label: "virtual memory",           unit: "kbytes" },
];

pub fn builtin_ulimit(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Default: no args → show/get -n (open files), soft limit
    if args.is_empty() {
        let rlim = get_rlimit(libc::RLIMIT_NOFILE)?;
        return Ok(ExecutionResult::success(format_limit(rlim.rlim_cur)));
    }

    let mut show_all = false;
    let mut hard = false;
    let mut resource_flag: Option<char> = None;
    let mut set_value: Option<&str> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'a' => show_all = true,
                    'H' => hard = true,
                    'S' => hard = false,
                    'c' | 'n' | 's' | 'u' | 'v' => resource_flag = Some(ch),
                    _ => return Err(anyhow!("ulimit: invalid option: -{}", ch)),
                }
            }
        } else {
            set_value = Some(arg.as_str());
        }
        i += 1;
    }

    if show_all {
        return show_all_limits(hard);
    }

    let flag = resource_flag.unwrap_or('n');
    let res = find_resource(flag)
        .ok_or_else(|| anyhow!("ulimit: no resource for flag -{}", flag))?;

    if let Some(val_str) = set_value {
        // Set the limit
        let new_val: libc::rlim_t = if val_str == "unlimited" {
            libc::RLIM_INFINITY
        } else {
            val_str.parse::<libc::rlim_t>()
                .map_err(|_| anyhow!("ulimit: invalid limit value: {}", val_str))?
        };

        let mut rlim = get_rlimit(res.resource)?;
        if hard {
            rlim.rlim_max = new_val;
            // When setting hard limit, soft must not exceed it
            if rlim.rlim_cur != libc::RLIM_INFINITY && rlim.rlim_cur > new_val {
                rlim.rlim_cur = new_val;
            }
        } else {
            rlim.rlim_cur = new_val;
        }
        set_rlimit(res.resource, &rlim)?;
        Ok(ExecutionResult::success(String::new()))
    } else {
        // Get the limit
        let rlim = get_rlimit(res.resource)?;
        let val = if hard { rlim.rlim_max } else { rlim.rlim_cur };
        Ok(ExecutionResult::success(format_limit(val)))
    }
}

fn show_all_limits(hard: bool) -> Result<ExecutionResult> {
    let mut output = String::new();
    for res in RESOURCES {
        let rlim = get_rlimit(res.resource)?;
        let val = if hard { rlim.rlim_max } else { rlim.rlim_cur };
        let unit_suffix = if res.unit.is_empty() {
            String::new()
        } else {
            format!(" ({})", res.unit)
        };
        output.push_str(&format!(
            "{:30} (-{})   {}\n",
            format!("{}{}", res.label, unit_suffix),
            res.flag,
            format_limit_inline(val),
        ));
    }
    Ok(ExecutionResult::success(output))
}

fn find_resource(flag: char) -> Option<&'static Resource> {
    RESOURCES.iter().find(|r| r.flag == flag)
}

fn get_rlimit(resource: libc::c_int) -> Result<libc::rlimit> {
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let ret = unsafe { libc::getrlimit(resource, &mut rlim) };
    if ret != 0 {
        return Err(anyhow!("ulimit: getrlimit failed: {}", std::io::Error::last_os_error()));
    }
    Ok(rlim)
}

fn set_rlimit(resource: libc::c_int, rlim: &libc::rlimit) -> Result<()> {
    let ret = unsafe { libc::setrlimit(resource, rlim) };
    if ret != 0 {
        return Err(anyhow!("ulimit: setrlimit failed: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// Format a limit value with a trailing newline (for single-value output).
fn format_limit(val: libc::rlim_t) -> String {
    format!("{}\n", format_limit_inline(val))
}

/// Format a limit value without a trailing newline (for -a table rows).
fn format_limit_inline(val: libc::rlim_t) -> String {
    if val == libc::RLIM_INFINITY {
        "unlimited".to_string()
    } else {
        val.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn run(args: &[&str]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut runtime = Runtime::new();
        builtin_ulimit(&args, &mut runtime).expect("ulimit failed")
    }

    fn run_err(args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut runtime = Runtime::new();
        builtin_ulimit(&args, &mut runtime)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn test_builtin_ulimit_get_nofile() {
        // Default (no args) → open file limit
        let result = run(&[]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let trimmed = out.trim();
        // Should be a number or "unlimited"
        assert!(trimmed == "unlimited" || trimmed.parse::<u64>().is_ok());
    }

    #[test]
    fn test_builtin_ulimit_get_n_explicit() {
        let result = run(&["-n"]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout().trim().to_string();
        assert!(out == "unlimited" || out.parse::<u64>().is_ok());
    }

    #[test]
    fn test_builtin_ulimit_show_all() {
        let result = run(&["-a"]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        assert!(out.contains("open files"));
        assert!(out.contains("stack size"));
        assert!(out.contains("max user processes"));
        assert!(out.contains("core file size"));
    }

    #[test]
    fn test_builtin_ulimit_hard_flag() {
        let result = run(&["-H", "-n"]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout().trim().to_string();
        assert!(out == "unlimited" || out.parse::<u64>().is_ok());
    }

    #[test]
    fn test_builtin_ulimit_invalid_option() {
        let err = run_err(&["-z"]);
        assert!(err.contains("invalid option"));
    }

    #[test]
    fn test_builtin_ulimit_invalid_value() {
        let err = run_err(&["-n", "notanumber"]);
        assert!(err.contains("invalid limit value"));
    }

    #[test]
    fn test_builtin_ulimit_get_stack() {
        let result = run(&["-s"]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout().trim().to_string();
        assert!(out == "unlimited" || out.parse::<u64>().is_ok());
    }

    #[test]
    fn test_builtin_ulimit_get_nproc() {
        let result = run(&["-u"]);
        assert_eq!(result.exit_code, 0);
        let out = result.stdout().trim().to_string();
        assert!(out == "unlimited" || out.parse::<u64>().is_ok());
    }
}
