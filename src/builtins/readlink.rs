use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::path::{Path, PathBuf};

struct ReadlinkOptions {
    /// -f / --canonicalize: canonicalize by following every symlink in every
    /// component of the given path recursively; all but the last component
    /// must exist.
    canonicalize: bool,
    /// -e / --canonicalize-existing: like -f, but all components must exist.
    canonicalize_existing: bool,
    /// -m / --canonicalize-missing: canonicalize even if components are missing.
    canonicalize_missing: bool,
    /// -n / --no-newline: do not output trailing newline.
    no_newline: bool,
    /// -q / --quiet / -s / --silent: suppress error messages.
    quiet: bool,
    files: Vec<String>,
}

impl ReadlinkOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = ReadlinkOptions {
            canonicalize: false,
            canonicalize_existing: false,
            canonicalize_missing: false,
            no_newline: false,
            quiet: false,
            files: vec![],
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--canonicalize" {
                opts.canonicalize = true;
            } else if arg == "--canonicalize-existing" {
                opts.canonicalize_existing = true;
            } else if arg == "--canonicalize-missing" {
                opts.canonicalize_missing = true;
            } else if arg == "--no-newline" {
                opts.no_newline = true;
            } else if arg == "--quiet" || arg == "--silent" {
                opts.quiet = true;
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        'f' => opts.canonicalize = true,
                        'e' => opts.canonicalize_existing = true,
                        'm' => opts.canonicalize_missing = true,
                        'n' => opts.no_newline = true,
                        'q' | 's' => opts.quiet = true,
                        _ => return Err(format!("readlink: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }
            i += 1;
        }
        Ok(opts)
    }
}

fn resolve_path(path_str: &str, cwd: &Path) -> PathBuf {
    let p = if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(path_str.trim_start_matches("~/"))
        } else {
            PathBuf::from(path_str)
        }
    } else {
        PathBuf::from(path_str)
    };
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}

pub fn builtin_readlink(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "readlink: missing operand\nTry 'readlink --help' for more information.\n"
                .to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let opts = match ReadlinkOptions::parse(args) {
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

    if opts.files.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "readlink: missing operand\nTry 'readlink --help' for more information.\n"
                .to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let cwd = runtime.get_cwd().clone();
    let canonicalize = opts.canonicalize || opts.canonicalize_existing || opts.canonicalize_missing;
    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    for (i, file_arg) in opts.files.iter().enumerate() {
        let path = resolve_path(file_arg, &cwd);
        let is_last = i == opts.files.len() - 1;

        let resolved = if canonicalize {
            // Follow every symlink component
            match path.canonicalize() {
                Ok(p) => Some(p.to_string_lossy().to_string()),
                Err(e) => {
                    if !opts.quiet {
                        stderr_output.push_str(&format!("readlink: {}: {}\n", file_arg, e));
                    }
                    exit_code = 1;
                    None
                }
            }
        } else {
            // Read the immediate symlink target only
            match std::fs::read_link(&path) {
                Ok(target) => Some(target.to_string_lossy().to_string()),
                Err(e) => {
                    if !opts.quiet {
                        stderr_output.push_str(&format!("readlink: {}: {}\n", file_arg, e));
                    }
                    exit_code = 1;
                    None
                }
            }
        };

        if let Some(target) = resolved {
            if opts.no_newline && is_last {
                output.push_str(&target);
            } else {
                output.push_str(&target);
                output.push('\n');
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: readlink [OPTION]... FILE...
Print value of a symbolic link or canonical file name.

Options:
  -f, --canonicalize            canonicalize by following every symlink;
                                all but the last component must exist
  -e, --canonicalize-existing   canonicalize; all components must exist
  -m, --canonicalize-missing    canonicalize even if components missing
  -n, --no-newline              do not output trailing newline
  -q, --quiet, -s, --silent     suppress most error messages
  --help                        display this help and exit

Examples:
  readlink /path/to/symlink     print where symlink points
  readlink -f ./relative/path   resolve to absolute canonical path
";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_runtime(dir: &TempDir) -> Runtime {
        let mut rt = Runtime::new();
        rt.set_cwd(dir.path().to_path_buf());
        rt
    }

    #[test]
    fn test_readlink_symlink() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, "data").unwrap();
        std::os::unix::fs::symlink(&target, tmp.path().join("link.txt")).unwrap();

        let result = builtin_readlink(&["link.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().trim() == target.to_string_lossy());
    }

    #[test]
    fn test_readlink_not_a_symlink_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("real.txt"), "data").unwrap();

        let result = builtin_readlink(&["real.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("readlink:"));
    }

    #[test]
    fn test_readlink_canonicalize() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, "data").unwrap();
        std::os::unix::fs::symlink(&target, tmp.path().join("link.txt")).unwrap();

        let result =
            builtin_readlink(&["-f".to_string(), "link.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        // Should return the canonical absolute path to target.txt
        assert!(result.stdout().trim().ends_with("target.txt"));
    }

    #[test]
    fn test_readlink_no_args_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_readlink(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_readlink_quiet_suppresses_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result =
            builtin_readlink(&["-q".to_string(), "nonexistent.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        // Quiet mode: no stderr output
        assert!(
            result.stderr.is_empty(),
            "expected no stderr, got: {}",
            result.stderr
        );
    }
}
