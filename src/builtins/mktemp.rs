use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::path::{Path, PathBuf};

struct MktempOptions {
    /// -d / --directory: create a directory, not a file.
    directory: bool,
    /// -u / --dry-run: do not create anything, just print the name.
    dry_run: bool,
    /// -q / --quiet: suppress diagnostics about file/dir creation failure.
    quiet: bool,
    /// --suffix=SUFF: append SUFF to the template.
    suffix: Option<String>,
    /// -p DIR / --tmpdir=DIR: interpret TEMPLATE relative to DIR.
    tmpdir: Option<String>,
    /// The template (positional argument). Default: tmp.XXXXXXXXXX
    template: Option<String>,
}

impl MktempOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = MktempOptions {
            directory: false,
            dry_run: false,
            quiet: false,
            suffix: None,
            tmpdir: None,
            template: None,
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                if let Some(t) = args.get(i + 1) {
                    opts.template = Some(t.clone());
                }
                break;
            } else if arg == "--directory" {
                opts.directory = true;
            } else if arg == "--dry-run" {
                opts.dry_run = true;
            } else if arg == "--quiet" {
                opts.quiet = true;
            } else if arg.starts_with("--suffix=") {
                opts.suffix = Some(arg["--suffix=".len()..].to_string());
            } else if arg == "--suffix" {
                i += 1;
                opts.suffix = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            "mktemp: option '--suffix' requires an argument".to_string()
                        })?
                        .clone(),
                );
            } else if arg.starts_with("--tmpdir=") {
                opts.tmpdir = Some(arg["--tmpdir=".len()..].to_string());
            } else if arg == "--tmpdir" {
                i += 1;
                opts.tmpdir = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            "mktemp: option '--tmpdir' requires an argument".to_string()
                        })?
                        .clone(),
                );
            } else if arg == "-p" {
                i += 1;
                opts.tmpdir = Some(
                    args.get(i)
                        .ok_or_else(|| "mktemp: option '-p' requires an argument".to_string())?
                        .clone(),
                );
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                let mut chars = arg[1..].chars().peekable();
                while let Some(ch) = chars.next() {
                    match ch {
                        'd' => opts.directory = true,
                        'u' => opts.dry_run = true,
                        'q' => opts.quiet = true,
                        'p' => {
                            // -p DIR may be -pDIR or -p DIR
                            let rest: String = chars.collect();
                            if !rest.is_empty() {
                                opts.tmpdir = Some(rest);
                            } else {
                                i += 1;
                                opts.tmpdir = Some(
                                    args.get(i)
                                        .ok_or_else(|| {
                                            "mktemp: option '-p' requires an argument".to_string()
                                        })?
                                        .clone(),
                                );
                            }
                            break;
                        }
                        _ => return Err(format!("mktemp: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.template = Some(arg.clone());
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

/// Replace the trailing run of 'X' characters in `template` with random chars.
fn instantiate_template(template: &str) -> Result<String, String> {
    // Count trailing X's (minimum 3 required by POSIX)
    let x_count = template.chars().rev().take_while(|&c| c == 'X').count();
    if x_count < 3 {
        return Err(format!("mktemp: too few X's in template '{}'", template));
    }

    let prefix = &template[..template.len() - x_count];
    let random_part = generate_random_string(x_count);
    Ok(format!("{}{}", prefix, random_part))
}

fn generate_random_string(len: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);

    let mut out = String::with_capacity(len);
    let mut seed = hasher.finish();
    for _ in 0..len {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let idx = ((seed >> 33) as usize) % charset.len();
        out.push(charset[idx] as char);
    }
    out
}

pub fn builtin_mktemp(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match MktempOptions::parse(args) {
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

    // Determine the base directory for the temp file.
    let base_dir: PathBuf = if let Some(ref dir) = opts.tmpdir {
        resolve_path(dir, runtime.get_cwd())
    } else {
        std::env::temp_dir()
    };

    // Determine the template.
    let raw_template = opts.template.as_deref().unwrap_or("tmp.XXXXXXXXXX");

    // Apply --suffix if given.
    let template_with_suffix = if let Some(ref suf) = opts.suffix {
        format!("{}{}", raw_template, suf)
    } else {
        raw_template.to_string()
    };

    // If the template is not absolute and no --tmpdir, treat it as a filename
    // and combine with base_dir only when opts.tmpdir was given.
    // Standard mktemp: template can include a path; if it has no dir component,
    // it's placed in TMPDIR (or /tmp).
    let template_path = PathBuf::from(&template_with_suffix);
    let final_path = if template_path.is_absolute() {
        template_path
    } else if template_path
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(true)
    {
        // No directory component — use base_dir
        base_dir.join(&template_with_suffix)
    } else {
        // Template has a relative dir component — join with cwd
        runtime.get_cwd().join(&template_with_suffix)
    };

    // Instantiate: replace trailing X's.
    let final_str = final_path.to_string_lossy().to_string();

    // We may need to retry if there's a collision (unlikely but correct).
    let max_attempts = 100;
    for _ in 0..max_attempts {
        let instantiated = match instantiate_template(&final_str) {
            Ok(s) => s,
            Err(e) => {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: format!("{}\n", e),
                    exit_code: 1,
                    error: None,
                })
            }
        };
        let path = PathBuf::from(&instantiated);

        if opts.dry_run {
            return Ok(ExecutionResult::success(format!("{}\n", path.display())));
        }

        if opts.directory {
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(ExecutionResult::success(format!("{}\n", path.display()))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    if !opts.quiet {
                        return Ok(ExecutionResult {
                            output: Output::Text(String::new()),
                            stderr: format!(
                                "mktemp: failed to create directory via template '{}': {}\n",
                                final_str, e
                            ),
                            exit_code: 1,
                            error: None,
                        });
                    }
                    return Ok(ExecutionResult {
                        output: Output::Text(String::new()),
                        stderr: String::new(),
                        exit_code: 1,
                        error: None,
                    });
                }
            }
        } else {
            use std::fs::OpenOptions;
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(ExecutionResult::success(format!("{}\n", path.display()))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    if !opts.quiet {
                        return Ok(ExecutionResult {
                            output: Output::Text(String::new()),
                            stderr: format!(
                                "mktemp: failed to create file via template '{}': {}\n",
                                final_str, e
                            ),
                            exit_code: 1,
                            error: None,
                        });
                    }
                    return Ok(ExecutionResult {
                        output: Output::Text(String::new()),
                        stderr: String::new(),
                        exit_code: 1,
                        error: None,
                    });
                }
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: format!(
            "mktemp: failed to create temp file after {} attempts\n",
            max_attempts
        ),
        exit_code: 1,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: mktemp [OPTION]... [TEMPLATE]
Create a temporary file or directory, safely, and print its name.
TEMPLATE must contain at least 3 consecutive 'X's (e.g. tmp.XXXXXXXXXX).
If TEMPLATE is not specified, use tmp.XXXXXXXXXX, and --tmpdir is implied.

Options:
  -d, --directory     create a directory, not a file
  -u, --dry-run       do not create anything; merely print a name (unsafe)
  -q, --quiet         suppress diagnostics about file/dir creation failure
  --suffix=SUFF       append SUFF to TEMPLATE; SUFF must not contain slash
  --tmpdir=DIR, -p DIR  interpret TEMPLATE relative to DIR
  --help              display this help and exit

Examples:
  mktemp                        create a temporary file in /tmp
  mktemp -d                     create a temporary directory in /tmp
  mktemp myfile.XXXXXXXX        create myfile.XXXXXXXX in /tmp
  mktemp -p /var/tmp foo.XXXXX  create foo.XXXXX in /var/tmp
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
    fn test_mktemp_creates_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mktemp(
            &["-p".to_string(), tmp.path().to_str().unwrap().to_string()],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let path_str = result.stdout().trim().to_string();
        assert!(!path_str.is_empty());
        assert!(
            PathBuf::from(&path_str).exists(),
            "file not created: {}",
            path_str
        );
    }

    #[test]
    fn test_mktemp_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mktemp(
            &[
                "-d".to_string(),
                "-p".to_string(),
                tmp.path().to_str().unwrap().to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let path_str = result.stdout().trim().to_string();
        assert!(
            PathBuf::from(&path_str).is_dir(),
            "dir not created: {}",
            path_str
        );
    }

    #[test]
    fn test_mktemp_dry_run_no_create() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_mktemp(
            &[
                "-u".to_string(),
                "-p".to_string(),
                tmp.path().to_str().unwrap().to_string(),
            ],
            &mut rt,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let path_str = result.stdout().trim().to_string();
        // Dry run: path printed but not created
        assert!(!PathBuf::from(&path_str).exists());
    }

    #[test]
    fn test_mktemp_too_few_xs_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        // Template has only 2 X's — should fail
        let result = builtin_mktemp(
            &[
                "-p".to_string(),
                tmp.path().to_str().unwrap().to_string(),
                "tmp.XX".to_string(),
            ],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("too few X's"));
    }

    #[test]
    fn test_mktemp_custom_template() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let template = format!("{}/myprefix.XXXXXXXX", tmp.path().display());

        let result = builtin_mktemp(&[template], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        let path_str = result.stdout().trim().to_string();
        let path = PathBuf::from(&path_str);
        assert!(path.exists());
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("myprefix."));
    }
}
