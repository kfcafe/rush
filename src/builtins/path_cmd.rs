use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn builtin_path(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    run_path_with_stdin(args, runtime, None)
}

pub fn builtin_path_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    run_path_with_stdin(args, runtime, Some(stdin_data))
}

fn run_path_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let subcommand = match args.first() {
        Some(s) => s.as_str(),
        None => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: usage(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let rest = &args[1..];

    match subcommand {
        "basename" => run_basename(rest, stdin),
        "dirname" => run_dirname(rest, stdin),
        "extension" => run_extension(rest, stdin),
        "stem" => run_stem(rest, stdin),
        "resolve" => run_resolve(rest, stdin),
        "normalize" => run_normalize(rest, stdin),
        "join" => run_join(rest, stdin),
        "is" => run_is(rest, stdin),
        // Alias: change-extension with empty string = stem
        "change-extension" => run_change_extension(rest, stdin),
        other => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("path: unknown subcommand '{}'\n{}", other, usage()),
            exit_code: 1,
            error: None,
        }),
    }
}

/// Read paths from args or, if empty, from stdin (one per line).
fn paths_from(args: &[String], stdin: Option<&[u8]>) -> Vec<String> {
    if args.is_empty() {
        let data = stdin.map(|d| d.to_vec()).unwrap_or_else(|| {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
            buf
        });
        let text = String::from_utf8_lossy(&data);
        text.lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect()
    } else {
        args.to_vec()
    }
}

/// Collect results into a newline-separated string, returning exit_code 0 if any
/// results were produced, 1 if none.
fn collect_results(results: Vec<String>) -> ExecutionResult {
    let exit_code = if results.is_empty() { 1 } else { 0 };
    let mut output = results.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    }
}

// --- Subcommand implementations ---

fn run_basename(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let results: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .collect();
    Ok(collect_results(results))
}

fn run_dirname(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let results: Vec<String> = paths
        .iter()
        .map(|p| {
            Path::new(p)
                .parent()
                .map(|d| {
                    let s = d.to_string_lossy();
                    if s.is_empty() {
                        ".".to_string()
                    } else {
                        s.into_owned()
                    }
                })
                .unwrap_or_else(|| ".".to_string())
        })
        .collect();
    Ok(collect_results(results))
}

fn run_extension(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let results: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            Path::new(p)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
        })
        .collect();
    Ok(collect_results(results))
}

fn run_stem(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let results: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    Ok(collect_results(results))
}

fn run_resolve(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let mut results = Vec::new();
    let mut stderr_lines = Vec::new();
    let mut exit_code = 0;

    for p in &paths {
        match std::fs::canonicalize(p) {
            Ok(abs) => results.push(abs.to_string_lossy().into_owned()),
            Err(e) => {
                stderr_lines.push(format!("path resolve: {}: {}", p, e));
                exit_code = 1;
            }
        }
    }

    let mut output = results.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: if stderr_lines.is_empty() {
            String::new()
        } else {
            stderr_lines.join("\n") + "\n"
        },
        exit_code,
        error: None,
    })
}

fn run_normalize(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    let paths = paths_from(args, stdin);
    let results: Vec<String> = paths.iter().map(|p| normalize_path(p)).collect();
    Ok(collect_results(results))
}

/// Pure lexical normalization (no filesystem access):
/// - Collapse multiple slashes
/// - Remove `.` components
/// - Resolve `..` components
fn normalize_path(path: &str) -> String {
    let is_absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();

    for component in path.split('/') {
        match component {
            "" | "." => {} // skip empties and dot
            ".." => {
                if parts.last().map(|p| *p != "..").unwrap_or(false) || is_absolute {
                    parts.pop();
                } else if !is_absolute {
                    parts.push("..");
                }
            }
            c => parts.push(c),
        }
    }

    if is_absolute {
        format!("/{}", parts.join("/"))
    } else if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn run_join(args: &[String], _stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "path join: at least one component required\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let mut result = PathBuf::new();
    for component in args {
        result.push(component);
    }

    let mut output = result.to_string_lossy().into_owned();
    output.push('\n');

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

/// `path is [-f|-d|-l|-e] [PATH...]`
/// Returns exit 0 if ALL specified paths match the test, 1 otherwise.
/// Outputs the matching paths (one per line).
fn run_is(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    // Parse flags (may appear before paths)
    let mut flags: Vec<char> = Vec::new();
    let mut path_args: Vec<String> = Vec::new();
    let mut parsing_flags = true;

    for arg in args {
        if parsing_flags && arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'f' | 'd' | 'l' | 'e' => flags.push(ch),
                    _ => {
                        return Ok(ExecutionResult {
                            output: Output::Text(String::new()),
                            stderr: format!("path is: unknown flag '-{}'\nUsage: path is [-f|-d|-l|-e] [PATH...]\n", ch),
                            exit_code: 1,
                            error: None,
                        });
                    }
                }
            }
        } else {
            parsing_flags = false;
            path_args.push(arg.clone());
        }
    }

    let paths = paths_from(&path_args, stdin);

    // Default: check existence (-e) if no flags
    if flags.is_empty() {
        flags.push('e');
    }

    let mut matched = Vec::new();
    let mut any_failed = false;

    for p in &paths {
        let path = Path::new(p);
        let matches = flags.iter().all(|&f| match f {
            'e' => path.exists(),
            'f' => path.is_file(),
            'd' => path.is_dir(),
            'l' => path.is_symlink(),
            _ => false,
        });

        if matches {
            matched.push(p.clone());
        } else {
            any_failed = true;
        }
    }

    let mut output = matched.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }

    // Exit 0 only if every path passed the test
    let exit_code = if any_failed || paths.is_empty() { 1 } else { 0 };

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

/// `path change-extension EXT [PATH...]` — replace or strip extension.
/// If EXT is empty or `.`, removes the extension. Otherwise adds/replaces.
fn run_change_extension(args: &[String], stdin: Option<&[u8]>) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "path change-extension: missing extension argument\nUsage: path change-extension EXT [PATH...]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let new_ext = &args[0];
    let path_args = &args[1..];
    let paths = paths_from(path_args, stdin);

    let results: Vec<String> = paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            let parent = path.parent();
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());

            match stem {
                None => p.clone(),
                Some(stem_str) => {
                    let base = if new_ext.is_empty() || new_ext == "." {
                        stem_str
                    } else {
                        let ext = new_ext.trim_start_matches('.');
                        format!("{}.{}", stem_str, ext)
                    };

                    match parent {
                        Some(par) if par != Path::new("") => {
                            format!("{}/{}", par.to_string_lossy(), base)
                        }
                        _ => base,
                    }
                }
            }
        })
        .collect();

    Ok(collect_results(results))
}

fn usage() -> String {
    "Usage: path <subcommand> [OPTIONS] [PATH...]

Subcommands:
  basename [PATH...]               Get filename component
  dirname  [PATH...]               Get directory component
  extension [PATH...]              Get file extension (including dot)
  stem     [PATH...]               Get filename without extension
  resolve  [PATH...]               Resolve to absolute path (realpath)
  normalize [PATH...]              Normalize path (remove ./ and ../)
  join     PART...                 Join path components
  is       [-f|-d|-l|-e] [PATH...]  Test path type (file/dir/link/exists)
  change-extension EXT [PATH...]   Change or strip file extension

All subcommands read paths from stdin if no PATH args are given.\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn rt() -> Runtime {
        Runtime::new()
    }

    fn path(args: &[&str]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_path(&args, &mut rt()).unwrap()
    }

    fn path_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_path_with_stdin(&args, &mut rt(), input).unwrap()
    }

    fn stdout(r: &ExecutionResult) -> &str {
        match &r.output {
            Output::Text(t) => t.as_str(),
            _ => "",
        }
    }

    // --- basename ---

    #[test]
    fn test_builtin_path_basename_simple() {
        let r = path(&["basename", "/usr/local/bin/fish"]);
        assert_eq!(stdout(&r), "fish\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_basename_with_extension() {
        let r = path(&["basename", "/home/user/file.txt"]);
        assert_eq!(stdout(&r), "file.txt\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_basename_relative() {
        let r = path(&["basename", "foo/bar/baz"]);
        assert_eq!(stdout(&r), "baz\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_basename_multiple() {
        let r = path(&["basename", "/a/b", "/c/d"]);
        assert_eq!(stdout(&r), "b\nd\n");
    }

    #[test]
    fn test_builtin_path_basename_stdin() {
        let r = path_stdin(&["basename"], b"/a/b/c\n/x/y/z\n");
        assert_eq!(stdout(&r), "c\nz\n");
        assert_eq!(r.exit_code, 0);
    }

    // --- dirname ---

    #[test]
    fn test_builtin_path_dirname_simple() {
        let r = path(&["dirname", "/usr/local/bin/fish"]);
        assert_eq!(stdout(&r), "/usr/local/bin\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_dirname_no_parent() {
        let r = path(&["dirname", "file.txt"]);
        assert_eq!(stdout(&r), ".\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_dirname_multiple() {
        let r = path(&["dirname", "/a/b", "/c/d"]);
        assert_eq!(stdout(&r), "/a\n/c\n");
    }

    #[test]
    fn test_builtin_path_dirname_stdin() {
        let r = path_stdin(&["dirname"], b"/a/b/c\n");
        assert_eq!(stdout(&r), "/a/b\n");
    }

    // --- extension ---

    #[test]
    fn test_builtin_path_extension_present() {
        let r = path(&["extension", "file.txt"]);
        assert_eq!(stdout(&r), ".txt\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_extension_absent() {
        let r = path(&["extension", "Makefile"]);
        // No extension — produces no output, exit 1
        assert_eq!(stdout(&r), "");
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn test_builtin_path_extension_multiple() {
        let r = path(&["extension", "a.rs", "b.toml", "Makefile"]);
        assert_eq!(stdout(&r), ".rs\n.toml\n");
    }

    // --- stem ---

    #[test]
    fn test_builtin_path_stem_simple() {
        let r = path(&["stem", "file.txt"]);
        assert_eq!(stdout(&r), "file\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_stem_no_extension() {
        let r = path(&["stem", "Makefile"]);
        assert_eq!(stdout(&r), "Makefile\n");
    }

    #[test]
    fn test_builtin_path_stem_full_path() {
        let r = path(&["stem", "/home/user/archive.tar.gz"]);
        // file_stem on "archive.tar.gz" gives "archive.tar"
        assert_eq!(stdout(&r), "archive.tar\n");
    }

    // --- normalize ---

    #[test]
    fn test_builtin_path_normalize_dots() {
        let r = path(&["normalize", "/a/./b/../c"]);
        assert_eq!(stdout(&r), "/a/c\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_normalize_trailing_slash() {
        let r = path(&["normalize", "/a/b//"]);
        assert_eq!(stdout(&r), "/a/b\n");
    }

    #[test]
    fn test_builtin_path_normalize_relative() {
        let r = path(&["normalize", "foo/./bar/../baz"]);
        assert_eq!(stdout(&r), "foo/baz\n");
    }

    #[test]
    fn test_builtin_path_normalize_empty_result() {
        let r = path(&["normalize", "."]);
        assert_eq!(stdout(&r), ".\n");
    }

    // --- join ---

    #[test]
    fn test_builtin_path_join_simple() {
        let r = path(&["join", "/usr", "local", "bin"]);
        assert_eq!(stdout(&r), "/usr/local/bin\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_join_absolute_mid() {
        // PathBuf::push replaces on absolute component
        let r = path(&["join", "/a", "/b"]);
        assert_eq!(stdout(&r), "/b\n");
    }

    #[test]
    fn test_builtin_path_join_relative() {
        let r = path(&["join", "a", "b", "c"]);
        assert_eq!(stdout(&r), "a/b/c\n");
    }

    #[test]
    fn test_builtin_path_join_no_args_fails() {
        let r = path(&["join"]);
        assert_eq!(r.exit_code, 1);
    }

    // --- is ---

    #[test]
    fn test_builtin_path_is_exists_real_dir() {
        let r = path(&["is", "-d", "/tmp"]);
        assert_eq!(r.exit_code, 0);
        assert!(stdout(&r).contains("/tmp"));
    }

    #[test]
    fn test_builtin_path_is_nonexistent() {
        let r = path(&["is", "-e", "/nonexistent/path/that/does/not/exist"]);
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn test_builtin_path_is_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"test").unwrap();
        f.flush().unwrap();
        let p = f.path().to_str().unwrap().to_string();
        let r = path(&["is", "-f", &p]);
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_is_not_dir_when_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"test").unwrap();
        f.flush().unwrap();
        let p = f.path().to_str().unwrap().to_string();
        let r = path(&["is", "-d", &p]);
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn test_builtin_path_is_multiple_all_match() {
        let r = path(&["is", "-d", "/tmp", "/"]);
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_is_multiple_partial_match() {
        let r = path(&["is", "-d", "/tmp", "/nonexistent"]);
        // One fails → exit 1, but matching paths are still printed
        assert_eq!(r.exit_code, 1);
        assert!(stdout(&r).contains("/tmp"));
    }

    // --- change-extension ---

    #[test]
    fn test_builtin_path_change_extension_replace() {
        let r = path(&["change-extension", "rs", "file.txt"]);
        assert_eq!(stdout(&r), "file.rs\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_change_extension_strip() {
        let r = path(&["change-extension", "", "file.txt"]);
        assert_eq!(stdout(&r), "file\n");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_builtin_path_change_extension_with_dot_prefix() {
        let r = path(&["change-extension", ".toml", "Cargo.lock"]);
        assert_eq!(stdout(&r), "Cargo.toml\n");
    }

    // --- no subcommand / unknown ---

    #[test]
    fn test_builtin_path_no_subcommand() {
        let r = path(&[]);
        assert_eq!(r.exit_code, 1);
        assert!(r.stderr.contains("Usage:"));
    }

    #[test]
    fn test_builtin_path_unknown_subcommand() {
        let r = path(&["frobnicate"]);
        assert_eq!(r.exit_code, 1);
        assert!(r.stderr.contains("unknown subcommand"));
    }

    // --- resolve (only test with real paths) ---

    #[test]
    fn test_builtin_path_resolve_tmp() {
        // /tmp should resolve to something (may be /private/tmp on macOS)
        let r = path(&["resolve", "/tmp"]);
        assert_eq!(r.exit_code, 0);
        assert!(!stdout(&r).is_empty());
    }

    #[test]
    fn test_builtin_path_resolve_nonexistent() {
        let r = path(&["resolve", "/nonexistent/path/that/does/not/exist"]);
        assert_eq!(r.exit_code, 1);
        assert!(r.stderr.contains("path resolve:"));
    }
}
