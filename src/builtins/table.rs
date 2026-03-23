// `table` builtin — render a JSON array of objects as an aligned terminal table.
//
// Usage:
//   ls --json | table
//   table data.json

use crate::executor::{ExecutionResult, Output};
use crate::output::table::render_json_table;
use crate::runtime::Runtime;
use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Read};

pub fn builtin_table(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_table_impl(args, None)
}

pub fn builtin_table_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    builtin_table_impl(args, Some(stdin_data))
}

fn builtin_table_impl(args: &[String], stdin_data: Option<&[u8]>) -> Result<ExecutionResult> {
    let input = read_input(args, stdin_data)?;

    let value: serde_json::Value =
        serde_json::from_str(&input).context("table: input is not valid JSON")?;

    match render_json_table(&value) {
        Some(rendered) => Ok(ExecutionResult::success(rendered)),
        None => {
            // Not an array of objects — fall back to pretty-printed JSON.
            let pretty =
                serde_json::to_string_pretty(&value).context("table: failed to format JSON")?;
            Ok(ExecutionResult::success(pretty + "\n"))
        }
    }
}

fn read_input(file_args: &[String], stdin_data: Option<&[u8]>) -> Result<String> {
    if let Some(data) = stdin_data {
        return Ok(String::from_utf8_lossy(data).into_owned());
    }

    if !file_args.is_empty() {
        let path = &file_args[0];
        return fs::read_to_string(path)
            .with_context(|| format!("table: cannot read file '{}'", path));
    }

    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("table: failed to read from stdin")?;
    Ok(buf)
}
