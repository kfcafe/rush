use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ChmodOptions {
    /// Recurse into directories (-R / --recursive)
    recursive: bool,
    /// Mode string (octal or symbolic)
    mode: String,
    /// Files/directories to operate on
    files: Vec<String>,
}

impl ChmodOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut recursive = false;
        let mut mode: Option<String> = None;
        let mut files: Vec<String> = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg == "--" {
                files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--recursive" {
                recursive = true;
            } else if arg == "--help" {
                return Err(anyhow!("HELP"));
            } else if arg.starts_with("--") {
                return Err(anyhow!("chmod: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 {
                for ch in arg[1..].chars() {
                    match ch {
                        'R' => recursive = true,
                        _ => return Err(anyhow!("chmod: invalid option -- '{}'", ch)),
                    }
                }
            } else if mode.is_none() {
                mode = Some(arg.clone());
            } else {
                files.push(arg.clone());
            }

            i += 1;
        }

        let mode = mode.ok_or_else(|| {
            anyhow!("chmod: missing operand\nTry 'chmod --help' for more information.")
        })?;

        if files.is_empty() {
            return Err(anyhow!(
                "chmod: missing operand after '{}'\nTry 'chmod --help' for more information.",
                mode
            ));
        }

        Ok(ChmodOptions {
            recursive,
            mode,
            files,
        })
    }
}

/// Apply chmod to a single path. Returns an error string on failure (non-fatal).
fn chmod_path(path: &Path, mode_str: &str) -> Option<String> {
    // Determine the current mode so symbolic specs can be applied relative to it.
    let current_mode = match std::fs::metadata(path) {
        Ok(m) => {
            use std::os::unix::fs::MetadataExt;
            m.mode()
        }
        Err(e) => {
            return Some(format!("chmod: cannot access '{}': {}", path.display(), e));
        }
    };

    let new_mode = match parse_mode(mode_str, current_mode) {
        Ok(m) => m,
        Err(e) => {
            return Some(format!("chmod: invalid mode '{}': {}", mode_str, e));
        }
    };

    use std::os::unix::ffi::OsStrExt;
    let mut path_bytes = path.as_os_str().as_bytes().to_vec();
    path_bytes.push(0);

    // SAFETY: path_bytes is nul-terminated; new_mode is a valid mode_t value.
    let ret = unsafe {
        libc::chmod(
            path_bytes.as_ptr() as *const libc::c_char,
            new_mode as libc::mode_t,
        )
    };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        Some(format!(
            "chmod: changing permissions of '{}': {}",
            path.display(),
            err
        ))
    } else {
        None
    }
}

/// Parse a mode string (octal or symbolic) and return the resulting mode bits.
///
/// `current` is the file's current mode (used for symbolic specs like `u+x`).
fn parse_mode(mode_str: &str, current: u32) -> Result<u32> {
    // Octal: only digits 0-7
    if mode_str.chars().all(|c| matches!(c, '0'..='7')) {
        let octal = u32::from_str_radix(mode_str, 8)
            .map_err(|_| anyhow!("invalid octal '{}'", mode_str))?;
        // Clamp to 12 bits (rwx × 3 + suid/sgid/sticky)
        return Ok(octal & 0o7777);
    }

    // Symbolic: comma-separated clauses like `u+x`, `g-w`, `a=r`, `ug+rw`
    let mut mode = current & 0o7777;

    for clause in mode_str.split(',') {
        mode = apply_symbolic_clause(clause, mode)?;
    }

    Ok(mode)
}

/// Apply one symbolic clause (e.g. `u+x`, `go-w`, `a=rx`) to `current` and return
/// the updated mode.
fn apply_symbolic_clause(clause: &str, current: u32) -> Result<u32> {
    if clause.is_empty() {
        return Err(anyhow!("empty clause"));
    }

    let bytes = clause.as_bytes();
    let mut idx = 0;

    // Parse optional who-set: [ugoa]*
    // If absent, treat as 'a' (all), but apply umask like POSIX (we skip umask here).
    let mut who_bits: u32 = 0;
    let mut explicit_who = false;

    while idx < bytes.len() {
        match bytes[idx] {
            b'u' => {
                who_bits |= 0o700;
                idx += 1;
                explicit_who = true;
            }
            b'g' => {
                who_bits |= 0o070;
                idx += 1;
                explicit_who = true;
            }
            b'o' => {
                who_bits |= 0o007;
                idx += 1;
                explicit_who = true;
            }
            b'a' => {
                who_bits |= 0o777;
                idx += 1;
                explicit_who = true;
            }
            _ => break,
        }
    }

    if !explicit_who {
        who_bits = 0o777; // default: all
    }

    if idx >= bytes.len() {
        return Err(anyhow!("missing operator in '{}'", clause));
    }

    // Parse operator: +, -, =
    let op = bytes[idx];
    if op != b'+' && op != b'-' && op != b'=' {
        return Err(anyhow!(
            "invalid operator '{}' in '{}'",
            bytes[idx] as char,
            clause
        ));
    }
    idx += 1;

    // Parse permission bits: [rwxXst]*
    let mut perm_bits: u32 = 0;
    let mut has_X = false; // capital X is conditional execute

    while idx < bytes.len() {
        match bytes[idx] {
            b'r' => {
                perm_bits |= 0o444;
                idx += 1;
            }
            b'w' => {
                perm_bits |= 0o222;
                idx += 1;
            }
            b'x' => {
                perm_bits |= 0o111;
                idx += 1;
            }
            b'X' => {
                has_X = true;
                idx += 1;
            }
            b's' => {
                // setuid or setgid
                // s applies to u or g bits (4000 and 2000)
                if who_bits & 0o700 != 0 {
                    perm_bits |= 0o4000;
                }
                if who_bits & 0o070 != 0 {
                    perm_bits |= 0o2000;
                }
                idx += 1;
            }
            b't' => {
                perm_bits |= 0o1000;
                idx += 1;
            } // sticky
            _ => {
                return Err(anyhow!(
                    "invalid permission char '{}' in '{}'",
                    bytes[idx] as char,
                    clause
                ))
            }
        }
    }

    // Capital X: add execute only if it's a directory or already has execute for anyone
    if has_X {
        let already_exec = current & 0o111 != 0;
        // We can't check if it's a directory here without metadata, so callers
        // handle the is-directory case. We rely on the file type from the mode:
        // S_IFDIR = 0o40000
        let is_dir = (current & 0o170000) == 0o040000;
        if already_exec || is_dir {
            perm_bits |= 0o111;
        }
    }

    // Mask perm_bits by who_bits (for rwx bits only; suid/sgid/sticky handled above)
    let rwx_bits = perm_bits & 0o777;
    let special_bits = perm_bits & 0o7000;

    let masked_rwx = rwx_bits & who_bits;

    let mut new_mode = current;
    match op {
        b'+' => {
            new_mode |= masked_rwx;
            new_mode |= special_bits;
        }
        b'-' => {
            new_mode &= !masked_rwx;
            new_mode &= !special_bits;
        }
        b'=' => {
            // For = operator: clear the who bits then set perm_bits
            new_mode &= !who_bits;
            // Also clear special bits that apply to this who
            if who_bits & 0o700 != 0 {
                new_mode &= !0o4000;
            }
            if who_bits & 0o070 != 0 {
                new_mode &= !0o2000;
            }
            if who_bits & 0o777 == 0o777 {
                new_mode &= !0o1000;
            }
            new_mode |= masked_rwx;
            new_mode |= special_bits;
        }
        _ => unreachable!(),
    }

    Ok(new_mode)
}

/// Recursively apply chmod to `path` and all its descendants.
fn chmod_recursive(path: &Path, mode_str: &str, errors: &mut Vec<String>) {
    if let Some(err) = chmod_path(path, mode_str) {
        errors.push(err);
    }

    if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    chmod_recursive(&entry.path(), mode_str, errors);
                }
            }
            Err(e) => {
                errors.push(format!(
                    "chmod: cannot open directory '{}': {}",
                    path.display(),
                    e
                ));
            }
        }
    }
}

pub fn builtin_chmod(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match ChmodOptions::parse(args) {
        Ok(o) => o,
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

    let cwd = runtime.get_cwd().to_path_buf();
    let mut errors: Vec<String> = Vec::new();

    for file_arg in &opts.files {
        let path = resolve_path(file_arg, &cwd);

        if opts.recursive {
            chmod_recursive(&path, &opts.mode, &mut errors);
        } else {
            if let Some(err) = chmod_path(&path, &opts.mode) {
                errors.push(err);
            }
        }
    }

    let exit_code = if errors.is_empty() { 0 } else { 1 };
    let stderr = errors
        .into_iter()
        .map(|e| format!("{}\n", e))
        .collect::<String>();

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr,
        exit_code,
        error: None,
    })
}

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

const HELP_TEXT: &str = "Usage: chmod [OPTION]... MODE FILE...
Change the file mode bits of each FILE to MODE.

MODE can be:
  Octal:    755, 644, 777, etc.
  Symbolic: [ugoa][+-=][rwxXst],...

  Who:      u=user/owner, g=group, o=other, a=all
  Op:       + add, - remove, = set exactly
  Perms:    r=read, w=write, x=execute, X=exec if dir/already exec,
            s=setuid/setgid, t=sticky

Options:
  -R, --recursive   change files and directories recursively
  --help            display this help and exit

Examples:
  chmod 755 file        Set rwxr-xr-x
  chmod u+x script.sh   Add execute for owner
  chmod go-w file       Remove write for group and other
  chmod -R 644 dir/     Set all files in dir to 644
  chmod a=r file        Set read-only for all
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

    #[test]
    fn test_chmod_octal() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("f.txt");
        std::fs::write(&file, "").unwrap();

        let result = builtin_chmod(&["644".to_string(), "f.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[test]
    fn test_chmod_symbolic_add_exec() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("script.sh");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        let result = builtin_chmod(&["u+x".to_string(), "script.sh".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o744);
    }

    #[test]
    fn test_chmod_symbolic_remove_write() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("ro.txt");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        let result = builtin_chmod(&["go-w".to_string(), "ro.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644); // already no group/other write; should stay 644
    }

    #[test]
    fn test_chmod_symbolic_equals() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("eq.txt");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = builtin_chmod(&["a=r".to_string(), "eq.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o444);
    }

    #[test]
    fn test_chmod_recursive() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let dir = tmp.path().join("subdir");
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("inner.txt");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        let result = builtin_chmod(
            &["-R".to_string(), "755".to_string(), "subdir".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn test_chmod_missing_operand() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let result = builtin_chmod(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_chmod_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let result = builtin_chmod(&["755".to_string(), "ghost.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("ghost.txt"));
    }

    #[test]
    fn test_parse_mode_octal() {
        assert_eq!(parse_mode("755", 0).unwrap(), 0o755);
        assert_eq!(parse_mode("644", 0).unwrap(), 0o644);
        assert_eq!(parse_mode("000", 0).unwrap(), 0o000);
    }

    #[test]
    fn test_parse_mode_symbolic_add() {
        // start with 644, add execute for user → 744
        let m = parse_mode("u+x", 0o644).unwrap();
        assert_eq!(m, 0o744);
    }

    #[test]
    fn test_parse_mode_symbolic_remove() {
        // start with 755, remove write for group and other → 755 & ~022 = 755
        // wait: 755 & ~022 = 755 & 755 = 755. Let me use 766 → 644
        let m = parse_mode("go-w", 0o766).unwrap();
        assert_eq!(m, 0o744);
    }

    #[test]
    fn test_parse_mode_symbolic_equals() {
        let m = parse_mode("a=r", 0o755).unwrap();
        assert_eq!(m, 0o444);
    }

    #[test]
    fn test_parse_mode_comma_separated() {
        // u=rw,g=r,o= → 640
        let m = parse_mode("u=rw,g=r,o=", 0o755).unwrap();
        assert_eq!(m, 0o640);
    }
}
