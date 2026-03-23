use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Options for the mkdir command
#[derive(Debug, Default)]
struct MkdirOptions {
    /// Create parent directories as needed (-p flag)
    create_parents: bool,
    /// Directories to create
    directories: Vec<String>,
}

impl MkdirOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = MkdirOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                // Parse flags
                for ch in arg[1..].chars() {
                    match ch {
                        'p' => opts.create_parents = true,
                        _ => return Err(anyhow!("mkdir: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                // This is a directory argument
                opts.directories.push(arg.clone());
            }
            i += 1;
        }

        if opts.directories.is_empty() {
            return Err(anyhow!(
                "mkdir: missing operand\nTry 'mkdir --help' for more information."
            ));
        }

        Ok(opts)
    }
}

pub fn builtin_mkdir(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // Handle --help
    if args.len() == 1 && args[0] == "--help" {
        return Ok(ExecutionResult::success(
            "Usage: mkdir [OPTION]... DIRECTORY...\n\
             Create the DIRECTORY(ies), if they do not already exist.\n\
             \n\
             Options:\n\
             -p          create parent directories as needed\n\
             --help      display this help and exit\n"
                .to_string(),
        ));
    }

    let opts = match MkdirOptions::parse(args) {
        Ok(opts) => opts,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("{}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut stderr_output = String::new();
    let mut exit_code = 0;
    let mut created_dirs = Vec::new();

    for dir_arg in &opts.directories {
        // Resolve path (handle ~ and make absolute)
        let path = resolve_path(dir_arg, runtime.get_cwd());

        // Try to create the directory
        let result = if opts.create_parents {
            create_dir_with_parents(&path, &mut created_dirs)
        } else {
            create_dir_single(&path, &mut created_dirs)
        };

        if let Err(e) = result {
            stderr_output.push_str(&format!(
                "mkdir: cannot create directory '{}': {}\n",
                dir_arg, e
            ));
            exit_code = 1;
        }
    }

    // Track created directories in undo system if any were created
    if !created_dirs.is_empty() {
        for dir_path in created_dirs {
            let description = format!("mkdir {}", dir_path.display());
            runtime
                .undo_manager_mut()
                .track_create(dir_path, description);
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(String::new()),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
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

/// Create a single directory (no parent creation)
fn create_dir_single(path: &Path, created_dirs: &mut Vec<PathBuf>) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            return Err(anyhow!("File exists"));
        } else {
            return Err(anyhow!("File exists (not a directory)"));
        }
    }

    // Check if parent exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(anyhow!("No such file or directory"));
        }
    }

    fs::create_dir(path).with_context(|| format!("Failed to create directory {:?}", path))?;

    created_dirs.push(path.to_path_buf());
    Ok(())
}

/// Create directory with parents (-p flag)
fn create_dir_with_parents(path: &Path, created_dirs: &mut Vec<PathBuf>) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            // Directory already exists, this is OK with -p
            return Ok(());
        } else {
            return Err(anyhow!("File exists (not a directory)"));
        }
    }

    // Track all directories we create
    let mut dirs_to_create = Vec::new();
    let mut current = path;

    // Find all non-existent parent directories
    while !current.exists() {
        dirs_to_create.push(current.to_path_buf());
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    // Create directories from parent to child
    for dir in dirs_to_create.iter().rev() {
        fs::create_dir(dir).with_context(|| format!("Failed to create directory {:?}", dir))?;
        created_dirs.push(dir.clone());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mkdir_single_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("testdir");
        let result = builtin_mkdir(&["testdir".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(test_dir.exists());
        assert!(test_dir.is_dir());
    }

    #[test]
    fn test_mkdir_multiple_directories() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        let dir3 = temp_dir.path().join("dir3");

        let result = builtin_mkdir(
            &["dir1".to_string(), "dir2".to_string(), "dir3".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(dir1.exists() && dir1.is_dir());
        assert!(dir2.exists() && dir2.is_dir());
        assert!(dir3.exists() && dir3.is_dir());
    }

    #[test]
    fn test_mkdir_with_parents() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let nested_dir = temp_dir
            .path()
            .join("parent")
            .join("child")
            .join("grandchild");
        let result = builtin_mkdir(
            &["-p".to_string(), "parent/child/grandchild".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_mkdir_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("existing");
        fs::create_dir(&test_dir).unwrap();

        let result = builtin_mkdir(&["existing".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("File exists"));
    }

    #[test]
    fn test_mkdir_already_exists_with_p_flag() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("existing");
        fs::create_dir(&test_dir).unwrap();

        let result =
            builtin_mkdir(&["-p".to_string(), "existing".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(test_dir.exists());
    }

    #[test]
    fn test_mkdir_no_parent_without_p_flag() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_mkdir(&["nonexistent/child".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("No such file or directory"));
    }

    #[test]
    fn test_mkdir_missing_operand() {
        let mut runtime = Runtime::new();
        let result = builtin_mkdir(&[], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("missing operand"));
    }

    #[test]
    fn test_mkdir_invalid_option() {
        let mut runtime = Runtime::new();
        let result = builtin_mkdir(&["-x".to_string(), "test".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    #[test]
    fn test_mkdir_help() {
        let mut runtime = Runtime::new();
        let result = builtin_mkdir(&["--help".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Usage: mkdir"));
        assert!(result.stdout().contains("-p"));
    }

    #[test]
    fn test_mkdir_file_exists_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("file.txt");
        fs::write(&test_file, "content").unwrap();

        let result = builtin_mkdir(&["file.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("File exists"));
    }

    #[test]
    fn test_mkdir_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();

        let abs_path = temp_dir.path().join("absolute_dir");
        let result =
            builtin_mkdir(&[abs_path.to_string_lossy().to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(abs_path.exists());
    }

    #[test]
    fn test_mkdir_undo_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("undoable");

        // Create directory
        let result = builtin_mkdir(&["undoable".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(test_dir.exists());

        // Verify it was tracked in undo system
        let ops = runtime.undo_manager_mut().list_operations(10);
        assert_eq!(ops.len(), 1);
        assert!(ops[0].description.contains("mkdir"));

        // Undo the creation
        let undo_result = runtime.undo_manager_mut().undo().unwrap();
        assert!(undo_result.contains("mkdir"));
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_mkdir_with_parents_undo_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let nested_dir = temp_dir.path().join("a").join("b").join("c");

        // Create nested directories
        let result = builtin_mkdir(&["-p".to_string(), "a/b/c".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(nested_dir.exists());

        // Verify all directories were tracked (a, a/b, a/b/c)
        let ops = runtime.undo_manager_mut().list_operations(10);
        assert_eq!(ops.len(), 3);

        // Undo all creations
        runtime.undo_manager_mut().undo().unwrap();
        runtime.undo_manager_mut().undo().unwrap();
        runtime.undo_manager_mut().undo().unwrap();

        assert!(!temp_dir.path().join("a").exists());
    }

    #[test]
    fn test_mkdir_partial_failure() {
        let temp_dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Create one directory that will exist
        let existing = temp_dir.path().join("existing");
        fs::create_dir(&existing).unwrap();

        // Try to create existing and a new one
        let result = builtin_mkdir(
            &["existing".to_string(), "newdir".to_string()],
            &mut runtime,
        )
        .unwrap();

        // Should have partial failure
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("File exists"));

        // But newdir should still be created
        let new_dir = temp_dir.path().join("newdir");
        assert!(new_dir.exists());
    }
}
