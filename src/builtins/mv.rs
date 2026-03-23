use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Options for the mv command
#[derive(Debug, Default)]
struct MvOptions {
    /// Don't overwrite existing destination (-n, --no-clobber)
    no_clobber: bool,
    /// Force overwrite without prompting (-f, --force)
    force: bool,
    /// Explain what is being done (-v, --verbose)
    verbose: bool,
    /// All operands (sources + destination as last element)
    paths: Vec<String>,
}

impl MvOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = MvOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.paths.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--no-clobber" {
                opts.no_clobber = true;
            } else if arg == "--force" {
                opts.force = true;
            } else if arg == "--verbose" {
                opts.verbose = true;
            } else if arg == "--help" {
                return Err(anyhow!("HELP"));
            } else if arg.starts_with("--") {
                return Err(anyhow!("mv: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        'n' => opts.no_clobber = true,
                        'f' => opts.force = true,
                        'v' => opts.verbose = true,
                        _ => return Err(anyhow!("mv: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.paths.push(arg.clone());
            }
            i += 1;
        }

        // -f overrides -n (last one wins, but we follow standard: -f clears -n)
        if opts.force {
            opts.no_clobber = false;
        }

        Ok(opts)
    }
}

/// Resolve a path string to an absolute PathBuf, expanding leading `~`.
fn resolve_path(path_str: &str, cwd: &Path) -> PathBuf {
    let path = if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(path_str.trim_start_matches("~/"))
        } else {
            PathBuf::from(path_str)
        }
    } else {
        PathBuf::from(path_str)
    };

    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

/// Copy a file or directory tree recursively, then delete the source.
/// Used as a cross-device fallback when `fs::rename` returns EXDEV.
fn copy_and_delete(src: &Path, dst: &Path) -> Result<()> {
    if src.is_symlink() {
        // Recreate symlink at destination
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dst)?;
        fs::remove_file(src)?;
    } else if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_and_delete(&entry.path(), &dst.join(entry.file_name()))?;
        }
        fs::remove_dir(src)?;
    } else {
        fs::copy(src, dst)?;
        fs::remove_file(src)?;
    }
    Ok(())
}

/// Attempt `fs::rename`; on cross-device failure (EXDEV) fall back to copy+delete.
fn move_path(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => copy_and_delete(src, dst),
        Err(e) => Err(e.into()),
    }
}

pub fn builtin_mv(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match MvOptions::parse(args) {
        Ok(opts) => opts,
        Err(e) if e.to_string() == "HELP" => {
            return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
        }
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\nTry 'mv --help' for more information.\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    // Need at least SOURCE and DEST
    if opts.paths.len() < 2 {
        let msg = if opts.paths.is_empty() {
            "mv: missing file operand\nTry 'mv --help' for more information.\n".to_string()
        } else {
            format!(
                "mv: missing destination file operand after '{}'\nTry 'mv --help' for more information.\n",
                opts.paths[0]
            )
        };
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: msg,
            exit_code: 1,
            error: None,
        });
    }

    let cwd = runtime.get_cwd().to_path_buf();
    let mut stderr_output = String::new();
    let mut stdout_output = String::new();
    let mut exit_code = 0;

    // Resolve all paths up front
    let resolved: Vec<PathBuf> = opts
        .paths
        .iter()
        .map(|p| resolve_path(p, &cwd))
        .collect();

    // Last path is the destination; everything before it is sources
    let (dest_resolved, srcs_resolved) = resolved.split_last().unwrap();
    let dest_original = opts.paths.last().unwrap();
    let srcs_original = &opts.paths[..opts.paths.len() - 1];

    // Multiple sources require destination to be a directory (existing or to be created isn't
    // our job — we just error if it's a non-directory file).
    if srcs_resolved.len() > 1 && dest_resolved.exists() && !dest_resolved.is_dir() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("mv: target '{}': Not a directory\n", dest_original),
            exit_code: 1,
            error: None,
        });
    }

    for (i, src) in srcs_resolved.iter().enumerate() {
        let original_src = &srcs_original[i];

        // Source must exist (or be a dangling symlink, which is_symlink() catches)
        if !src.exists() && !src.is_symlink() {
            stderr_output.push_str(&format!(
                "mv: cannot stat '{}': No such file or directory\n",
                original_src
            ));
            exit_code = 1;
            continue;
        }

        // Compute the real destination path
        let actual_dest = if dest_resolved.is_dir() {
            let file_name = src
                .file_name()
                .ok_or_else(|| anyhow!("mv: cannot determine file name for '{}'", original_src))?;
            dest_resolved.join(file_name)
        } else {
            dest_resolved.clone()
        };

        // -n: silently skip if destination already exists
        if opts.no_clobber && actual_dest.exists() {
            continue;
        }

        // Record the move in the undo system before performing it
        let desc = format!("mv {} {}", src.display(), actual_dest.display());
        runtime
            .undo_manager_mut()
            .track_move(src.clone(), actual_dest.clone(), desc);

        match move_path(src, &actual_dest) {
            Ok(()) => {
                if opts.verbose {
                    stdout_output.push_str(&format!(
                        "renamed '{}' -> '{}'\n",
                        original_src,
                        actual_dest.display()
                    ));
                }
            }
            Err(e) => {
                stderr_output.push_str(&format!(
                    "mv: cannot move '{}' to '{}': {}\n",
                    original_src,
                    actual_dest.display(),
                    e
                ));
                exit_code = 1;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(stdout_output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: mv [OPTION]... SOURCE DEST
  or:  mv [OPTION]... SOURCE... DIRECTORY
Rename SOURCE to DEST, or move SOURCE(s) to DIRECTORY.

Options:
  -f, --force        do not prompt before overwriting (default behavior)
  -n, --no-clobber   do not overwrite an existing file
  -v, --verbose      explain what is being done
  --help             display this help and exit

UNDO SUPPORT:
  Moves are tracked and can be reversed with the 'undo' command.
  Use 'undo list' to see recent operations.

Examples:
  mv file.txt newname.txt     Rename a file
  mv file.txt /tmp/           Move file into /tmp/
  mv -n src dst               Move only if dst does not exist
  mv -v a b c dir/            Move multiple files into dir verbosely
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

    // ---- basic rename ----

    #[test]
    fn test_builtin_mv_rename_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "hello").unwrap();

        let result = builtin_mv(
            &["src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("src.txt").exists());
        assert!(tmp.path().join("dst.txt").exists());
        assert_eq!(fs::read_to_string(tmp.path().join("dst.txt")).unwrap(), "hello");
    }

    // ---- move into directory ----

    #[test]
    fn test_builtin_mv_into_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("file.txt"), "data").unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();

        let result = builtin_mv(
            &["file.txt".to_string(), "subdir".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("file.txt").exists());
        assert!(tmp.path().join("subdir/file.txt").exists());
    }

    // ---- move multiple sources into directory ----

    #[test]
    fn test_builtin_mv_multiple_sources() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("a.txt"), "a").unwrap();
        fs::write(tmp.path().join("b.txt"), "b").unwrap();
        fs::write(tmp.path().join("c.txt"), "c").unwrap();
        fs::create_dir(tmp.path().join("dest")).unwrap();

        let result = builtin_mv(
            &[
                "a.txt".to_string(),
                "b.txt".to_string(),
                "c.txt".to_string(),
                "dest".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(tmp.path().join("dest/a.txt").exists());
        assert!(tmp.path().join("dest/b.txt").exists());
        assert!(tmp.path().join("dest/c.txt").exists());
    }

    // ---- -n / --no-clobber ----

    #[test]
    fn test_builtin_mv_no_clobber_skips_existing() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new").unwrap();
        fs::write(tmp.path().join("dst.txt"), "original").unwrap();

        let result = builtin_mv(
            &["-n".to_string(), "src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        // dst should be untouched
        assert_eq!(
            fs::read_to_string(tmp.path().join("dst.txt")).unwrap(),
            "original"
        );
        // src should still exist (skipped)
        assert!(tmp.path().join("src.txt").exists());
    }

    // ---- -f / --force ----

    #[test]
    fn test_builtin_mv_force_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new").unwrap();
        fs::write(tmp.path().join("dst.txt"), "old").unwrap();

        let result = builtin_mv(
            &["-f".to_string(), "src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("src.txt").exists());
        assert_eq!(
            fs::read_to_string(tmp.path().join("dst.txt")).unwrap(),
            "new"
        );
    }

    // ---- -v / --verbose ----

    #[test]
    fn test_builtin_mv_verbose() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "data").unwrap();

        let result = builtin_mv(
            &["-v".to_string(), "src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().contains("renamed"));
        assert!(result.stdout().contains("src.txt"));
    }

    // ---- error: nonexistent source ----

    #[test]
    fn test_builtin_mv_nonexistent_source() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mv(
            &["ghost.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("No such file or directory"));
    }

    // ---- error: missing operand ----

    #[test]
    fn test_builtin_mv_no_args() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mv(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing file operand"));
    }

    #[test]
    fn test_builtin_mv_single_arg() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mv(&["only.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing destination"));
    }

    // ---- error: multiple sources, dest not a directory ----

    #[test]
    fn test_builtin_mv_multiple_sources_dest_not_dir() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("a.txt"), "a").unwrap();
        fs::write(tmp.path().join("b.txt"), "b").unwrap();
        fs::write(tmp.path().join("dst.txt"), "dst").unwrap();

        let result = builtin_mv(
            &[
                "a.txt".to_string(),
                "b.txt".to_string(),
                "dst.txt".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("Not a directory"));
    }

    // ---- error: invalid option ----

    #[test]
    fn test_builtin_mv_invalid_option() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mv(
            &["-z".to_string(), "src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    // ---- help ----

    #[test]
    fn test_builtin_mv_help() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mv(&["--help".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Usage: mv"));
    }

    // ---- undo tracking ----

    #[test]
    fn test_builtin_mv_undo_tracked() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("original.txt"), "content").unwrap();

        let result = builtin_mv(
            &["original.txt".to_string(), "renamed.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        let ops = rt.undo_manager_mut().list_operations(10);
        assert!(!ops.is_empty(), "undo operation should be recorded");
        assert!(
            ops[0].description.contains("mv"),
            "description should mention mv"
        );
    }

    #[test]
    fn test_builtin_mv_undo_restores() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("original.txt"), "content").unwrap();

        // Move the file
        let result = builtin_mv(
            &["original.txt".to_string(), "renamed.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!tmp.path().join("original.txt").exists());
        assert!(tmp.path().join("renamed.txt").exists());

        // Undo should move it back
        let undo_msg = rt.undo_manager_mut().undo().unwrap();
        assert!(undo_msg.contains("Undone"));
        assert!(tmp.path().join("original.txt").exists());
        assert!(!tmp.path().join("renamed.txt").exists());
    }

    // ---- move directory ----

    #[test]
    fn test_builtin_mv_rename_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::create_dir(tmp.path().join("old_dir")).unwrap();
        fs::write(tmp.path().join("old_dir/file.txt"), "hi").unwrap();

        let result = builtin_mv(
            &["old_dir".to_string(), "new_dir".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("old_dir").exists());
        assert!(tmp.path().join("new_dir").exists());
        assert!(tmp.path().join("new_dir/file.txt").exists());
    }

    // ---- partial failure (one missing source among several) ----

    #[test]
    fn test_builtin_mv_partial_failure() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("exists.txt"), "data").unwrap();
        fs::create_dir(tmp.path().join("dest")).unwrap();

        let result = builtin_mv(
            &[
                "exists.txt".to_string(),
                "ghost.txt".to_string(),
                "dest".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        // Should partially succeed: exit_code 1 due to ghost.txt
        assert_eq!(result.exit_code, 1);
        // exists.txt should have moved
        assert!(tmp.path().join("dest/exists.txt").exists());
        // Error for ghost.txt
        assert!(result.stderr.contains("No such file or directory"));
    }

    // ---- absolute paths ----

    #[test]
    fn test_builtin_mv_absolute_paths() {
        let tmp = TempDir::new().unwrap();
        let mut rt = Runtime::new(); // no cwd set — use absolute paths directly

        let src = tmp.path().join("abs_src.txt");
        let dst = tmp.path().join("abs_dst.txt");
        fs::write(&src, "absolute").unwrap();

        let result = builtin_mv(
            &[
                src.to_string_lossy().to_string(),
                dst.to_string_lossy().to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!src.exists());
        assert!(dst.exists());
    }

    // ---- overwrite by default (no flags) ----

    #[test]
    fn test_builtin_mv_overwrites_by_default() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new content").unwrap();
        fs::write(tmp.path().join("dst.txt"), "old content").unwrap();

        let result = builtin_mv(
            &["src.txt".to_string(), "dst.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("src.txt").exists());
        assert_eq!(
            fs::read_to_string(tmp.path().join("dst.txt")).unwrap(),
            "new content"
        );
    }

    // ---- -f overrides -n ----

    #[test]
    fn test_builtin_mv_force_overrides_no_clobber() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new").unwrap();
        fs::write(tmp.path().join("dst.txt"), "old").unwrap();

        // -n then -f: force should win
        let result = builtin_mv(
            &[
                "-n".to_string(),
                "-f".to_string(),
                "src.txt".to_string(),
                "dst.txt".to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(!tmp.path().join("src.txt").exists());
        assert_eq!(
            fs::read_to_string(tmp.path().join("dst.txt")).unwrap(),
            "new"
        );
    }
}
