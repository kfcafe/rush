use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// All known shopt option names
const SHOPT_OPTIONS: &[&str] = &[
    "autocd",
    "cdspell",
    "checkwinsize",
    "dotglob",
    "extglob",
    "failglob",
    "globstar",
    "histappend",
    "inherit_errexit",
    "nocaseglob",
    "nocasematch",
    "nullglob",
];

/// Get the current value of a named shopt option from the runtime
fn get_shopt(runtime: &Runtime, name: &str) -> Option<bool> {
    match name {
        "autocd" => Some(runtime.shopt.autocd),
        "cdspell" => Some(runtime.shopt.cdspell),
        "checkwinsize" => Some(runtime.shopt.checkwinsize),
        "dotglob" => Some(runtime.shopt.dotglob),
        "extglob" => Some(runtime.shopt.extglob),
        "failglob" => Some(runtime.shopt.failglob),
        "globstar" => Some(runtime.shopt.globstar),
        "histappend" => Some(runtime.shopt.histappend),
        "inherit_errexit" => Some(runtime.shopt.inherit_errexit),
        "nocaseglob" => Some(runtime.shopt.nocaseglob),
        "nocasematch" => Some(runtime.shopt.nocasematch),
        "nullglob" => Some(runtime.shopt.nullglob),
        _ => None,
    }
}

/// Set a named shopt option in the runtime
fn set_shopt(runtime: &mut Runtime, name: &str, value: bool) -> Result<()> {
    match name {
        "autocd" => runtime.shopt.autocd = value,
        "cdspell" => runtime.shopt.cdspell = value,
        "checkwinsize" => runtime.shopt.checkwinsize = value,
        "dotglob" => runtime.shopt.dotglob = value,
        "extglob" => runtime.shopt.extglob = value,
        "failglob" => runtime.shopt.failglob = value,
        "globstar" => runtime.shopt.globstar = value,
        "histappend" => runtime.shopt.histappend = value,
        "inherit_errexit" => runtime.shopt.inherit_errexit = value,
        "nocaseglob" => runtime.shopt.nocaseglob = value,
        "nocasematch" => runtime.shopt.nocasematch = value,
        "nullglob" => runtime.shopt.nullglob = value,
        _ => return Err(anyhow!("shopt: {}: invalid shell option name", name)),
    }
    Ok(())
}

pub fn builtin_shopt(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Parse flags: -s (set), -u (unset), -q (quiet query), -p (print in set/unset form)
    let mut enable = false;
    let mut disable = false;
    let mut quiet = false;
    let mut print_form = false;
    let mut option_names: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-s" => enable = true,
            "-u" => disable = true,
            "-q" => quiet = true,
            "-p" => print_form = true,
            arg if arg.starts_with('-') => {
                return Err(anyhow!("shopt: {}: invalid option", arg));
            }
            name => option_names.push(name.to_string()),
        }
        i += 1;
    }

    // Enabling and disabling at the same time is a user error
    if enable && disable {
        return Err(anyhow!("shopt: cannot use -s and -u together"));
    }

    // If no option names given, list all options
    if option_names.is_empty() {
        let mut output = String::new();
        for &name in SHOPT_OPTIONS {
            let val = get_shopt(runtime, name).unwrap_or(false);
            if print_form {
                let flag = if val { "-s" } else { "-u" };
                output.push_str(&format!("shopt {} {}\n", flag, name));
            } else {
                let status = if val { "on" } else { "off" };
                output.push_str(&format!("{}\t{}\n", name, status));
            }
        }
        return Ok(ExecutionResult::success(output));
    }

    // Process named options
    if enable || disable {
        // Set or unset mode
        for name in &option_names {
            set_shopt(runtime, name, enable)?;
        }
        return Ok(ExecutionResult::success(String::new()));
    }

    // Query mode: list status of named options, exit 1 if any is off
    let mut all_on = true;
    let mut output = String::new();
    for name in &option_names {
        let val = get_shopt(runtime, name)
            .ok_or_else(|| anyhow!("shopt: {}: invalid shell option name", name))?;
        if !val {
            all_on = false;
        }
        if !quiet {
            if print_form {
                let flag = if val { "-s" } else { "-u" };
                output.push_str(&format!("shopt {} {}\n", flag, name));
            } else {
                let status = if val { "on" } else { "off" };
                output.push_str(&format!("{}\t{}\n", name, status));
            }
        }
    }

    let exit_code = if all_on { 0 } else { 1 };
    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_shopt_set_and_query() {
        let mut rt = Runtime::new();

        // Enable globstar
        let res = builtin_shopt(&["-s".to_string(), "globstar".to_string()], &mut rt).unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(rt.shopt.globstar);

        // Query globstar — should be on (exit 0)
        let res = builtin_shopt(&["globstar".to_string()], &mut rt).unwrap();
        assert_eq!(res.exit_code, 0);

        // Disable globstar
        builtin_shopt(&["-u".to_string(), "globstar".to_string()], &mut rt).unwrap();
        assert!(!rt.shopt.globstar);

        // Query globstar — should be off (exit 1)
        let res = builtin_shopt(&["globstar".to_string()], &mut rt).unwrap();
        assert_eq!(res.exit_code, 1);
    }

    #[test]
    fn test_shopt_list_all() {
        let mut rt = Runtime::new();
        let res = builtin_shopt(&[], &mut rt).unwrap();
        assert_eq!(res.exit_code, 0);
        let out = res.stdout();
        assert!(out.contains("globstar"));
        assert!(out.contains("nullglob"));
        assert!(out.contains("dotglob"));
    }

    #[test]
    fn test_shopt_invalid_option() {
        let mut rt = Runtime::new();
        let res = builtin_shopt(&["-s".to_string(), "fakeoption".to_string()], &mut rt);
        assert!(res.is_err());
    }

    #[test]
    fn test_shopt_quiet() {
        let mut rt = Runtime::new();
        // -q should produce no output but correct exit code
        let res = builtin_shopt(&["-q".to_string(), "globstar".to_string()], &mut rt).unwrap();
        assert_eq!(res.exit_code, 1); // off by default
        assert!(res.stdout().is_empty());
    }
}
