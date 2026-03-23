use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Options for the rm command
#[derive(Debug, Default)]
struct RmOptions {
    /// Remove directories and their contents recursively (-r, -R, --recursive)
    recursive: bool,
    /// Ignore nonexistent files and arguments, never prompt (-f, --force)
    force: bool,
    /// Never prompt (combine with -f for scripts) (-y, --yes)
    yes: bool,
    /// Prompt before every removal (-i)
    interactive: bool,
    /// Remove empty directories (-d, --dir)
    dir: bool,
    /// Explain what is being done (-v, --verbose)
    verbose: bool,
    /// Files/directories to remove
    paths: Vec<String>,
}

/// Statistics about files to be deleted
#[derive(Debug, Default)]
struct DeletionStats {
    file_count: u64,
    dir_count: u64,
    total_size: u64,
}

impl RmOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = RmOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                // Everything after -- is a path
                opts.paths.extend(args[i + 1..].iter().cloned());
                break;
            } else if arg == "--recursive" {
                opts.recursive = true;
            } else if arg == "--force" {
                opts.force = true;
            } else if arg == "--yes" {
                opts.yes = true;
            } else if arg == "--interactive" {
                opts.interactive = true;
            } else if arg == "--dir" {
                opts.dir = true;
            } else if arg == "--verbose" {
                opts.verbose = true;
            } else if arg == "--help" {
                return Err(anyhow!("HELP"));
            } else if arg.starts_with("--") {
                return Err(anyhow!("rm: unrecognized option '{}'", arg));
            } else if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                // Parse short flags
                for ch in arg[1..].chars() {
                    match ch {
                        'r' | 'R' => opts.recursive = true,
                        'f' => opts.force = true,
                        'y' => opts.yes = true,
                        'i' => opts.interactive = true,
                        'd' => opts.dir = true,
                        'v' => opts.verbose = true,
                        _ => return Err(anyhow!("rm: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                // This is a path argument
                opts.paths.push(arg.clone());
            }
            i += 1;
        }

        // -f overrides -i
        if opts.force {
            opts.interactive = false;
        }

        Ok(opts)
    }

    /// Check if this is a destructive recursive operation that needs confirmation
    fn needs_confirmation(&self) -> bool {
        // Confirm for recursive operations unless --yes or --force is set
        self.recursive && !self.yes && !self.force
    }
}

impl DeletionStats {
    /// Calculate statistics for a path (file or directory)
    fn calculate(path: &Path) -> Result<Self> {
        let mut stats = DeletionStats::default();
        Self::calculate_recursive(path, &mut stats)?;
        Ok(stats)
    }

    fn calculate_recursive(path: &Path, stats: &mut Self) -> Result<()> {
        if path.is_symlink() {
            // Symlinks: count as file, size is link size (not target)
            stats.file_count += 1;
            if let Ok(metadata) = path.symlink_metadata() {
                stats.total_size += metadata.len();
            }
        } else if path.is_dir() {
            stats.dir_count += 1;
            // Recursively count contents
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    Self::calculate_recursive(&entry.path(), stats)?;
                }
            }
        } else if path.is_file() {
            stats.file_count += 1;
            if let Ok(metadata) = path.metadata() {
                stats.total_size += metadata.len();
            }
        }
        Ok(())
    }

    /// Format the size in human-readable form
    fn format_size(&self) -> String {
        let size = self.total_size;
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Generate summary string for confirmation prompt
    fn summary(&self) -> String {
        let files = if self.file_count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", self.file_count)
        };
        let dirs = if self.dir_count == 1 {
            "1 directory".to_string()
        } else {
            format!("{} directories", self.dir_count)
        };
        format!("{}, {} ({})", files, dirs, self.format_size())
    }
}

/// Resolve path (handle ~ and make absolute)
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

    // Make absolute if relative
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

/// Check if running interactively (stdin is a tty)
fn is_interactive() -> bool {
    use nix::libc;
    use std::os::unix::io::AsRawFd;
    unsafe { libc::isatty(io::stdin().as_raw_fd()) != 0 }
}

/// Prompt user for confirmation and return their response
fn prompt_confirmation(message: &str) -> bool {
    if !is_interactive() {
        // Non-interactive mode: default to No
        return false;
    }

    eprint!("{} [y/N] ", message);
    io::stderr().flush().ok();

    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_ok() {
        let response = line.trim().to_lowercase();
        matches!(response.as_str(), "y" | "yes")
    } else {
        false
    }
}

/// Remove a file with undo tracking
fn remove_file(
    path: &Path,
    runtime: &mut Runtime,
    verbose: bool,
    output: &mut String,
) -> Result<()> {
    let display_path = path.display().to_string();

    // Track in undo system before deletion
    let description = format!("rm {}", display_path);
    runtime.undo_manager_mut().track_delete(path, description)?;

    // Perform the deletion
    fs::remove_file(path)?;

    if verbose {
        output.push_str(&format!("removed '{}'\n", display_path));
    }

    Ok(())
}

/// Remove a directory (empty) with undo tracking
fn remove_empty_dir(
    path: &Path,
    runtime: &mut Runtime,
    verbose: bool,
    output: &mut String,
) -> Result<()> {
    let display_path = path.display().to_string();

    // Track creation (since we're deleting an empty dir, tracking it as create
    // means undo will delete the recreated dir - but we need backup for non-empty)
    // For empty dirs, we just track the path
    let description = format!("rmdir {}", display_path);
    runtime
        .undo_manager_mut()
        .track_create(path.to_path_buf(), description);

    // Perform the deletion
    fs::remove_dir(path)?;

    if verbose {
        output.push_str(&format!("removed directory '{}'\n", display_path));
    }

    Ok(())
}

/// Remove a directory recursively with undo tracking
fn remove_dir_recursive(
    path: &Path,
    runtime: &mut Runtime,
    verbose: bool,
    output: &mut String,
) -> Result<()> {
    // First, recursively process contents
    let entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();

    for entry in entries {
        let entry_path = entry.path();
        if entry_path.is_dir() && !entry_path.is_symlink() {
            remove_dir_recursive(&entry_path, runtime, verbose, output)?;
        } else {
            remove_file(&entry_path, runtime, verbose, output)?;
        }
    }

    // Now remove the empty directory
    remove_empty_dir(path, runtime, verbose, output)
}

pub fn builtin_rm(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Handle --help
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
    }

    let opts = match RmOptions::parse(args) {
        Ok(opts) => opts,
        Err(e) if e.to_string() == "HELP" => {
            return Ok(ExecutionResult::success(HELP_TEXT.to_string()));
        }
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\nTry 'rm --help' for more information.\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    if opts.paths.is_empty() {
        if opts.force {
            // rm -f with no args is a silent success (POSIX behavior)
            return Ok(ExecutionResult::success(String::new()));
        }
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "rm: missing operand\nTry 'rm --help' for more information.\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let mut stdout_output = String::new();
    let mut stderr_output = String::new();
    let mut exit_code = 0;

    // Resolve all paths first
    let resolved_paths: Vec<PathBuf> = opts
        .paths
        .iter()
        .map(|p| resolve_path(p, runtime.get_cwd()))
        .collect();

    // For recursive operations, calculate stats and confirm if needed
    if opts.needs_confirmation() {
        let mut total_stats = DeletionStats::default();
        let mut paths_to_process = Vec::new();

        for (i, path) in resolved_paths.iter().enumerate() {
            let original = &opts.paths[i];
            if !path.exists() {
                if !opts.force {
                    stderr_output.push_str(&format!(
                        "rm: cannot remove '{}': No such file or directory\n",
                        original
                    ));
                    exit_code = 1;
                }
                continue;
            }

            if path.is_dir() {
                match DeletionStats::calculate(path) {
                    Ok(stats) => {
                        total_stats.file_count += stats.file_count;
                        total_stats.dir_count += stats.dir_count;
                        total_stats.total_size += stats.total_size;
                        paths_to_process.push((path.clone(), original.clone()));
                    }
                    Err(e) => {
                        stderr_output
                            .push_str(&format!("rm: cannot access '{}': {}\n", original, e));
                        exit_code = 1;
                    }
                }
            } else {
                // Single file in recursive mode
                if let Ok(metadata) = path.metadata() {
                    total_stats.file_count += 1;
                    total_stats.total_size += metadata.len();
                }
                paths_to_process.push((path.clone(), original.clone()));
            }
        }

        // Show what will be deleted and ask for confirmation
        if !paths_to_process.is_empty() && (total_stats.file_count > 0 || total_stats.dir_count > 0)
        {
            let prompt = format!("rm: about to delete {}\nProceed?", total_stats.summary());

            if !prompt_confirmation(&prompt) {
                return Ok(ExecutionResult {
                    output: Output::Text(String::new()),
                    stderr: "rm: operation cancelled\n".to_string(),
                    exit_code: 1,
                    error: None,
                });
            }
        }
    }

    // Process each path
    for (i, path) in resolved_paths.iter().enumerate() {
        let original = &opts.paths[i];

        if !path.exists() {
            if !opts.force {
                stderr_output.push_str(&format!(
                    "rm: cannot remove '{}': No such file or directory\n",
                    original
                ));
                exit_code = 1;
            }
            continue;
        }

        // Interactive mode: prompt for each item
        if opts.interactive {
            let prompt = if path.is_dir() {
                format!("rm: remove directory '{}'?", original)
            } else {
                format!("rm: remove file '{}'?", original)
            };
            if !prompt_confirmation(&prompt) {
                continue;
            }
        }

        if path.is_dir() && !path.is_symlink() {
            if opts.recursive {
                // Recursive removal
                if let Err(e) =
                    remove_dir_recursive(path, runtime, opts.verbose, &mut stdout_output)
                {
                    stderr_output.push_str(&format!("rm: cannot remove '{}': {}\n", original, e));
                    exit_code = 1;
                }
            } else if opts.dir {
                // Remove empty directory only
                if let Err(e) = remove_empty_dir(path, runtime, opts.verbose, &mut stdout_output) {
                    stderr_output.push_str(&format!("rm: cannot remove '{}': {}\n", original, e));
                    exit_code = 1;
                }
            } else {
                stderr_output.push_str(&format!(
                    "rm: cannot remove '{}': Is a directory\n",
                    original
                ));
                exit_code = 1;
            }
        } else {
            // Regular file or symlink
            if let Err(e) = remove_file(path, runtime, opts.verbose, &mut stdout_output) {
                stderr_output.push_str(&format!("rm: cannot remove '{}': {}\n", original, e));
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

const HELP_TEXT: &str = "Usage: rm [OPTION]... [FILE]...
Remove (unlink) the FILE(s).

Options:
  -f, --force       ignore nonexistent files and arguments, never prompt
  -i                prompt before every removal
  -r, -R, --recursive  remove directories and their contents recursively
  -d, --dir         remove empty directories
  -v, --verbose     explain what is being done
  -y, --yes         skip confirmation prompts (for use in scripts)
  --help            display this help and exit

CONFIRMATION PROMPTS:
  When using -r (recursive), rm will prompt for confirmation showing:
    - Number of files to be deleted
    - Number of directories to be deleted
    - Total size of data to be deleted

  To skip the confirmation:
    - Use -y or --yes flag
    - Use -f or --force flag
    - Pipe input (non-interactive mode defaults to No)

  In scripts, use -y or -f to auto-confirm, or the command will fail.

UNDO SUPPORT:
  Deleted files are backed up and can be restored with the 'undo' command.
  Use 'undo list' to see recent operations.

Examples:
  rm file.txt              Remove a file
  rm -r dir                Remove directory (with confirmation)
  rm -rf dir               Remove directory without confirmation
  rm -ry dir               Remove directory, auto-confirm
  rm -i *.txt              Interactively remove all .txt files
  rm -v file1 file2        Remove files verbosely
";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rm_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let result = builtin_rm(&["test.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!test_file.exists());
    }

    #[test]
    fn test_rm_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file3 = temp_dir.path().join("file3.txt");
        fs::write(&file1, "1").unwrap();
        fs::write(&file2, "2").unwrap();
        fs::write(&file3, "3").unwrap();

        let result = builtin_rm(
            &[
                "file1.txt".to_string(),
                "file2.txt".to_string(),
                "file3.txt".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(!file3.exists());
    }

    #[test]
    fn test_rm_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_rm(&["nonexistent.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("No such file or directory"));
    }

    #[test]
    fn test_rm_force_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_rm(
            &["-f".to_string(), "nonexistent.txt".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_rm_force_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_rm(&["-f".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_rm_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_rm(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_rm_directory_without_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("testdir");
        fs::create_dir(&test_dir).unwrap();

        let result = builtin_rm(&["testdir".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("Is a directory"));
        assert!(test_dir.exists());
    }

    #[test]
    fn test_rm_empty_dir_with_d_flag() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("emptydir");
        fs::create_dir(&test_dir).unwrap();

        let result = builtin_rm(&["-d".to_string(), "emptydir".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_rm_recursive_with_yes() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("recursive_dir");
        let sub_dir = test_dir.join("subdir");
        let file1 = test_dir.join("file1.txt");
        let file2 = sub_dir.join("file2.txt");

        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let result = builtin_rm(
            &["-ry".to_string(), "recursive_dir".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_rm_recursive_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("force_dir");
        let file1 = test_dir.join("file.txt");

        fs::create_dir(&test_dir).unwrap();
        fs::write(&file1, "content").unwrap();

        let result =
            builtin_rm(&["-rf".to_string(), "force_dir".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_rm_verbose() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("verbose.txt");
        fs::write(&test_file, "content").unwrap();

        let result =
            builtin_rm(&["-v".to_string(), "verbose.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("removed"));
        assert!(result.stdout().contains("verbose.txt"));
    }

    #[test]
    fn test_rm_help() {
        let mut runtime = Runtime::new();
        let result = builtin_rm(&["--help".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Usage: rm"));
        assert!(result.stdout().contains("-r"));
        assert!(result.stdout().contains("-f"));
    }

    #[test]
    fn test_rm_invalid_option() {
        let mut runtime = Runtime::new();
        let result = builtin_rm(&["-z".to_string(), "file.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    #[test]
    fn test_rm_undo_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("undoable.txt");
        fs::write(&test_file, "original content").unwrap();

        // Delete the file
        let result = builtin_rm(&["undoable.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!test_file.exists());

        // Verify it was tracked in undo system
        let ops = runtime.undo_manager_mut().list_operations(10);
        assert!(!ops.is_empty());
        assert!(ops[0].description.contains("rm"));

        // Undo the deletion
        let undo_result = runtime.undo_manager_mut().undo().unwrap();
        assert!(undo_result.contains("restored"));
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "original content");
    }

    #[test]
    fn test_rm_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        fs::write(&target, "content").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = builtin_rm(&["link.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!link.exists());
        assert!(target.exists()); // Target should remain
    }

    #[test]
    fn test_rm_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();

        let abs_path = temp_dir.path().join("absolute.txt");
        fs::write(&abs_path, "content").unwrap();

        let result = builtin_rm(&[abs_path.to_string_lossy().to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!abs_path.exists());
    }

    #[test]
    fn test_deletion_stats_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "hello world").unwrap(); // 11 bytes

        let stats = DeletionStats::calculate(&test_file).unwrap();

        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.dir_count, 0);
        assert_eq!(stats.total_size, 11);
    }

    #[test]
    fn test_deletion_stats_directory() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("testdir");
        let sub_dir = test_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(test_dir.join("file1.txt"), "12345").unwrap(); // 5 bytes
        fs::write(sub_dir.join("file2.txt"), "1234567890").unwrap(); // 10 bytes

        let stats = DeletionStats::calculate(&test_dir).unwrap();

        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.dir_count, 2); // testdir and subdir
        assert_eq!(stats.total_size, 15);
    }

    #[test]
    fn test_deletion_stats_format_size() {
        let mut stats = DeletionStats::default();

        stats.total_size = 500;
        assert!(stats.format_size().contains("500 B"));

        stats.total_size = 2048;
        assert!(stats.format_size().contains("KB"));

        stats.total_size = 2 * 1024 * 1024;
        assert!(stats.format_size().contains("MB"));

        stats.total_size = 3 * 1024 * 1024 * 1024;
        assert!(stats.format_size().contains("GB"));
    }

    #[test]
    fn test_rm_options_parse() {
        // Test basic flags
        let opts = RmOptions::parse(&["-rf".to_string(), "file.txt".to_string()]).unwrap();
        assert!(opts.recursive);
        assert!(opts.force);
        assert_eq!(opts.paths, vec!["file.txt"]);

        // Test long flags
        let opts = RmOptions::parse(&[
            "--recursive".to_string(),
            "--yes".to_string(),
            "dir".to_string(),
        ])
        .unwrap();
        assert!(opts.recursive);
        assert!(opts.yes);
        assert_eq!(opts.paths, vec!["dir"]);

        // Test combined flags
        let opts =
            RmOptions::parse(&["-rvy".to_string(), "a".to_string(), "b".to_string()]).unwrap();
        assert!(opts.recursive);
        assert!(opts.verbose);
        assert!(opts.yes);
        assert_eq!(opts.paths, vec!["a", "b"]);
    }

    #[test]
    fn test_rm_needs_confirmation() {
        let mut opts = RmOptions::default();

        // Not recursive - no confirmation needed
        assert!(!opts.needs_confirmation());

        // Recursive without yes/force - needs confirmation
        opts.recursive = true;
        assert!(opts.needs_confirmation());

        // Recursive with yes - no confirmation needed
        opts.yes = true;
        assert!(!opts.needs_confirmation());

        // Recursive with force - no confirmation needed
        opts.yes = false;
        opts.force = true;
        assert!(!opts.needs_confirmation());
    }

    #[test]
    fn test_rm_partial_failure() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let existing = temp_dir.path().join("existing.txt");
        fs::write(&existing, "content").unwrap();

        // Try to remove existing and nonexistent
        let result = builtin_rm(
            &["existing.txt".to_string(), "nonexistent.txt".to_string()],
            &mut runtime,
        )
        .unwrap();

        // Should have exit code 1 due to error
        assert_eq!(result.exit_code, 1);
        // But existing file should still be removed
        assert!(!existing.exists());
        // Error message for nonexistent
        assert!(result.stderr.contains("No such file or directory"));
    }

    #[test]
    fn test_rm_recursive_non_interactive_defaults_to_no() {
        // This tests that in non-interactive mode (like tests),
        // rm -r (without -y or -f) fails because confirmation defaults to No
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("test_dir");
        let file = test_dir.join("file.txt");
        fs::create_dir(&test_dir).unwrap();
        fs::write(&file, "content").unwrap();

        // rm -r without -y or -f should fail in non-interactive mode
        let result = builtin_rm(&["-r".to_string(), "test_dir".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("cancelled"));
        assert!(test_dir.exists()); // Directory should still exist
    }
}
