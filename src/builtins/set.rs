use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

pub fn builtin_set(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // No args: show current options
    if args.is_empty() {
        return show_options(runtime);
    }

    // Find if there's a -- argument (signals end of options, start of positional params)
    let double_dash_pos = args.iter().position(|a| a == "--");

    // Parse options (everything before --)
    let option_args = if let Some(pos) = double_dash_pos {
        &args[..pos]
    } else {
        args
    };

    let mut i = 0;
    while i < option_args.len() {
        let arg = &option_args[i];
        if let Some(option_chars) = arg.strip_prefix('-') {
            // Setting options: -e, -u, -x, etc.

            // Handle -o followed by option name
            if option_chars == "o" {
                if i + 1 >= option_args.len() {
                    return Err(anyhow!("set: -o requires an argument"));
                }
                let option_name = &option_args[i + 1];
                runtime.set_option(option_name, true)?;
                i += 2;
                continue;
            }

            // Handle combined short options like -ex
            for ch in option_chars.chars() {
                let opt = ch.to_string();
                runtime.set_option(&opt, true)?;
            }
        } else if let Some(option_chars) = arg.strip_prefix('+') {
            // Unsetting options: +e, +u, +x, etc.

            // Handle +o followed by option name
            if option_chars == "o" {
                if i + 1 >= option_args.len() {
                    return Err(anyhow!("set: +o requires an argument"));
                }
                let option_name = &option_args[i + 1];
                runtime.set_option(option_name, false)?;
                i += 2;
                continue;
            }

            // Handle combined short options
            for ch in option_chars.chars() {
                let opt = ch.to_string();
                runtime.set_option(&opt, false)?;
            }
        } else if arg != "pipefail" && arg != "nounset" && arg != "errexit" && arg != "xtrace" {
            // Skip long option names that are arguments to -o/+o
            return Err(anyhow!("set: invalid argument: {}", arg));
        }
        i += 1;
    }

    // Handle positional parameters (everything after --)
    if let Some(pos) = double_dash_pos {
        let positional_args: Vec<String> = args[pos + 1..].to_vec();
        runtime.set_positional_params(positional_args);
    }

    Ok(ExecutionResult::success(String::new()))
}

fn show_options(runtime: &Runtime) -> Result<ExecutionResult> {
    let mut output = String::new();

    // Show all options in the format "set [-/+]option"
    if runtime.options.errexit {
        output.push_str("set -e\n");
    } else {
        output.push_str("set +e\n");
    }

    if runtime.options.nounset {
        output.push_str("set -u\n");
    } else {
        output.push_str("set +u\n");
    }

    if runtime.options.xtrace {
        output.push_str("set -x\n");
    } else {
        output.push_str("set +x\n");
    }

    if runtime.options.pipefail {
        output.push_str("set -o pipefail\n");
    } else {
        output.push_str("set +o pipefail\n");
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_set_single_option() {
        let mut runtime = Runtime::new();

        // Test setting errexit
        let result = builtin_set(&["-e".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(runtime.options.errexit);

        // Test unsetting errexit
        let result = builtin_set(&["+e".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(!runtime.options.errexit);
    }

    #[test]
    fn test_set_multiple_options() {
        let mut runtime = Runtime::new();

        // Test setting multiple options at once
        let result = builtin_set(&["-eux".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(runtime.options.errexit);
        assert!(runtime.options.nounset);
        assert!(runtime.options.xtrace);
    }

    #[test]
    fn test_set_o_option() {
        let mut runtime = Runtime::new();

        // Test setting pipefail
        let result = builtin_set(&["-o".to_string(), "pipefail".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(runtime.options.pipefail);

        // Test unsetting pipefail
        let result = builtin_set(&["+o".to_string(), "pipefail".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert!(!runtime.options.pipefail);
    }

    #[test]
    fn test_set_show_options() {
        let mut runtime = Runtime::new();
        runtime.options.errexit = true;
        runtime.options.xtrace = true;

        let result = builtin_set(&[], &mut runtime).unwrap();
        assert!(result.stdout().contains("set -e"));
        assert!(result.stdout().contains("set -x"));
        assert!(result.stdout().contains("set +u"));
        assert!(result.stdout().contains("set +o pipefail"));
    }

    #[test]
    fn test_set_invalid_option() {
        let mut runtime = Runtime::new();

        let result = builtin_set(&["-z".to_string()], &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_positional_params() {
        let mut runtime = Runtime::new();

        // Test setting positional parameters with --
        let result = builtin_set(
            &[
                "--".to_string(),
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ],
            &mut runtime,
        );
        assert!(result.is_ok());

        // Check positional parameters
        assert_eq!(runtime.get_positional_param(1), Some("one".to_string()));
        assert_eq!(runtime.get_positional_param(2), Some("two".to_string()));
        assert_eq!(runtime.get_positional_param(3), Some("three".to_string()));
        assert_eq!(runtime.param_count(), 3);
    }

    #[test]
    fn test_set_positional_params_empty() {
        let mut runtime = Runtime::new();

        // First set some params
        runtime.set_positional_params(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(runtime.param_count(), 2);

        // Then clear them with set --
        let result = builtin_set(&["--".to_string()], &mut runtime);
        assert!(result.is_ok());

        // Should have no positional parameters now
        assert_eq!(runtime.param_count(), 0);
    }

    #[test]
    fn test_set_options_and_positional_params() {
        let mut runtime = Runtime::new();

        // Test setting options AND positional parameters
        let result = builtin_set(
            &[
                "-e".to_string(),
                "--".to_string(),
                "arg1".to_string(),
                "arg2".to_string(),
            ],
            &mut runtime,
        );
        assert!(result.is_ok());

        // Check option was set
        assert!(runtime.options.errexit);

        // Check positional parameters
        assert_eq!(runtime.get_positional_param(1), Some("arg1".to_string()));
        assert_eq!(runtime.get_positional_param(2), Some("arg2".to_string()));
        assert_eq!(runtime.param_count(), 2);
    }
}
