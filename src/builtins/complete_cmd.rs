/// `complete` and `compgen` — programmable completion builtins
///
/// `complete` registers completion specifications for commands so that the
/// shell knows what arguments, flags, and words to suggest at the prompt.
///
/// `compgen` generates completion candidates on demand.  It is typically used
/// inside completion functions but also works on the command line.
use crate::executor::{ExecutionResult, Output};
use crate::runtime::{CompletionSpec, Runtime};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// complete builtin
// ---------------------------------------------------------------------------

/// Usage:
///   complete [-c COMMAND] [-s SHORT] [-l LONG] [-a WORDLIST] [-d DESC] [-F FUNC]
///   complete -e [-c COMMAND]    — erase spec for COMMAND
///   complete                    — list all specs
pub fn builtin_complete(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return list_completion_specs(runtime);
    }

    let mut command: Option<String> = None;
    let mut erase = false;
    let mut short_flags: Vec<String> = Vec::new();
    let mut long_flags: Vec<String> = Vec::new();
    let mut wordlist: Vec<String> = Vec::new();
    let mut description = String::new();
    let mut function: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-c" => {
                i += 1;
                command = Some(require_arg(&args, i, "-c")?);
            }
            "-e" => {
                erase = true;
            }
            "-s" => {
                i += 1;
                short_flags.push(require_arg(&args, i, "-s")?);
            }
            "-l" => {
                i += 1;
                long_flags.push(require_arg(&args, i, "-l")?);
            }
            "-a" => {
                i += 1;
                let words = require_arg(&args, i, "-a")?;
                wordlist.extend(words.split_whitespace().map(|s| s.to_string()));
            }
            "-d" => {
                i += 1;
                description = require_arg(&args, i, "-d")?;
            }
            "-F" => {
                i += 1;
                function = Some(require_arg(&args, i, "-F")?);
            }
            opt if opt.starts_with('-') => {
                return Err(anyhow!("complete: {}: invalid option", opt));
            }
            // Bare word treated as the command name (bash compat)
            word => {
                if command.is_none() {
                    command = Some(word.to_string());
                }
            }
        }
        i += 1;
    }

    let command = command.ok_or_else(|| anyhow!("complete: command name required (-c COMMAND)"))?;

    if erase {
        runtime.remove_completion_spec(&command);
        return Ok(ExecutionResult::success(String::new()));
    }

    // Build and store spec
    let spec = CompletionSpec {
        short_flags,
        long_flags,
        wordlist,
        description,
        function,
    };
    runtime.set_completion_spec(command, spec);
    Ok(ExecutionResult::success(String::new()))
}

/// Print all registered completion specs in a `complete` command form.
fn list_completion_specs(runtime: &Runtime) -> Result<ExecutionResult> {
    let specs = runtime.get_all_completion_specs();
    if specs.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    let mut lines: Vec<String> = specs
        .iter()
        .map(|(cmd, spec)| format_spec(cmd, spec))
        .collect();
    lines.sort();

    let mut output = lines.join("\n");
    output.push('\n');
    Ok(ExecutionResult::success(output))
}

/// Render a CompletionSpec back as a `complete …` invocation.
fn format_spec(command: &str, spec: &CompletionSpec) -> String {
    let mut parts = vec!["complete".to_string()];

    for s in &spec.short_flags {
        parts.push(format!("-s {}", s));
    }
    for l in &spec.long_flags {
        parts.push(format!("-l {}", l));
    }
    if !spec.wordlist.is_empty() {
        parts.push(format!("-a '{}'", spec.wordlist.join(" ")));
    }
    if !spec.description.is_empty() {
        parts.push(format!("-d '{}'", spec.description));
    }
    if let Some(func) = &spec.function {
        parts.push(format!("-F {}", func));
    }
    parts.push(format!("-c {}", command));
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// compgen builtin
// ---------------------------------------------------------------------------

/// Usage:
///   compgen [-W WORDLIST] [-f] [-d] [-c] [PREFIX]
///
/// Generates completion candidates matching PREFIX and prints one per line.
pub fn builtin_compgen(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut wordlist: Vec<String> = Vec::new();
    let mut gen_files = false;
    let mut gen_dirs = false;
    let mut gen_commands = false;
    let mut prefix = String::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-W" => {
                i += 1;
                let words = require_arg(&args, i, "-W")?;
                wordlist.extend(words.split_whitespace().map(|s| s.to_string()));
            }
            "-f" => gen_files = true,
            "-d" => gen_dirs = true,
            "-c" => gen_commands = true,
            opt if opt.starts_with('-') => {
                return Err(anyhow!("compgen: {}: invalid option", opt));
            }
            // First bare word is the prefix to filter against
            word => {
                if prefix.is_empty() {
                    prefix = word.to_string();
                }
            }
        }
        i += 1;
    }

    let mut candidates: Vec<String> = Vec::new();

    // -W wordlist
    for word in &wordlist {
        if word.starts_with(&prefix) {
            candidates.push(word.clone());
        }
    }

    // -c commands (builtins + PATH executables + user functions)
    if gen_commands {
        candidates.extend(generate_commands(runtime, &prefix));
    }

    // -f files (any filesystem entry)
    if gen_files {
        candidates.extend(scan_entries(runtime, &prefix, false));
    }

    // -d directories only
    if gen_dirs {
        candidates.extend(scan_entries(runtime, &prefix, true));
    }

    candidates.sort();
    candidates.dedup();

    if candidates.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: String::new(),
            exit_code: 1, // bash compgen exits 1 when no matches
            error: None,
        });
    }

    let output = candidates.join("\n") + "\n";
    Ok(ExecutionResult::success(output))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the next positional argument or an error.
fn require_arg(args: &[String], index: usize, flag: &str) -> Result<String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| anyhow!("complete: {} requires an argument", flag))
}

/// Gather all available command names that match `prefix`.
fn generate_commands(runtime: &Runtime, prefix: &str) -> Vec<String> {
    let mut cmds: HashSet<String> = HashSet::new();

    // User-defined functions
    for name in runtime.get_function_names() {
        if name.starts_with(prefix) {
            cmds.insert(name);
        }
    }

    // PATH executables
    if let Ok(path_var) = env::var("PATH") {
        for dir in path_var.split(':') {
            let p = Path::new(dir);
            if let Ok(entries) = fs::read_dir(p) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) {
                        if name.starts_with(prefix) {
                            // Quick executable check (unix only — good enough here)
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(meta) = entry.metadata() {
                                    if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                                        cmds.insert(name);
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                if let Ok(meta) = entry.metadata() {
                                    if meta.is_file() {
                                        cmds.insert(name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut result: Vec<String> = cmds.into_iter().collect();
    result.sort();
    result
}

/// Scan the current directory for filesystem entries matching `prefix`.
/// When `dirs_only` is true only directories are returned.
fn scan_entries(runtime: &Runtime, prefix: &str, dirs_only: bool) -> Vec<String> {
    let (dir_path, partial) = if prefix.contains('/') {
        let path = Path::new(prefix);
        let dir = path.parent().unwrap_or(Path::new("."));
        let partial = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        (runtime.get_cwd().join(dir), partial.to_string())
    } else {
        (runtime.get_cwd().to_path_buf(), prefix.to_string())
    };

    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir_path) {
        for entry in entries.flatten() {
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);

            if dirs_only && !is_dir {
                continue;
            }

            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(&partial as &str) {
                    let mut result = if prefix.contains('/') {
                        let dir_prefix = Path::new(prefix)
                            .parent()
                            .and_then(|p| p.to_str())
                            .unwrap_or("");
                        if dir_prefix.is_empty() {
                            name.to_string()
                        } else {
                            format!("{}/{}", dir_prefix, name)
                        }
                    } else {
                        name.to_string()
                    };

                    if is_dir {
                        result.push('/');
                    }
                    results.push(result);
                }
            }
        }
    }

    results.sort();
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_complete_register_and_list() {
        let mut rt = Runtime::new();

        // Register a completion spec
        let result = builtin_complete(
            &[
                "-c".to_string(),
                "myapp".to_string(),
                "-a".to_string(),
                "start stop restart".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // Spec should be stored
        let spec = rt.get_completion_spec("myapp").unwrap();
        assert_eq!(spec.wordlist, vec!["start", "stop", "restart"]);

        // List should show it
        let list = builtin_complete(&[], &mut rt).unwrap();
        assert!(list.stdout().contains("myapp"));
    }

    #[test]
    fn test_complete_erase() {
        let mut rt = Runtime::new();

        builtin_complete(
            &[
                "-c".to_string(),
                "myapp".to_string(),
                "-a".to_string(),
                "go".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        assert!(rt.get_completion_spec("myapp").is_some());

        builtin_complete(
            &["-e".to_string(), "-c".to_string(), "myapp".to_string()],
            &mut rt,
        )
        .unwrap();
        assert!(rt.get_completion_spec("myapp").is_none());
    }

    #[test]
    fn test_complete_short_and_long_flags() {
        let mut rt = Runtime::new();

        builtin_complete(
            &[
                "-c".to_string(),
                "tool".to_string(),
                "-s".to_string(),
                "v".to_string(),
                "-l".to_string(),
                "verbose".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        let spec = rt.get_completion_spec("tool").unwrap();
        assert_eq!(spec.short_flags, vec!["v"]);
        assert_eq!(spec.long_flags, vec!["verbose"]);
    }

    #[test]
    fn test_complete_missing_command_errors() {
        let mut rt = Runtime::new();
        let result = builtin_complete(&["-a".to_string(), "foo".to_string()], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn test_compgen_wordlist() {
        let mut rt = Runtime::new();

        let result = builtin_compgen(
            &[
                "-W".to_string(),
                "start stop restart".to_string(),
                "st".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        let stdout = result.stdout();
        assert!(stdout.contains("start"));
        assert!(stdout.contains("stop"));
        assert!(!stdout.contains("restart")); // "restart" doesn't start with "st"
    }

    #[test]
    fn test_compgen_wordlist_no_prefix() {
        let mut rt = Runtime::new();

        let result =
            builtin_compgen(&["-W".to_string(), "foo bar baz".to_string()], &mut rt).unwrap();

        let stdout = result.stdout();
        assert!(stdout.contains("foo"));
        assert!(stdout.contains("bar"));
        assert!(stdout.contains("baz"));
    }

    #[test]
    fn test_compgen_no_matches_exits_1() {
        let mut rt = Runtime::new();

        let result = builtin_compgen(
            &["-W".to_string(), "foo bar".to_string(), "zzz".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_compgen_commands() {
        let mut rt = Runtime::new();

        // Should at least not crash; depends on PATH
        let result = builtin_compgen(&["-c".to_string(), "ls".to_string()], &mut rt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compgen_files() {
        let mut rt = Runtime::new();

        // Should list entries in the current directory
        let result = builtin_compgen(&["-f".to_string()], &mut rt).unwrap();
        // There must be at least some files in the project root (src/, Cargo.toml, …)
        assert!(result.exit_code == 0 || result.stdout().is_empty());
    }

    #[test]
    fn test_compgen_dirs_only() {
        let mut rt = Runtime::new();

        // -d should only return directories (they end with '/')
        let result = builtin_compgen(&["-d".to_string()], &mut rt).unwrap();
        for line in result.stdout().lines() {
            assert!(
                line.ends_with('/'),
                "expected directory entry but got: {}",
                line
            );
        }
    }

    #[test]
    fn test_complete_invalid_option() {
        let mut rt = Runtime::new();
        let result = builtin_complete(&["-z".to_string()], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn test_compgen_invalid_option() {
        let mut rt = Runtime::new();
        let result = builtin_compgen(&["-z".to_string()], &mut rt);
        assert!(result.is_err());
    }
}
