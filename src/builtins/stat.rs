use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

struct StatOptions {
    /// -c / --format FORMAT: print using FORMAT instead of default
    format: Option<String>,
    /// -t / --terse: print in terse form
    terse: bool,
    /// -L / --dereference: follow symlinks
    dereference: bool,
    files: Vec<String>,
}

impl StatOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = StatOptions {
            format: None,
            terse: false,
            dereference: false,
            files: vec![],
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--terse" {
                opts.terse = true;
            } else if arg == "--dereference" {
                opts.dereference = true;
            } else if arg.starts_with("--format=") {
                opts.format = Some(arg["--format=".len()..].to_string());
            } else if arg == "--format" || arg == "-c" {
                i += 1;
                opts.format = Some(
                    args.get(i)
                        .ok_or_else(|| format!("stat: option '{}' requires an argument", arg))?
                        .clone(),
                );
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                for ch in arg[1..].chars() {
                    match ch {
                        't' => opts.terse = true,
                        'L' => opts.dereference = true,
                        'c' => {
                            // handled above via long form; here treat chars after c as format
                            let rest = arg[arg.find('c').unwrap() + 1..].to_string();
                            if !rest.is_empty() {
                                opts.format = Some(rest);
                            } else {
                                i += 1;
                                opts.format = Some(
                                    args.get(i)
                                        .ok_or_else(|| {
                                            "stat: option '-c' requires an argument".to_string()
                                        })?
                                        .clone(),
                                );
                            }
                            break;
                        }
                        _ => return Err(format!("stat: invalid option -- '{}'", ch)),
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

fn file_type_str(meta: &std::fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    let ft = meta.file_type();
    if ft.is_symlink() {
        "symbolic link"
    } else if ft.is_dir() {
        "directory"
    } else if ft.is_file() {
        "regular file"
    } else if ft.is_block_device() {
        "block special file"
    } else if ft.is_char_device() {
        "character special file"
    } else if ft.is_fifo() {
        "fifo"
    } else if ft.is_socket() {
        "socket"
    } else {
        "unknown"
    }
}

fn file_type_char(meta: &std::fs::Metadata) -> char {
    use std::os::unix::fs::FileTypeExt;
    let ft = meta.file_type();
    if ft.is_symlink() {
        'l'
    } else if ft.is_dir() {
        'd'
    } else if ft.is_block_device() {
        'b'
    } else if ft.is_char_device() {
        'c'
    } else if ft.is_fifo() {
        'p'
    } else if ft.is_socket() {
        's'
    } else {
        '-'
    }
}

fn format_permissions(mode: u32) -> String {
    let chars: Vec<char> = "rwxrwxrwx".chars().collect();
    let mut out = String::with_capacity(9);
    for i in 0..9 {
        let bit = 1u32 << (8 - i);
        out.push(if mode & bit != 0 { chars[i] } else { '-' });
    }
    out
}

fn format_time(secs: i64) -> String {
    // Format as "YYYY-MM-DD HH:MM:SS.NANOS +0000"
    use std::time::{Duration, SystemTime};
    let st = if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs((-secs) as u64)
    };
    // Use a simple formatted representation
    let dt = chrono::DateTime::<chrono::Utc>::from(st);
    dt.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
}

fn apply_format(fmt: &str, meta: &std::fs::Metadata, path: &Path) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('n') => out.push_str(&path.to_string_lossy()),
                Some('N') => {
                    if meta.file_type().is_symlink() {
                        if let Ok(target) = std::fs::read_link(path) {
                            out.push_str(&format!(
                                "'{}' -> '{}'",
                                path.display(),
                                target.display()
                            ));
                        } else {
                            out.push_str(&path.to_string_lossy());
                        }
                    } else {
                        out.push_str(&format!("'{}'", path.display()));
                    }
                }
                Some('s') => out.push_str(&meta.size().to_string()),
                Some('b') => out.push_str(&meta.blocks().to_string()),
                Some('B') => out.push_str("512"),
                Some('f') => out.push_str(&format!("{:x}", meta.mode())),
                Some('F') => out.push_str(file_type_str(meta)),
                Some('i') => out.push_str(&meta.ino().to_string()),
                Some('h') => out.push_str(&meta.nlink().to_string()),
                Some('u') => out.push_str(&meta.uid().to_string()),
                Some('U') => {
                    // Attempt to resolve username
                    out.push_str(&meta.uid().to_string());
                }
                Some('g') => out.push_str(&meta.gid().to_string()),
                Some('G') => {
                    out.push_str(&meta.gid().to_string());
                }
                Some('d') => out.push_str(&meta.dev().to_string()),
                Some('r') => out.push_str(&meta.rdev().to_string()),
                Some('a') => out.push_str(&meta.atime().to_string()),
                Some('m') => out.push_str(&meta.mtime().to_string()),
                Some('c') => out.push_str(&meta.ctime().to_string()),
                Some('A') => out.push_str(&format_time(meta.atime())),
                Some('M') => out.push_str(&format_time(meta.mtime())),
                Some('C') => out.push_str(&format_time(meta.ctime())),
                Some('o') => out.push_str(&format!("{}", meta.blksize())),
                Some('x') => out.push_str(&format_permissions(meta.mode() & 0o777)),
                Some('X') => out.push_str(&format!("{:04o}", meta.mode() & 0o7777)),
                Some('%') => out.push('%'),
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        } else if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn stat_file(path: &Path, opts: &StatOptions) -> Result<String, String> {
    let meta = if opts.dereference {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    }
    .map_err(|e| format!("stat: cannot stat '{}': {}", path.display(), e))?;

    if let Some(fmt) = &opts.format {
        return Ok(format!("{}\n", apply_format(fmt, &meta, path)));
    }

    if opts.terse {
        // Terse: name size blocks file_type mode nlink uid gid device inode atime mtime ctime blksize
        return Ok(format!(
            "{} {} {} {:x} {:x} {} {} {} {} {} {} {} {} {}\n",
            path.display(),
            meta.size(),
            meta.blocks(),
            meta.mode(),
            meta.dev(),
            meta.ino(),
            meta.nlink(),
            meta.uid(),
            meta.gid(),
            meta.rdev(),
            meta.atime(),
            meta.mtime(),
            meta.ctime(),
            meta.blksize(),
        ));
    }

    let ftype = file_type_str(&meta);
    let mode_str = format_permissions(meta.mode() & 0o777);
    let octal_mode = format!("{:04o}", meta.mode() & 0o7777);

    let name_line = if meta.file_type().is_symlink() {
        if let Ok(target) = std::fs::read_link(path) {
            format!("File: {} -> {}", path.display(), target.display())
        } else {
            format!("File: {}", path.display())
        }
    } else {
        format!("File: {}", path.display())
    };

    Ok(format!(
        "  {}\n  Size: {:<12} Blocks: {:<10} IO Block: {:<6} {}\nDevice: {:x}h/{:o}d  Inode: {:<10} Links: {}\nAccess: ({}/{}{})  Uid: ({:5}/{:5})   Gid: ({:5}/{:5})\nAccess: {}\nModify: {}\nChange: {}\n Birth: -\n",
        name_line,
        meta.size(),
        meta.blocks(),
        meta.blksize(),
        ftype,
        meta.dev(),
        meta.dev(),
        meta.ino(),
        meta.nlink(),
        octal_mode,
        file_type_char(&meta),
        mode_str,
        meta.uid(),
        meta.uid(),
        meta.gid(),
        meta.gid(),
        format_time(meta.atime()),
        format_time(meta.mtime()),
        format_time(meta.ctime()),
    ))
}

pub fn builtin_stat(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "stat: missing operand\nTry 'stat --help' for more information.\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let opts = match StatOptions::parse(args) {
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
            stderr: "stat: missing operand\nTry 'stat --help' for more information.\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let cwd = runtime.get_cwd().clone();
    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    for file_arg in &opts.files {
        let path = resolve_path(file_arg, &cwd);
        match stat_file(&path, &opts) {
            Ok(s) => output.push_str(&s),
            Err(e) => {
                stderr_output.push_str(&format!("{}\n", e));
                exit_code = 1;
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

const HELP_TEXT: &str = "Usage: stat [OPTION]... FILE...
Display file or file system status.

Options:
  -L, --dereference     follow links
  -c, --format=FORMAT   use the specified FORMAT instead of the default
  -t, --terse           print the information in terse form
  --help                display this help and exit

Format sequences (for -c FORMAT):
  %n   file name
  %N   quoted file name with dereference if symbolic link
  %s   total size in bytes
  %b   number of 512-byte blocks allocated
  %f   raw mode in hex
  %F   file type
  %i   inode number
  %h   number of hard links
  %u   user ID of owner
  %g   group ID of owner
  %d   device number in decimal
  %a   time of last access (seconds since Epoch)
  %m   time of last modification (seconds since Epoch)
  %A   time of last access (human-readable)
  %M   time of last modification (human-readable)
  %X   access rights in octal
  %x   access rights in human-readable form

Examples:
  stat file.txt
  stat -c '%n: %s bytes' file.txt
  stat -L symlink
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
    fn test_stat_regular_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("test.txt"), "hello world").unwrap();

        let result = builtin_stat(&["test.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().contains("regular file"));
        assert!(result.stdout().contains("test.txt"));
    }

    #[test]
    fn test_stat_format_size() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("f.txt"), "hello").unwrap();

        let result = builtin_stat(
            &["-c".to_string(), "%s".to_string(), "f.txt".to_string()],
            &mut rt,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().trim() == "5");
    }

    #[test]
    fn test_stat_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_stat(&["nonexistent.txt".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("cannot stat"));
    }

    #[test]
    fn test_stat_no_args_errors() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_stat(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_stat_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        let sub = tmp.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();

        let result = builtin_stat(&["subdir".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().contains("directory"));
    }
}
