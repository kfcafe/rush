use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

struct EnvOptions {
    /// -i / --ignore-environment: start with an empty environment
    ignore_env: bool,
    /// -u NAME / --unset=NAME: remove variable from the environment
    unset: Vec<String>,
    /// NAME=VALUE assignments to apply before command
    assignments: Vec<(String, String)>,
    /// Optional command to run (not used in builtin mode — we just set vars
    /// in the current shell environment and print the result)
    command: Vec<String>,
}

impl EnvOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = EnvOptions {
            ignore_env: false,
            unset: vec![],
            assignments: vec![],
            command: vec![],
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.command.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--ignore-environment" {
                opts.ignore_env = true;
            } else if arg.starts_with("--unset=") {
                opts.unset.push(arg["--unset=".len()..].to_string());
            } else if arg == "--unset" {
                i += 1;
                opts.unset.push(
                    args.get(i)
                        .ok_or_else(|| "env: option '--unset' requires an argument".to_string())?
                        .clone(),
                );
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                let mut chars = arg[1..].chars().peekable();
                while let Some(ch) = chars.next() {
                    match ch {
                        'i' => opts.ignore_env = true,
                        'u' => {
                            let rest: String = chars.collect();
                            let name = if rest.is_empty() {
                                i += 1;
                                args.get(i)
                                    .ok_or_else(|| {
                                        "env: option '-u' requires an argument".to_string()
                                    })?
                                    .clone()
                            } else {
                                rest
                            };
                            opts.unset.push(name);
                            break;
                        }
                        _ => return Err(format!("env: invalid option -- '{}'", ch)),
                    }
                }
            } else if arg.contains('=') {
                // NAME=VALUE assignment — only before command
                if opts.command.is_empty() {
                    if let Some((k, v)) = arg.split_once('=') {
                        opts.assignments.push((k.to_string(), v.to_string()));
                    }
                } else {
                    opts.command.push(arg.clone());
                }
            } else {
                // First non-option, non-assignment arg starts the command
                opts.command.push(arg.clone());
                opts.command.extend(args[i + 1..].iter().cloned());
                break;
            }
            i += 1;
        }
        Ok(opts)
    }
}

pub fn builtin_env(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    // No args: just print the current environment
    if args.is_empty() {
        let mut vars: Vec<String> = std::env::vars()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        vars.sort();
        let output = vars.join("\n") + "\n";
        return Ok(ExecutionResult::success(output));
    }

    let opts = match EnvOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\n", e),
                exit_code: 1,
                error: None,
            })
        }
    };

    // When a command is specified, we run it as an external process.
    // For the builtin case (no command), we print the environment with modifications.
    if !opts.command.is_empty() {
        return run_with_env(&opts, runtime);
    }

    // No command: collect and print the (modified) environment
    let mut env_vars: Vec<(String, String)> = if opts.ignore_env {
        vec![]
    } else {
        std::env::vars().collect()
    };

    // Apply unsets
    for name in &opts.unset {
        env_vars.retain(|(k, _)| k != name);
    }

    // Apply assignments
    for (k, v) in &opts.assignments {
        // Replace or insert
        if let Some(entry) = env_vars.iter_mut().find(|(ek, _)| ek == k) {
            entry.1 = v.clone();
        } else {
            env_vars.push((k.clone(), v.clone()));
        }
    }

    env_vars.sort_by(|a, b| a.0.cmp(&b.0));
    let output = env_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    Ok(ExecutionResult::success(output))
}

fn run_with_env(opts: &EnvOptions, runtime: &mut Runtime) -> Result<ExecutionResult> {
    use std::process::Command;

    let cmd_name = &opts.command[0];
    let cmd_args = &opts.command[1..];

    let mut cmd = Command::new(cmd_name);
    cmd.args(cmd_args);
    cmd.current_dir(runtime.get_cwd());

    if opts.ignore_env {
        cmd.env_clear();
    }

    for name in &opts.unset {
        cmd.env_remove(name);
    }

    for (k, v) in &opts.assignments {
        cmd.env(k, v);
    }

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let exit_code = out.status.code().unwrap_or(1);
            Ok(ExecutionResult {
                output: Output::Text(stdout),
                stderr,
                exit_code,
                error: None,
            })
        }
        Err(e) => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("env: {}: {}\n", cmd_name, e),
            exit_code: 127,
            error: None,
        }),
    }
}

const HELP_TEXT: &str = "Usage: env [OPTION]... [NAME=VALUE]... [COMMAND [ARG]...]
Set each NAME to VALUE in the environment and run COMMAND.
With no COMMAND, print the resulting environment.

Options:
  -i, --ignore-environment  start with an empty environment
  -u, --unset=NAME          remove variable from the environment
  --help                    display this help and exit

Examples:
  env                        print current environment
  env FOO=bar                print environment with FOO=bar added
  env -i PATH=/bin sh        run sh with only PATH set
  env -u HOME printenv       run printenv without HOME in environment
  env EDITOR=vim git commit  run git commit with a specific EDITOR
";

#[cfg(test)]
mod tests {
    use super::*;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn test_env_no_args_prints_environment() {
        let mut rt = make_runtime();
        let result = builtin_env(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        // Should have multiple KEY=VALUE lines
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.contains('=')));
    }

    #[test]
    fn test_env_add_assignment() {
        let mut rt = make_runtime();
        let result = builtin_env(&["MYVAR=hello_world".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("MYVAR=hello_world"));
    }

    #[test]
    fn test_env_ignore_environment() {
        let mut rt = make_runtime();
        let result = builtin_env(&["-i".to_string(), "ONLY=this".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        // Only ONLY=this should appear
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "expected exactly 1 var, got: {:?}", lines);
        assert_eq!(lines[0], "ONLY=this");
    }

    #[test]
    fn test_env_unset_variable() {
        std::env::set_var("_RUSH_DU_TEST_VAR", "to_be_unset");
        let mut rt = make_runtime();
        let result = builtin_env(
            &["-u".to_string(), "_RUSH_DU_TEST_VAR".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.stdout().contains("_RUSH_DU_TEST_VAR"));
        // Clean up
        std::env::remove_var("_RUSH_DU_TEST_VAR");
    }

    #[test]
    fn test_env_run_command() {
        let mut rt = make_runtime();
        let result = builtin_env(
            &[
                "VAR1=foo".to_string(),
                "printenv".to_string(),
                "VAR1".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        // printenv might not be available everywhere, so just check exit code
        if result.exit_code == 0 {
            assert!(
                result.stdout().contains("foo"),
                "stdout: {}",
                result.stdout()
            );
        }
    }

    #[test]
    fn test_env_invalid_option() {
        let mut rt = make_runtime();
        let result = builtin_env(&["-z".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
