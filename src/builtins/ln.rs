use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

struct LnOptions {
    symbolic: bool,
    force: bool,
    /// -n / --no-dereference: treat LINK_NAME as a normal file if it is a
    /// symlink to a directory (don't follow it when deciding to use it as dir).
    no_deref: bool,
    files: Vec<String>,
}

impl LnOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = LnOptions {
            symbolic: false,
            force: false,
            no_deref: false,
            files: vec![],
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--symbolic" {
                opts.symbolic = true;
            } else if arg == "--force" {
                opts.force = true;
            } else if arg == "--no-dereference" {
                opts.no_deref = true;
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        's' => opts.symbolic = true,
                        'f' => opts.force = true,
                        'n' => opts.no_deref = true,
                        _ => return Err(anyhow!("ln: invalid option -- '{}'", ch)),
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

pub fn builtin_ln(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "ln: missing file operand\nTry 'ln --help' for more information.\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let opts = match LnOptions::parse(args) {
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

    if opts.files.len() < 2 {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "ln: missing destination file operand after source\nTry 'ln --help' for more information.\n"
                .to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let cwd = runtime.get_cwd().clone();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    let dest_raw = opts.files.last().unwrap();
    let dest = resolve_path(dest_raw, &cwd);

    let sources: Vec<PathBuf> = opts.files[..opts.files.len() - 1]
        .iter()
        .map(|s| resolve_path(s, &cwd))
        .collect();

    // When -n is set, a symlink-to-dir destination is treated as a file, not a dir.
    let dest_is_dir = if opts.no_deref {
        dest.is_dir() && !dest.is_symlink()
    } else {
        dest.is_dir()
    };

    if sources.len() > 1 && !dest_is_dir {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("ln: target '{}': Not a directory\n", dest_raw),
            exit_code: 1,
            error: None,
        });
    }

    for src in &sources {
        let link_path = if dest_is_dir {
            let name = src.file_name().unwrap_or_default();
            dest.join(name)
        } else {
            dest.clone()
        };

        if opts.force && (link_path.exists() || link_path.is_symlink()) {
            if let Err(e) = std::fs::remove_file(&link_path) {
                stderr_output.push_str(&format!(
                    "ln: cannot remove '{}': {}\n",
                    link_path.display(),
                    e
                ));
                exit_code = 1;
                continue;
            }
        }

        let result = if opts.symbolic {
            std::os::unix::fs::symlink(src, &link_path)
        } else {
            std::fs::hard_link(src, &link_path)
        };

        if let Err(e) = result {
            let kind = if opts.symbolic {
                "symbolic link"
            } else {
                "hard link"
            };
            stderr_output.push_str(&format!(
                "ln: failed to create {} '{}' -> '{}': {}\n",
                kind,
                link_path.display(),
                src.display(),
                e
            ));
            exit_code = 1;
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: ln [OPTION]... [-T] TARGET LINK_NAME
   or: ln [OPTION]... TARGET
   or: ln [OPTION]... TARGET... DIRECTORY
Create hard links or symbolic links between files.

Options:
  -s, --symbolic       make symbolic links instead of hard links
  -f, --force          remove existing destination files
  -n, --no-dereference treat LINK_NAME as a normal file if it is a
                       symlink to a directory
  --help               display this help and exit

Examples:
  ln -s /path/to/file link        create a symbolic link
  ln file1 file2                  create a hard link
  ln -sf target existing_link     replace an existing symlink
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
    fn test_ln_hard_link() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("source.txt"), "hello").unwrap();

        let result =
            builtin_ln(&["source.txt".to_string(), "link.txt".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(tmp.path().join("link.txt").exists());
    }

    #[test]
    fn test_ln_symbolic_link() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("target.txt"), "data").unwrap();

        let result = builtin_ln(
            &[
                "-s".to_string(),
                "target.txt".to_string(),
                "symlink.txt".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(tmp.path().join("symlink.txt").is_symlink());
    }

    #[test]
    fn test_ln_force_replaces_existing() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("target.txt"), "data").unwrap();
        std::fs::write(tmp.path().join("existing.txt"), "old").unwrap();

        let result = builtin_ln(
            &[
                "-sf".to_string(),
                "target.txt".to_string(),
                "existing.txt".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
    }

    #[test]
    fn test_ln_missing_dest_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_ln(&["only_one".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_ln_no_args_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_ln(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing file operand"));
    }
}
