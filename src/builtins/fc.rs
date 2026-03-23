use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// The `fc` builtin — list, edit, and re-execute history entries.
///
/// Usage:
///   fc -l [-nr] [first [last]]        List history entries
///   fc [-e editor] [first [last]]     Edit entries and re-execute
///   fc -s [old=new] [first]           Re-execute without editing (with optional substitution)
///
/// Entry references can be:
///   - A positive integer: absolute history index (1-based)
///   - A negative integer: offset from end of history (-1 = most recent)
///   - A string: most recent entry whose command starts with that string
///
/// Flags:
///   -l   List entries (don't edit/execute)
///   -n   Suppress line numbers when listing
///   -r   Reverse order when listing
///   -s   Re-execute without opening editor
///   -e editor   Use the specified editor instead of $FCEDIT / $EDITOR / vi
pub fn builtin_fc(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // ── parse flags ────────────────────────────────────────────────────────
    let mut do_list = false;
    let mut no_numbers = false;
    let mut reverse = false;
    let mut do_silent = false; // -s: re-execute without editing
    let mut editor_override: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-l" => do_list = true,
            "-n" => no_numbers = true,
            "-r" => reverse = true,
            "-s" | "-e" if args[i] == "-s" => do_silent = true,
            "-e" => {
                i += 1;
                editor_override = Some(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| anyhow!("fc: -e: requires an editor argument"))?,
                );
            }
            // Treat "--" as end of options
            "--" => {
                positional.extend_from_slice(&args[i + 1..]);
                break;
            }
            arg if arg.starts_with('-')
                && arg.len() > 1
                && arg[1..].chars().all(|c| "lnrse".contains(c)) =>
            {
                // Clustered short flags like -ln, -nr, -lr
                for ch in arg[1..].chars() {
                    match ch {
                        'l' => do_list = true,
                        'n' => no_numbers = true,
                        'r' => reverse = true,
                        's' => do_silent = true,
                        'e' => {
                            i += 1;
                            editor_override =
                                Some(args.get(i).cloned().ok_or_else(|| {
                                    anyhow!("fc: -e: requires an editor argument")
                                })?);
                        }
                        _ => {}
                    }
                }
            }
            _ => positional.push(args[i].clone()),
        }
        i += 1;
    }

    let history = runtime.history_mut();
    let entries = history.entries();

    if entries.is_empty() {
        if do_list {
            return Ok(ExecutionResult::success(String::new()));
        }
        return Err(anyhow!("fc: no commands in history"));
    }

    let hist_len = entries.len();

    // ── resolve a range reference to a 0-based index ───────────────────────
    let resolve_ref = |r: &str| -> Option<usize> {
        if let Ok(n) = r.parse::<i64>() {
            if n >= 1 {
                // 1-based absolute index
                let idx = (n as usize).saturating_sub(1);
                if idx < hist_len {
                    Some(idx)
                } else {
                    None
                }
            } else if n < 0 {
                // Negative: offset from most-recent
                let idx = (hist_len as i64 + n) as usize;
                if idx < hist_len {
                    Some(idx)
                } else {
                    None
                }
            } else {
                None // 0 is invalid
            }
        } else {
            // String prefix: find most-recent matching entry
            (0..hist_len)
                .rev()
                .find(|&i| entries[i].command.starts_with(r))
        }
    };

    // ── list mode ──────────────────────────────────────────────────────────
    if do_list {
        // Default range: last 16 entries (bash default)
        let (first, last) = match positional.len() {
            0 => {
                let first = if hist_len > 16 { hist_len - 16 } else { 0 };
                (first, hist_len - 1)
            }
            1 => {
                let idx = resolve_ref(&positional[0])
                    .ok_or_else(|| anyhow!("fc: {}: not found", positional[0]))?;
                (idx, hist_len - 1)
            }
            _ => {
                let first = resolve_ref(&positional[0])
                    .ok_or_else(|| anyhow!("fc: {}: not found", positional[0]))?;
                let last = resolve_ref(&positional[1])
                    .ok_or_else(|| anyhow!("fc: {}: not found", positional[1]))?;
                (first.min(last), first.max(last))
            }
        };

        let mut indices: Vec<usize> = (first..=last).collect();
        if reverse {
            indices.reverse();
        }

        let mut out = String::new();
        for idx in indices {
            let cmd = &entries[idx].command;
            // History numbers are 1-based to match bash/POSIX
            let line_no = idx + 1;
            if no_numbers {
                out.push_str(&format!("\t{}\n", cmd));
            } else {
                out.push_str(&format!("{}\t{}\n", line_no, cmd));
            }
        }

        return Ok(ExecutionResult::success(out));
    }

    // ── resolve the target command ─────────────────────────────────────────
    // For -s and editing modes, we default to the most-recent entry.
    let target_idx = match positional.first().filter(|s| !s.contains('=')) {
        None => hist_len - 1,
        Some(r) => resolve_ref(r).ok_or_else(|| anyhow!("fc: {}: not found", r))?,
    };

    let mut command_text = entries[target_idx].command.clone();

    // Apply old=new substitution pairs (positional args that contain '=')
    for sub in positional.iter().filter(|s| s.contains('=')) {
        let (old, new) = sub.split_once('=').unwrap();
        command_text = command_text.replacen(old, new, 1);
    }

    // ── silent re-execution (-s) ───────────────────────────────────────────
    if do_silent {
        return execute_command(&command_text, runtime);
    }

    // ── editor mode ───────────────────────────────────────────────────────
    // Handle a range of entries for the editor (second positional arg = last).
    let last_idx = if positional.len() >= 2 {
        match positional.get(1) {
            Some(r) => resolve_ref(r).ok_or_else(|| anyhow!("fc: {}: not found", r))?,
            None => target_idx,
        }
    } else {
        target_idx
    };

    let (range_first, range_last) = (target_idx.min(last_idx), target_idx.max(last_idx));

    // Build the text to edit: each command on its own line.
    let edit_text: String = entries[range_first..=range_last]
        .iter()
        .map(|e| e.command.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    // Write to a temp file.
    let tmp_path = std::env::temp_dir().join(format!("rush_fc_{}.sh", std::process::id()));
    std::fs::write(&tmp_path, &edit_text)
        .map_err(|e| anyhow!("fc: failed to write temp file: {}", e))?;

    // Determine editor: -e flag > $FCEDIT > $EDITOR > vi
    let editor = editor_override
        .or_else(|| std::env::var("FCEDIT").ok())
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "vi".to_string());

    // Open the editor.
    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .map_err(|e| anyhow!("fc: {}: {}", editor, e))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: String::new(),
            exit_code: status.code().unwrap_or(1),
            error: None,
        });
    }

    // Read back the (possibly modified) commands.
    let edited = std::fs::read_to_string(&tmp_path)
        .map_err(|e| anyhow!("fc: failed to read temp file: {}", e))?;
    let _ = std::fs::remove_file(&tmp_path);

    // Execute each non-empty, non-comment line from the edited file.
    let mut last_result = ExecutionResult::success(String::new());
    for line in edited.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        last_result = execute_command(line, runtime)?;
    }

    Ok(last_result)
}

/// Parse and execute a single command string within the current runtime.
fn execute_command(command: &str, runtime: &mut Runtime) -> Result<ExecutionResult> {
    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    let tokens = Lexer::tokenize(command).map_err(|e| anyhow!("fc: tokenize error: {}", e))?;

    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("fc: parse error: {}", e))?;

    let mut executor = Executor::new_embedded();
    *executor.runtime_mut() = runtime.clone();

    let result = executor
        .execute(statements)
        .map_err(|e| anyhow!("fc: execution error: {}", e))?;

    *runtime = executor.runtime_mut().clone();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_with_history(cmds: &[&str]) -> Runtime {
        let mut rt = Runtime::new();
        for cmd in cmds {
            rt.history_mut().add(cmd.to_string()).unwrap();
        }
        rt
    }

    #[test]
    fn test_fc_list_empty_history() {
        let mut runtime = Runtime::new();
        let args = vec!["-l".to_string()];
        let result = builtin_fc(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout(), "");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_fc_list_shows_commands() {
        let mut rt = runtime_with_history(&["echo hello", "ls -la", "pwd"]);
        let args = vec!["-l".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert!(result.stdout().contains("echo hello"));
        assert!(result.stdout().contains("ls -la"));
        assert!(result.stdout().contains("pwd"));
    }

    #[test]
    fn test_fc_list_no_numbers() {
        let mut rt = runtime_with_history(&["echo hello", "ls"]);
        let args = vec!["-l".to_string(), "-n".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        // Lines should start with a tab, not a number.
        for line in result.stdout().lines() {
            assert!(
                line.starts_with('\t'),
                "expected tab-indented line, got: {:?}",
                line
            );
        }
    }

    #[test]
    fn test_fc_list_reverse() {
        let mut rt = runtime_with_history(&["echo hello", "ls -la", "pwd"]);
        let args = vec!["-l".to_string(), "-r".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        let stdout = result.stdout();
        let pos_hello = stdout.find("echo hello").unwrap();
        let pos_pwd = stdout.find("pwd").unwrap();
        // In reverse order, "pwd" (most-recent) should appear before "echo hello".
        assert!(
            pos_pwd < pos_hello,
            "reverse: pwd should come before echo hello"
        );
    }

    #[test]
    fn test_fc_list_with_range() {
        let mut rt = runtime_with_history(&["cmd1", "cmd2", "cmd3", "cmd4"]);
        // List entries 2..3 (1-based)
        let args = vec!["-l".to_string(), "2".to_string(), "3".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert!(result.stdout().contains("cmd2"));
        assert!(result.stdout().contains("cmd3"));
        assert!(!result.stdout().contains("cmd1"));
        assert!(!result.stdout().contains("cmd4"));
    }

    #[test]
    fn test_fc_list_negative_index() {
        let mut rt = runtime_with_history(&["cmd1", "cmd2", "cmd3"]);
        // -1 = most recent (cmd3), range -1..-1 means just the last entry
        let args = vec!["-l".to_string(), "-1".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert!(result.stdout().contains("cmd3"));
    }

    #[test]
    fn test_fc_silent_reexec() {
        let mut rt = runtime_with_history(&["echo hello"]);
        let args = vec!["-s".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("hello"));
    }

    #[test]
    fn test_fc_silent_with_substitution() {
        let mut rt = runtime_with_history(&["echo hello"]);
        // Re-execute most-recent, replacing "hello" with "world"
        let args = vec!["-s".to_string(), "hello=world".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("world"));
    }

    #[test]
    fn test_fc_no_history_error() {
        let mut runtime = Runtime::new();
        let result = builtin_fc(&[], &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_fc_list_string_prefix() {
        let mut rt = runtime_with_history(&["echo hello", "ls -la", "echo world"]);
        // List from "echo" (should match most-recent "echo world") to end
        let args = vec!["-l".to_string(), "echo".to_string()];
        let result = builtin_fc(&args, &mut rt).unwrap();
        assert!(result.stdout().contains("echo world"));
    }
}
