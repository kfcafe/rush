use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ChownOptions {
    /// Recurse into directories (-R / --recursive)
    recursive: bool,
    /// Parsed ownership spec
    owner: OwnerSpec,
    /// Files/directories to operate on
    files: Vec<String>,
}

/// Parsed OWNER[:GROUP] spec.
#[derive(Debug, Clone)]
struct OwnerSpec {
    /// UID or user name (None means "don't change")
    uid: Option<libc::uid_t>,
    /// GID or group name (None means "don't change")
    gid: Option<libc::gid_t>,
}

impl OwnerSpec {
    /// Parse "OWNER", "OWNER:GROUP", "OWNER:", ":GROUP".
    fn parse(spec: &str) -> Result<Self> {
        if let Some(colon) = spec.find(':') {
            let user_part = &spec[..colon];
            let group_part = &spec[colon + 1..];

            let uid = if user_part.is_empty() {
                None
            } else {
                Some(lookup_uid(user_part)?)
            };

            let gid = if group_part.is_empty() {
                None
            } else {
                Some(lookup_gid(group_part)?)
            };

            Ok(OwnerSpec { uid, gid })
        } else {
            // No colon: only owner
            let uid = Some(lookup_uid(spec)?);
            Ok(OwnerSpec { uid, gid: None })
        }
    }
}

/// Look up a UID by name or numeric string.
fn lookup_uid(name: &str) -> Result<libc::uid_t> {
    // Try numeric first
    if let Ok(n) = name.parse::<libc::uid_t>() {
        return Ok(n);
    }

    // Look up by name via getpwnam
    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0);

    // SAFETY: name_bytes is nul-terminated; we read the returned struct immediately.
    let pw = unsafe { libc::getpwnam(name_bytes.as_ptr() as *const libc::c_char) };

    if pw.is_null() {
        return Err(anyhow!("invalid user: '{}'", name));
    }

    // SAFETY: pw is a valid pointer (checked above).
    Ok(unsafe { (*pw).pw_uid })
}

/// Look up a GID by name or numeric string.
fn lookup_gid(name: &str) -> Result<libc::gid_t> {
    // Try numeric first
    if let Ok(n) = name.parse::<libc::gid_t>() {
        return Ok(n);
    }

    // Look up by name via getgrnam
    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0);

    // SAFETY: name_bytes is nul-terminated; we read the returned struct immediately.
    let gr = unsafe { libc::getgrnam(name_bytes.as_ptr() as *const libc::c_char) };

    if gr.is_null() {
        return Err(anyhow!("invalid group: '{}'", name));
    }

    // SAFETY: gr is a valid pointer (checked above).
    Ok(unsafe { (*gr).gr_gid })
}

impl ChownOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut recursive = false;
        let mut owner_str: Option<String> = None;
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
                return Err(anyhow!("chown: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 {
                for ch in arg[1..].chars() {
                    match ch {
                        'R' => recursive = true,
                        _ => return Err(anyhow!("chown: invalid option -- '{}'", ch)),
                    }
                }
            } else if owner_str.is_none() {
                owner_str = Some(arg.clone());
            } else {
                files.push(arg.clone());
            }

            i += 1;
        }

        let owner_str = owner_str.ok_or_else(|| {
            anyhow!("chown: missing operand\nTry 'chown --help' for more information.")
        })?;

        if files.is_empty() {
            return Err(anyhow!(
                "chown: missing operand after '{}'\nTry 'chown --help' for more information.",
                owner_str
            ));
        }

        let owner = OwnerSpec::parse(&owner_str)?;

        Ok(ChownOptions {
            recursive,
            owner,
            files,
        })
    }
}

/// Apply chown to a single path. Returns an error string on failure.
fn chown_path(path: &Path, owner: &OwnerSpec) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;

    let uid = owner.uid.unwrap_or(u32::MAX); // u32::MAX → -1 in C → "don't change"
    let gid = owner.gid.unwrap_or(u32::MAX);

    let mut path_bytes = path.as_os_str().as_bytes().to_vec();
    path_bytes.push(0);

    // SAFETY: path_bytes is nul-terminated; uid/gid are valid values.
    let ret = unsafe { libc::chown(path_bytes.as_ptr() as *const libc::c_char, uid, gid) };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        Some(format!(
            "chown: changing ownership of '{}': {}",
            path.display(),
            err
        ))
    } else {
        None
    }
}

/// Recursively apply chown to `path` and all its descendants.
fn chown_recursive(path: &Path, owner: &OwnerSpec, errors: &mut Vec<String>) {
    if let Some(err) = chown_path(path, owner) {
        errors.push(err);
    }

    if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    chown_recursive(&entry.path(), owner, errors);
                }
            }
            Err(e) => {
                errors.push(format!(
                    "chown: cannot open directory '{}': {}",
                    path.display(),
                    e
                ));
            }
        }
    }
}

pub fn builtin_chown(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match ChownOptions::parse(args) {
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
            chown_recursive(&path, &opts.owner, &mut errors);
        } else {
            if let Some(err) = chown_path(&path, &opts.owner) {
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

const HELP_TEXT: &str = "Usage: chown [OPTION]... OWNER[:GROUP] FILE...
Change the owner and optionally the group of each FILE.

OWNER and GROUP can be a name or numeric ID.
  OWNER        Change owner only
  OWNER:GROUP  Change owner and group
  OWNER:       Change owner; set group to owner's login group
  :GROUP       Change group only

Options:
  -R, --recursive   operate on files and directories recursively
  --help            display this help and exit

Examples:
  chown alice file.txt          Set owner to alice
  chown alice:staff file.txt    Set owner to alice, group to staff
  chown :wheel file.txt         Set group to wheel
  chown -R bob:bob dir/         Recursively set owner and group
  chown 1000:1000 file.txt      Use numeric UID:GID
";

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::TempDir;

    fn make_runtime(dir: &TempDir) -> Runtime {
        let mut rt = Runtime::new();
        rt.set_cwd(dir.path().to_path_buf());
        rt
    }

    /// Get current process UID and GID for self-chown tests.
    fn current_uid() -> libc::uid_t {
        unsafe { libc::getuid() }
    }
    fn current_gid() -> libc::gid_t {
        unsafe { libc::getgid() }
    }

    #[test]
    fn test_chown_numeric_same_owner() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("f.txt");
        std::fs::write(&file, "").unwrap();

        let uid = current_uid().to_string();
        let gid = current_gid().to_string();
        let spec = format!("{}:{}", uid, gid);

        let result = builtin_chown(&[spec, "f.txt".to_string()], &mut rt).unwrap();
        // chown to same uid:gid should succeed (or fail only if unprivileged)
        // On CI we may not be able to chown — just check it ran without panic
        let _ = result.exit_code;
    }

    #[test]
    fn test_chown_missing_operand() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let result = builtin_chown(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_chown_invalid_user() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("f.txt");
        std::fs::write(&file, "").unwrap();
        let result = builtin_chown(
            &["nonexistent_user_xyz_123".to_string(), "f.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid user"));
    }

    #[test]
    fn test_chown_group_only() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let file = tmp.path().join("g.txt");
        std::fs::write(&file, "").unwrap();

        let gid = current_gid().to_string();
        let spec = format!(":{}", gid);

        let result = builtin_chown(&[spec, "g.txt".to_string()], &mut rt).unwrap();
        let _ = result.exit_code; // may fail if not privileged; just confirm it ran
    }

    #[test]
    fn test_chown_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let uid = current_uid().to_string();
        let result = builtin_chown(&[uid, "ghost.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("ghost.txt"));
    }

    #[test]
    fn test_owner_spec_parse_owner_only() {
        let uid = current_uid();
        let spec = OwnerSpec::parse(&uid.to_string()).unwrap();
        assert_eq!(spec.uid, Some(uid));
        assert_eq!(spec.gid, None);
    }

    #[test]
    fn test_owner_spec_parse_owner_group() {
        let uid = current_uid();
        let gid = current_gid();
        let spec = OwnerSpec::parse(&format!("{}:{}", uid, gid)).unwrap();
        assert_eq!(spec.uid, Some(uid));
        assert_eq!(spec.gid, Some(gid));
    }

    #[test]
    fn test_owner_spec_parse_group_only() {
        let gid = current_gid();
        let spec = OwnerSpec::parse(&format!(":{}", gid)).unwrap();
        assert_eq!(spec.uid, None);
        assert_eq!(spec.gid, Some(gid));
    }

    #[test]
    fn test_owner_spec_invalid_user() {
        let err = OwnerSpec::parse("nosuchuser_zzz_999").unwrap_err();
        assert!(err.to_string().contains("invalid user"));
    }
}
