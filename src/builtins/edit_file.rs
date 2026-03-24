use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::path::Path;

/// `edit` builtin — surgical find-and-replace in a file with undo tracking.
///
/// Usage:
///   edit <path> --old <text> --new <text>
///
/// Replaces the first exact match of `old` with `new`. Tracks the
/// operation in the undo system so `undo` can revert it.
pub fn builtin_edit(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut path_str: Option<&str> = None;
    let mut old_text: Option<&str> = None;
    let mut new_text: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--old" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("edit: --old requires a value"));
                }
                old_text = Some(&args[i]);
            }
            "--new" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("edit: --new requires a value"));
                }
                new_text = Some(&args[i]);
            }
            _ => {
                if path_str.is_none() {
                    path_str = Some(&args[i]);
                } else {
                    return Err(anyhow!("edit: unexpected argument '{}'", args[i]));
                }
            }
        }
        i += 1;
    }

    let path_str = path_str.ok_or_else(|| {
        anyhow!("edit: missing file path\nUsage: edit <path> --old <text> --new <text>")
    })?;
    let old_text = old_text.ok_or_else(|| anyhow!("edit: --old is required"))?;
    let new_text = new_text.ok_or_else(|| anyhow!("edit: --new is required"))?;

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(anyhow!("edit: {}: No such file", path_str));
    }

    let content = std::fs::read_to_string(path)?;

    if !content.contains(old_text) {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("edit: old text not found in {}\n", path_str),
            exit_code: 1,
            error: None,
        });
    }

    // Track in undo system before modifying
    runtime
        .undo_manager_mut()
        .track_modify(&path.to_path_buf(), format!("edit {}", path_str))
        .ok();

    let updated = content.replacen(old_text, new_text, 1);
    std::fs::write(path, &updated)?;

    // Count what changed for the summary
    let old_lines = old_text.lines().count();
    let new_lines = new_text.lines().count();

    Ok(ExecutionResult {
        output: Output::Text(format!(
            "Edited {} ({} lines → {} lines)\n",
            path_str, old_lines, new_lines
        )),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}
