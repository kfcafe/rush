use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Options for the cp command
#[derive(Debug, Default)]
struct CpOptions {
    /// Recursive copy (-r, -R, --recursive)
    recursive: bool,
    /// Preserve permissions and timestamps (-p, --preserve)
    preserve: bool,
    /// Force overwrite — silently clobber existing files (-f, --force)
    force: bool,
    /// No-clobber — skip if destination exists (-n, --no-clobber)
    no_clobber: bool,
    /// Verbose — print each file as it is copied (-v, --verbose)
    verbose: bool,
    /// Source paths (all but the last)
    sources: Vec<String>,
    /// Destination path (last argument)
    destination: String,
}

impl CpOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = CpOptions::default();
        let mut positional: Vec<String> = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                positional.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--recursive" {
                opts.recursive = true;
            } else if arg == "--preserve" {
                opts.preserve = true;
            } else if arg == "--force" {
                opts.force = true;
            } else if arg == "--no-clobber" {
                opts.no_clobber = true;
            } else if arg == "--verbose" {
                opts.verbose = true;
            } else if arg == "--help" {
                return Err(anyhow!("HELP"));
            } else if arg.starts_with("--") {
                return Err(anyhow!("cp: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        'r' | 'R' => opts.recursive = true,
                        'p' => opts.preserve = true,
                        'f' => opts.force = true,
                        'n' => opts.no_clobber = true,
                        'v' => opts.verbose = true,
                        _ => return Err(anyhow!("cp: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                positional.push(arg.clone());
            }
            i += 1;
        }

        if positional.len() < 2 {
            return Err(anyhow!(
                "cp: missing destination file operand after '{}'\nTry 'cp --help' for more information.",
                positional.first().map(|s| s.as_str()).unwrap_or("cp")
            ));
        }

        let dest = positional.pop().unwrap();
        opts.sources = positional;
        opts.destination = dest;

        Ok(opts)
    }
}

/// Resolve path against CWD, expanding leading `~`.
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

/// Copy a single file from `src` to `dest` (dest is the final file path, not a dir).
/// Handles no-clobber, force, and preserve options.
fn copy_file(
    src: &Path,
    dest: &Path,
    opts: &CpOptions,
    runtime: &mut Runtime,
    stdout: &mut String,
) -> Result<()> {
    // No-clobber: skip if destination exists
    if opts.no_clobber && dest.exists() {
        return Ok(());
    }

    // If dest exists and force is not set, we still overwrite (cp default);
    // -f has no special meaning beyond suppressing errors for cp on Linux.

    // Track destination in undo system before writing
    if dest.exists() {
        // Overwriting: back up existing file so undo can restore it
        let description = format!("cp {} -> {}", src.display(), dest.display());
        runtime
            .undo_manager_mut()
            .track_modify(dest, description)?;
    } else {
        // Creating a new file: undo will delete it
        let description = format!("cp {} -> {}", src.display(), dest.display());
        runtime
            .undo_manager_mut()
            .track_create(dest.to_path_buf(), description);
    }

    // Ensure destination parent directory exists
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::copy(src, dest).map_err(|e| anyhow!("cp: cannot copy '{}' to '{}': {}", src.display(), dest.display(), e))?;

    if opts.preserve {
        preserve_metadata(src, dest)?;
    }

    if opts.verbose {
        stdout.push_str(&format!("'{}' -> '{}'\n", src.display(), dest.display()));
    }

    Ok(())
}

/// Copy `src` directory recursively into `dest`.
/// `dest` is the final destination directory path.
fn copy_dir_recursive(
    src: &Path,
    dest: &Path,
    opts: &CpOptions,
    runtime: &mut Runtime,
    stdout: &mut String,
) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
        let description = format!("cp -r {} -> {}", src.display(), dest.display());
        runtime
            .undo_manager_mut()
            .track_create(dest.to_path_buf(), description);
    }

    if opts.verbose {
        stdout.push_str(&format!("'{}' -> '{}'\n", src.display(), dest.display()));
    }

    for entry in fs::read_dir(src)
        .map_err(|e| anyhow!("cp: cannot read directory '{}': {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| anyhow!("cp: read_dir error: {}", e))?;
        let src_child = entry.path();
        let dest_child = dest.join(entry.file_name());

        if src_child.is_dir() && !src_child.is_symlink() {
            copy_dir_recursive(&src_child, &dest_child, opts, runtime, stdout)?;
        } else {
            copy_file(&src_child, &dest_child, opts, runtime, stdout)?;
        }
    }

    if opts.preserve {
        preserve_metadata(src, dest)?;
    }

    Ok(())
}

/// Preserve file permissions and modification time from `src` on `dest`.
fn preserve_metadata(src: &Path, dest: &Path) -> Result<()> {
    let meta = fs::metadata(src)
        .map_err(|e| anyhow!("cp: cannot read metadata of '{}': {}", src.display(), e))?;

    // Permissions
    fs::set_permissions(dest, meta.permissions())
        .map_err(|e| anyhow!("cp: cannot set permissions on '{}': {}", dest.display(), e))?;

    // Timestamps via filetime crate if available — fall back to noop on failure
    // We use std::fs::File's set_times via the standard library on nightly,
    // but for stable Rust we use the libc utimensat approach already used in touch.rs.
    use std::os::unix::ffi::OsStrExt;
    use std::time::UNIX_EPOCH;

    let mtime = meta.modified().unwrap_or(std::time::SystemTime::now());
    let atime = meta.accessed().unwrap_or(std::time::SystemTime::now());

    let to_timespec = |t: std::time::SystemTime| -> libc::timespec {
        let dur = t.duration_since(UNIX_EPOCH).unwrap_or_default();
        libc::timespec {
            tv_sec: dur.as_secs() as libc::time_t,
            tv_nsec: dur.subsec_nanos() as libc::c_long,
        }
    };

    let times = [to_timespec(atime), to_timespec(mtime)];
    let mut path_bytes = dest.as_os_str().as_bytes().to_vec();
    path_bytes.push(0);

    // SAFETY: path_bytes is nul-terminated; times is a valid 2-element array
    let ret = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            path_bytes.as_ptr() as *const libc::c_char,
            times.as_ptr(),
            0,
        )
    };

    if ret != 0 {
        // Non-fatal: permissions were set, timestamp failure is acceptable
        // (e.g. cross-filesystem or permission restrictions)
    }

    Ok(())
}

pub fn builtin_cp(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match CpOptions::parse(args) {
        Ok(opts) => opts,
        Err(e) if e.to_string() == "HELP" => {
            return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
        }
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    let dest_raw = resolve_path(&opts.destination, runtime.get_cwd());

    // Multiple sources require the destination to be a directory
    if opts.sources.len() > 1 && !dest_raw.is_dir() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!(
                "cp: target '{}' is not a directory\n",
                opts.destination
            ),
            exit_code: 1,
            error: None,
        });
    }

    for src_arg in &opts.sources {
        let src = resolve_path(src_arg, runtime.get_cwd());

        if !src.exists() {
            stderr.push_str(&format!(
                "cp: cannot stat '{}': No such file or directory\n",
                src_arg
            ));
            exit_code = 1;
            continue;
        }

        if src.is_dir() && !src.is_symlink() {
            if !opts.recursive {
                stderr.push_str(&format!(
                    "cp: omitting directory '{}'\n",
                    src_arg
                ));
                exit_code = 1;
                continue;
            }

            // Determine destination directory
            let dest = if dest_raw.is_dir() {
                // cp -r src/ existing_dir/ → existing_dir/src/
                dest_raw.join(
                    src.file_name()
                        .ok_or_else(|| anyhow!("cp: invalid source path '{}'", src_arg))?,
                )
            } else {
                // cp -r src/ new_name → new_name/
                dest_raw.clone()
            };

            if let Err(e) = copy_dir_recursive(&src, &dest, &opts, runtime, &mut stdout) {
                stderr.push_str(&format!("{}\n", e));
                exit_code = 1;
            }
        } else {
            // Source is a regular file or symlink
            let dest = if dest_raw.is_dir() {
                // Copy into the directory, keeping the source filename
                dest_raw.join(
                    src.file_name()
                        .ok_or_else(|| anyhow!("cp: invalid source path '{}'", src_arg))?,
                )
            } else {
                dest_raw.clone()
            };

            if let Err(e) = copy_file(&src, &dest, &opts, runtime, &mut stdout) {
                stderr.push_str(&format!("{}\n", e));
                exit_code = 1;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(stdout),
        stderr,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: cp [OPTION]... SOURCE DEST
  or:  cp [OPTION]... SOURCE... DIRECTORY
Copy SOURCE to DEST, or multiple SOURCEs to DIRECTORY.

Options:
  -r, -R, --recursive  copy directories recursively
  -p, --preserve       preserve permissions and timestamps
  -f, --force          do not prompt before overwriting (default behavior)
  -n, --no-clobber     do not overwrite an existing file
  -v, --verbose        explain what is being done
  --help               display this help and exit

UNDO SUPPORT:
  Copied files are tracked and can be reverted with the 'undo' command.

Examples:
  cp file.txt backup.txt         Copy file to backup.txt
  cp -r src/ dest/               Recursively copy directory
  cp -p file.txt copy.txt        Copy preserving permissions/timestamps
  cp -n file.txt dest.txt        Copy only if dest.txt doesn't exist
  cp -v file1 file2 dir/         Copy multiple files verbosely into dir/
";

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn make_runtime(dir: &TempDir) -> Runtime {
        let mut rt = Runtime::new();
        rt.set_cwd(dir.path().to_path_buf());
        rt
    }

    // ---- basic file copy ----

    #[test]
    fn test_builtin_cp_single_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let src = tmp.path().join("src.txt");
        fs::write(&src, "hello").unwrap();

        let result = builtin_cp(&["src.txt".to_string(), "dest.txt".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(tmp.path().join("dest.txt").exists());
        assert_eq!(fs::read_to_string(tmp.path().join("dest.txt")).unwrap(), "hello");
        // Source remains
        assert!(src.exists());
    }

    #[test]
    fn test_builtin_cp_absolute_paths() {
        let tmp = TempDir::new().unwrap();
        let mut rt = Runtime::new();

        let src = tmp.path().join("abs_src.txt");
        let dest = tmp.path().join("abs_dest.txt");
        fs::write(&src, "data").unwrap();

        let result = builtin_cp(
            &[src.to_string_lossy().to_string(), dest.to_string_lossy().to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
    }

    #[test]
    fn test_builtin_cp_into_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let src = tmp.path().join("file.txt");
        let dir = tmp.path().join("mydir");
        fs::write(&src, "content").unwrap();
        fs::create_dir(&dir).unwrap();

        let result = builtin_cp(&["file.txt".to_string(), "mydir".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(dir.join("file.txt").exists());
        assert_eq!(fs::read_to_string(dir.join("file.txt")).unwrap(), "content");
    }

    #[test]
    fn test_builtin_cp_multiple_files_into_dir() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
        fs::write(tmp.path().join("b.txt"), "bbb").unwrap();
        let dir = tmp.path().join("out");
        fs::create_dir(&dir).unwrap();

        let result = builtin_cp(
            &["a.txt".to_string(), "b.txt".to_string(), "out".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "aaa");
        assert_eq!(fs::read_to_string(dir.join("b.txt")).unwrap(), "bbb");
    }

    #[test]
    fn test_builtin_cp_multiple_sources_no_dir_fails() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
        fs::write(tmp.path().join("b.txt"), "bbb").unwrap();

        let result = builtin_cp(
            &["a.txt".to_string(), "b.txt".to_string(), "notadir.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("not a directory"));
    }

    // ---- recursive copy ----

    #[test]
    fn test_builtin_cp_recursive_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let src_dir = tmp.path().join("src");
        let sub = src_dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(src_dir.join("root.txt"), "root").unwrap();
        fs::write(sub.join("child.txt"), "child").unwrap();

        let result = builtin_cp(&["-r".to_string(), "src".to_string(), "dest".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let dest = tmp.path().join("dest");
        assert!(dest.is_dir());
        assert_eq!(fs::read_to_string(dest.join("root.txt")).unwrap(), "root");
        assert_eq!(fs::read_to_string(dest.join("sub/child.txt")).unwrap(), "child");
    }

    #[test]
    fn test_builtin_cp_recursive_into_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let src_dir = tmp.path().join("mylib");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "code").unwrap();

        let dest_dir = tmp.path().join("backup");
        fs::create_dir(&dest_dir).unwrap();

        let result = builtin_cp(
            &["-r".to_string(), "mylib".to_string(), "backup".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        // When dest exists as dir, src is copied inside: backup/mylib/lib.rs
        assert!(dest_dir.join("mylib/lib.rs").exists());
    }

    #[test]
    fn test_builtin_cp_dir_without_recursive_fails() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::create_dir(tmp.path().join("srcdir")).unwrap();

        let result = builtin_cp(&["srcdir".to_string(), "destdir".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("omitting directory"));
    }

    // ---- no-clobber ----

    #[test]
    fn test_builtin_cp_no_clobber_skips_existing() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new").unwrap();
        fs::write(tmp.path().join("dest.txt"), "original").unwrap();

        let result = builtin_cp(
            &["-n".to_string(), "src.txt".to_string(), "dest.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        // Destination unchanged
        assert_eq!(fs::read_to_string(tmp.path().join("dest.txt")).unwrap(), "original");
    }

    #[test]
    fn test_builtin_cp_overwrites_by_default() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new content").unwrap();
        fs::write(tmp.path().join("dest.txt"), "old content").unwrap();

        let result = builtin_cp(&["src.txt".to_string(), "dest.txt".to_string()], &mut rt).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(fs::read_to_string(tmp.path().join("dest.txt")).unwrap(), "new content");
    }

    // ---- preserve ----

    #[test]
    fn test_builtin_cp_preserve_permissions() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let src = tmp.path().join("exec.sh");
        fs::write(&src, "#!/bin/sh").unwrap();
        fs::set_permissions(&src, fs::Permissions::from_mode(0o755)).unwrap();

        let result = builtin_cp(
            &["-p".to_string(), "exec.sh".to_string(), "copy.sh".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let dest = tmp.path().join("copy.sh");
        assert!(dest.exists());
        let mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    // ---- verbose ----

    #[test]
    fn test_builtin_cp_verbose() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("v.txt"), "x").unwrap();

        let result = builtin_cp(
            &["-v".to_string(), "v.txt".to_string(), "v_copy.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("v.txt"));
        assert!(result.stdout().contains("v_copy.txt"));
    }

    // ---- error cases ----

    #[test]
    fn test_builtin_cp_missing_source() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_cp(
            &["ghost.txt".to_string(), "dest.txt".to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("No such file or directory"));
    }

    #[test]
    fn test_builtin_cp_no_args() {
        let mut rt = Runtime::new();
        let result = builtin_cp(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing destination"));
    }

    #[test]
    fn test_builtin_cp_invalid_option() {
        let mut rt = Runtime::new();
        let result = builtin_cp(&["-z".to_string(), "a".to_string(), "b".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    #[test]
    fn test_builtin_cp_help() {
        let mut rt = Runtime::new();
        let result = builtin_cp(&["--help".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Usage: cp"));
    }

    // ---- undo integration ----

    #[test]
    fn test_builtin_cp_undo_new_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("orig.txt"), "original").unwrap();

        let result = builtin_cp(
            &["orig.txt".to_string(), "copy.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        let copy = tmp.path().join("copy.txt");
        assert!(copy.exists());

        // Undo should remove the copy
        let ops = rt.undo_manager_mut().list_operations(10);
        assert!(!ops.is_empty());
        assert!(ops[0].description.contains("cp"));

        rt.undo_manager_mut().undo().unwrap();
        assert!(!copy.exists());
    }

    #[test]
    fn test_builtin_cp_undo_overwrite() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        fs::write(tmp.path().join("src.txt"), "new").unwrap();
        fs::write(tmp.path().join("dest.txt"), "old").unwrap();

        let result = builtin_cp(
            &["src.txt".to_string(), "dest.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        // After cp dest.txt contains "new"
        assert_eq!(fs::read_to_string(tmp.path().join("dest.txt")).unwrap(), "new");

        // Undo should restore "old"
        rt.undo_manager_mut().undo().unwrap();
        assert_eq!(fs::read_to_string(tmp.path().join("dest.txt")).unwrap(), "old");
    }
}
