use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;
use std::path::{Path, PathBuf};

struct DuOptions {
    /// -s / --summarize: display only a total for each argument
    summarize: bool,
    /// -h / --human-readable: print sizes in human readable format
    human_readable: bool,
    /// -a / --all: show counts for all files, not just directories
    all: bool,
    /// -c / --total: produce a grand total
    total: bool,
    /// -d N / --max-depth=N: print sizes up to N levels deep
    max_depth: Option<usize>,
    /// --block-size=SIZE: scale sizes by SIZE before printing
    block_size: u64,
    /// -k: like --block-size=1024
    kilobytes: bool,
    /// -m: like --block-size=1M
    megabytes: bool,
    files: Vec<String>,
}

impl DuOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = DuOptions {
            summarize: false,
            human_readable: false,
            all: false,
            total: false,
            max_depth: None,
            block_size: 1024, // default: 1K blocks (like GNU du)
            kilobytes: false,
            megabytes: false,
            files: vec![],
        };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                opts.files.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--summarize" {
                opts.summarize = true;
            } else if arg == "--human-readable" {
                opts.human_readable = true;
            } else if arg == "--all" {
                opts.all = true;
            } else if arg == "--total" {
                opts.total = true;
            } else if arg.starts_with("--max-depth=") {
                let n = arg["--max-depth=".len()..].parse::<usize>().map_err(|_| {
                    format!(
                        "du: invalid maximum depth '{}'",
                        &arg["--max-depth=".len()..]
                    )
                })?;
                opts.max_depth = Some(n);
            } else if arg == "--max-depth" || arg == "-d" {
                i += 1;
                let n = args
                    .get(i)
                    .ok_or_else(|| format!("du: option '{}' requires an argument", arg))?
                    .parse::<usize>()
                    .map_err(|v| format!("du: invalid maximum depth '{}'", v))?;
                opts.max_depth = Some(n);
            } else if arg.starts_with("--block-size=") {
                opts.block_size = parse_block_size(&arg["--block-size=".len()..]).map_err(|e| e)?;
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                let mut chars = arg[1..].chars().peekable();
                while let Some(ch) = chars.next() {
                    match ch {
                        's' => opts.summarize = true,
                        'h' => opts.human_readable = true,
                        'a' => opts.all = true,
                        'c' => opts.total = true,
                        'k' => opts.kilobytes = true,
                        'm' => opts.megabytes = true,
                        'd' => {
                            let rest: String = chars.collect();
                            let depth_str = if rest.is_empty() {
                                i += 1;
                                args.get(i)
                                    .ok_or_else(|| {
                                        "du: option '-d' requires an argument".to_string()
                                    })?
                                    .clone()
                            } else {
                                rest
                            };
                            opts.max_depth = Some(depth_str.parse::<usize>().map_err(|_| {
                                format!("du: invalid maximum depth '{}'", depth_str)
                            })?);
                            break;
                        }
                        _ => return Err(format!("du: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                opts.files.push(arg.clone());
            }
            i += 1;
        }

        // Block size priority: -m > -k > default 1K
        if opts.megabytes {
            opts.block_size = 1024 * 1024;
        } else if opts.kilobytes {
            opts.block_size = 1024;
        }

        Ok(opts)
    }
}

fn parse_block_size(s: &str) -> Result<u64, String> {
    let s = s.to_uppercase();
    if s.ends_with("KB") {
        Ok(1000)
    } else if s.ends_with('K') {
        Ok(1024)
    } else if s.ends_with("MB") {
        Ok(1_000_000)
    } else if s.ends_with('M') {
        Ok(1024 * 1024)
    } else if s.ends_with("GB") {
        Ok(1_000_000_000)
    } else if s.ends_with('G') {
        Ok(1024 * 1024 * 1024)
    } else {
        s.parse::<u64>()
            .map_err(|_| format!("du: invalid block size '{}'", s))
    }
}

fn format_size(bytes: u64, opts: &DuOptions) -> String {
    if opts.human_readable {
        human_size(bytes)
    } else {
        let blocks = (bytes + opts.block_size - 1) / opts.block_size;
        blocks.to_string()
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    if bytes == 0 {
        return "0B".to_string();
    }
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if size < 10.0 {
        format!("{:.1}{}", size, UNITS[unit_idx])
    } else {
        format!("{:.0}{}", size, UNITS[unit_idx])
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

/// Compute disk usage for a path. Returns (total_bytes, lines_to_emit).
/// `depth` is current depth (0 = the root argument).
fn du_path(path: &Path, opts: &DuOptions, depth: usize, output: &mut String) -> u64 {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };

    if meta.is_dir() {
        let mut total: u64 = 0;
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let child_path = entry.path();
                    let child_bytes = du_path(&child_path, opts, depth + 1, output);
                    total += child_bytes;
                }
            }
            Err(_) => {}
        }
        // Also count the directory itself (minimal; on Linux it's typically 4096 bytes)
        total += meta.len();

        let show = if opts.summarize {
            depth == 0
        } else if let Some(max) = opts.max_depth {
            depth <= max
        } else {
            true
        };

        if show {
            output.push_str(&format!(
                "{}\t{}\n",
                format_size(total, opts),
                path.display()
            ));
        }

        total
    } else {
        let size = meta.len();

        if opts.all {
            let show = if opts.summarize {
                false // -s suppresses individual file lines
            } else if let Some(max) = opts.max_depth {
                depth <= max
            } else {
                true
            };
            if show {
                output.push_str(&format!(
                    "{}\t{}\n",
                    format_size(size, opts),
                    path.display()
                ));
            }
        }

        size
    }
}

pub fn builtin_du(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match DuOptions::parse(args) {
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

    let paths: Vec<PathBuf> = if opts.files.is_empty() {
        vec![runtime.get_cwd().clone()]
    } else {
        opts.files
            .iter()
            .map(|f| resolve_path(f, runtime.get_cwd()))
            .collect()
    };

    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut grand_total: u64 = 0;
    let mut exit_code = 0;

    for path in &paths {
        if !path.exists() && !path.is_symlink() {
            stderr_output.push_str(&format!(
                "du: cannot access '{}': No such file or directory\n",
                path.display()
            ));
            exit_code = 1;
            continue;
        }
        let bytes = du_path(path, &opts, 0, &mut output);
        grand_total += bytes;
    }

    if opts.total {
        output.push_str(&format!("{}\ttotal\n", format_size(grand_total, &opts)));
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

const HELP_TEXT: &str = "Usage: du [OPTION]... [FILE]...
Summarize disk usage of the set of FILEs, recursively for directories.

Options:
  -a, --all             write counts for all files, not just directories
  -c, --total           produce a grand total
  -d, --max-depth=N     print the total for a directory only if it is N
                        or fewer levels below the command line argument
  -h, --human-readable  print sizes in human readable format (e.g., 1K, 234M)
  -k                    like --block-size=1K (default)
  -m                    like --block-size=1M
  -s, --summarize       display only a total for each argument
  --block-size=SIZE     scale sizes by SIZE before printing
  --help                display this help and exit

Examples:
  du                    show disk usage of current directory
  du -sh .              summarize current directory in human-readable form
  du -h --max-depth=1   show size of immediate subdirectories
  du -a /tmp            show all files in /tmp
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
    fn test_du_directory() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("file.txt"), "hello world").unwrap();

        let result = builtin_du(&[".".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().contains('.'));
    }

    #[test]
    fn test_du_summarize() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("f.txt"), "data").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("g.txt"), "more data").unwrap();

        let result = builtin_du(&["-s".to_string(), ".".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        // -s: only one line
        let out = result.stdout();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "expected exactly 1 line: {:?}", lines);
    }

    #[test]
    fn test_du_human_readable() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        // Write a 1025-byte file (> 1K)
        std::fs::write(tmp.path().join("big.txt"), vec![b'x'; 1025]).unwrap();

        let result = builtin_du(&["-sh".to_string(), ".".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        // Should contain a size with a unit letter
        let out = result.stdout();
        assert!(
            out.contains('K') || out.contains('M') || out.contains('G') || out.contains('B'),
            "expected human-readable size in: {}",
            out
        );
    }

    #[test]
    fn test_du_nonexistent_path() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);

        let result = builtin_du(&["/nonexistent/du_test_path".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("cannot access"));
    }

    #[test]
    fn test_du_with_total() {
        let tmp = TempDir::new().unwrap();
        let mut rt = make_runtime(&tmp);
        std::fs::write(tmp.path().join("f1.txt"), "aaa").unwrap();
        std::fs::write(tmp.path().join("f2.txt"), "bbb").unwrap();

        let result = builtin_du(&["-sc".to_string(), ".".to_string()], &mut rt).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout().contains("total"));
    }
}
