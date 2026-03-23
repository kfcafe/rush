//! `funced` — interactively edit a shell function definition.
//!
//! Opens the function's saved file (or a temp file with a stub) in `$EDITOR`.
//! After the editor exits, the file is sourced back into the current session.
//!
//! Usage:
//!   funced <function_name>
//!
//! Exit codes:
//!   0 — editor exited cleanly and the file was re-sourced
//!   1 — error (no name given, editor not found, source failed, etc.)

use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn builtin_funced(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("funced: usage: funced <function_name>"));
    }

    let name = &args[0];

    // Determine the file to edit: prefer the persisted autoload file, fall
    // back to a freshly-created temp file with a skeleton.
    let edit_path = resolve_edit_path(name, runtime)?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let status = Command::new(&editor)
        .arg(&edit_path)
        .status()
        .map_err(|e| anyhow!("funced: failed to launch '{}': {}", editor, e))?;

    if !status.success() {
        return Ok(ExecutionResult {
            output: crate::executor::Output::Text(String::new()),
            stderr: format!("funced: editor exited with non-zero status\n"),
            exit_code: status.code().unwrap_or(1),
            error: None,
        });
    }

    // Re-source the edited file so the updated function is live immediately.
    let contents = fs::read_to_string(&edit_path)
        .map_err(|e| anyhow!("funced: failed to read '{}': {}", edit_path.display(), e))?;

    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    let tokens = Lexer::tokenize(&contents)
        .map_err(|e| anyhow!("funced: parse error in edited file: {}", e))?;
    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("funced: parse error in edited file: {}", e))?;

    let mut executor = Executor::new();
    *executor.runtime_mut() = runtime.clone();
    let result = executor.execute(statements)?;
    *runtime = executor.runtime_mut().clone();

    Ok(ExecutionResult::success(result.stdout().to_string()))
}

/// Return the path of the file to edit for `name`:
///  1. The persisted autoload file if it exists.
///  2. A new file in `~/.config/rush/functions/` with a stub if the function
///     is defined in-memory.
///  3. A temp file with a blank skeleton otherwise.
fn resolve_edit_path(name: &str, runtime: &Runtime) -> Result<PathBuf> {
    // 1. Already-saved autoload file.
    if let Some(path) = runtime.get_autoload_file(name) {
        return Ok(path);
    }

    // 2. Function exists in memory — write it to the canonical save location.
    let functions_dir = dirs::home_dir()
        .map(|h| h.join(".config").join("rush").join("functions"))
        .ok_or_else(|| anyhow!("funced: cannot determine home directory"))?;

    fs::create_dir_all(&functions_dir)
        .map_err(|e| anyhow!("funced: failed to create functions directory: {}", e))?;

    let path = functions_dir.join(format!("{}.rush", name));

    if !path.exists() {
        let stub = if runtime.get_function(name).is_some() {
            format!(
                "function {}\n    # Edit the function body here\nend\n",
                name
            )
        } else {
            format!(
                "function {}\n    # New function — add the body here\nend\n",
                name
            )
        };
        fs::write(&path, stub).map_err(|e| anyhow!("funced: failed to create stub: {}", e))?;
    }

    Ok(path)
}
