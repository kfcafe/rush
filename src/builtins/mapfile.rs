use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::io::{self, BufRead};

/// Options parsed from mapfile/readarray arguments
#[derive(Debug, Default)]
struct MapfileOptions {
    /// Trim trailing newline from each line (-t)
    trim_newline: bool,
    /// Maximum lines to read (-n COUNT, 0 means unlimited)
    count: usize,
    /// Starting array index (-O ORIGIN, default 0)
    origin: usize,
    /// Array variable name (positional, required)
    array_name: String,
}

impl MapfileOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = MapfileOptions::default();
        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "-t" => opts.trim_newline = true,
                "-n" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("mapfile: -n: option requires an argument"));
                    }
                    opts.count = args[i]
                        .parse::<usize>()
                        .map_err(|_| anyhow!("mapfile: {}: invalid count", args[i]))?;
                }
                "-O" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("mapfile: -O: option requires an argument"));
                    }
                    opts.origin = args[i]
                        .parse::<usize>()
                        .map_err(|_| anyhow!("mapfile: {}: invalid origin", args[i]))?;
                }
                arg if arg.starts_with('-') => {
                    return Err(anyhow!("mapfile: {}: invalid option", arg));
                }
                name => {
                    opts.array_name = name.to_string();
                    break;
                }
            }
            i += 1;
        }

        if opts.array_name.is_empty() {
            // Default array name if none provided
            opts.array_name = "MAPFILE".to_string();
        }

        Ok(opts)
    }
}

/// Store lines into the runtime as indexed array variables.
///
/// Arrays are stored as `NAME[0]`, `NAME[1]`, etc. since this shell does not
/// have a native array type. We also set `NAME` to the space-joined value for
/// simple use-cases, and `NAME_count` to the number of elements.
fn store_array(runtime: &mut Runtime, name: &str, lines: &[String], origin: usize) {
    // Clear any existing elements (simple strategy: set count to 0 first)
    let count = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let key = format!("{}[{}]", name, origin + i);
        runtime.set_variable(key, line.clone());
    }
    // Store the element count as a companion variable
    runtime.set_variable(format!("{}_count", name), count.to_string());
    // Also expose as plain variable (newline-joined) for simple expansions
    runtime.set_variable(name.to_string(), lines.join("\n"));
}

/// Read lines from a byte slice into the array
fn read_lines_from_bytes(data: &[u8], opts: &MapfileOptions) -> Vec<String> {
    let cursor = std::io::Cursor::new(data);
    collect_lines(cursor, opts)
}

/// Read lines from a BufRead source, applying mapfile options
fn collect_lines<R: BufRead>(reader: R, opts: &MapfileOptions) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line_result in reader.lines() {
        if let Ok(mut line) = line_result {
            if !opts.trim_newline {
                line.push('\n');
            }
            lines.push(line);
            if opts.count > 0 && lines.len() >= opts.count {
                break;
            }
        }
    }
    lines
}

/// Entry point when called without piped stdin (tries to read from real stdin)
pub fn builtin_mapfile(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = MapfileOptions::parse(args)?;

    // Read from real stdin
    let stdin = io::stdin();
    let reader = stdin.lock();
    let lines = collect_lines(reader, &opts);

    store_array(runtime, &opts.array_name, &lines, opts.origin);

    Ok(ExecutionResult::success(String::new()))
}

/// Entry point when stdin data is piped/redirected (common case for `mapfile ARRAY < file`)
pub fn builtin_mapfile_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = MapfileOptions::parse(args)?;
    let lines = read_lines_from_bytes(stdin_data, &opts);
    store_array(runtime, &opts.array_name, &lines, opts.origin);
    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_mapfile_basic() {
        let mut rt = Runtime::new();
        let data = b"line1\nline2\nline3\n";
        let res = builtin_mapfile_with_stdin(&["MYARRAY".to_string()], &mut rt, data).unwrap();
        assert_eq!(res.exit_code, 0);

        // Lines include trailing newline by default
        assert_eq!(rt.get_variable("MYARRAY[0]"), Some("line1\n".to_string()));
        assert_eq!(rt.get_variable("MYARRAY[1]"), Some("line2\n".to_string()));
        assert_eq!(rt.get_variable("MYARRAY[2]"), Some("line3\n".to_string()));
        assert_eq!(rt.get_variable("MYARRAY_count"), Some("3".to_string()));
    }

    #[test]
    fn test_mapfile_trim() {
        let mut rt = Runtime::new();
        let data = b"alpha\nbeta\ngamma\n";
        builtin_mapfile_with_stdin(&["-t".to_string(), "LINES".to_string()], &mut rt, data)
            .unwrap();

        assert_eq!(rt.get_variable("LINES[0]"), Some("alpha".to_string()));
        assert_eq!(rt.get_variable("LINES[1]"), Some("beta".to_string()));
        assert_eq!(rt.get_variable("LINES[2]"), Some("gamma".to_string()));
    }

    #[test]
    fn test_mapfile_count_limit() {
        let mut rt = Runtime::new();
        let data = b"a\nb\nc\nd\ne\n";
        builtin_mapfile_with_stdin(
            &[
                "-n".to_string(),
                "3".to_string(),
                "-t".to_string(),
                "ARR".to_string(),
            ],
            &mut rt,
            data,
        )
        .unwrap();

        assert_eq!(rt.get_variable("ARR_count"), Some("3".to_string()));
        assert_eq!(rt.get_variable("ARR[0]"), Some("a".to_string()));
        assert_eq!(rt.get_variable("ARR[2]"), Some("c".to_string()));
        // Fourth element should not exist
        assert_eq!(rt.get_variable("ARR[3]"), None);
    }

    #[test]
    fn test_mapfile_default_name() {
        let mut rt = Runtime::new();
        let data = b"one\ntwo\n";
        // No array name given — should default to MAPFILE
        builtin_mapfile_with_stdin(&["-t".to_string()], &mut rt, data).unwrap();
        assert_eq!(rt.get_variable("MAPFILE[0]"), Some("one".to_string()));
    }

    #[test]
    fn test_mapfile_origin() {
        let mut rt = Runtime::new();
        let data = b"x\ny\n";
        builtin_mapfile_with_stdin(
            &[
                "-t".to_string(),
                "-O".to_string(),
                "5".to_string(),
                "BUF".to_string(),
            ],
            &mut rt,
            data,
        )
        .unwrap();
        assert_eq!(rt.get_variable("BUF[5]"), Some("x".to_string()));
        assert_eq!(rt.get_variable("BUF[6]"), Some("y".to_string()));
    }
}
