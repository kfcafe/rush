use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::path::Path;

/// `write` builtin — write content to a file with undo tracking.
///
/// Usage:
///   write <path> <content>         Write string content to file
///   write <path> --stdin           Read content from piped stdin
///
/// Creates parent directories automatically. Tracks the operation in
/// the undo system so `undo` can revert it.
pub fn builtin_write(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_write_with_stdin(args, runtime, None)
}

pub fn builtin_write_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: Option<&[u8]>,
) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!(
            "write: missing file path\nUsage: write <path> <content>"
        ));
    }

    let path_str = &args[0];
    let path = Path::new(path_str);

    // Determine content source
    let content = if args.len() >= 2 && args[1] == "--stdin" {
        // Read from piped stdin
        match stdin_data {
            Some(data) => String::from_utf8_lossy(data).to_string(),
            None => {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                buf
            }
        }
    } else if args.len() >= 2 {
        // Content is the remaining args joined
        args[1..].join(" ")
    } else {
        // Read from piped stdin (default when no content arg)
        match stdin_data {
            Some(data) => String::from_utf8_lossy(data).to_string(),
            None => {
                return Err(anyhow!(
                    "write: missing content\nUsage: write <path> <content>"
                ))
            }
        }
    };

    // Track in undo system
    let undo = runtime.undo_manager_mut();
    if path.exists() {
        undo.track_modify(&path.to_path_buf(), format!("write {}", path_str))
            .ok();
    } else {
        undo.track_create(path.to_path_buf(), format!("write {}", path_str));
    }

    // Create parent directories
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let bytes = content.len();
    std::fs::write(path, &content)?;

    Ok(ExecutionResult {
        output: Output::Text(format!("Wrote {} bytes to {}\n", bytes, path_str)),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}
