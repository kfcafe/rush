use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use git2::Repository;
use std::path::Path;

/// Native `git add` implementation via git2 bindings.
///
/// Supports:
///   git add FILE...   — stage specific files
///   git add .         — stage all changes in the working tree
///   git add -A        — stage all changes including deletions
///   git add -p        — deferred to external git (interactive patch mode)
pub fn builtin_git_add(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();

    // Open repo from the current working directory
    let repo = match Repository::discover(cwd) {
        Ok(r) => r,
        Err(_) => {
            return Ok(ExecutionResult::error(
                "fatal: not a git repository (or any of the parent directories): .git\n"
                    .to_string(),
            ));
        }
    };

    if args.is_empty() {
        return Ok(ExecutionResult::error(
            "Nothing specified, nothing added.\nhint: Maybe you wanted to say 'git add .'?\n"
                .to_string(),
        ));
    }

    // Detect flags
    let has_patch = args.iter().any(|a| a == "-p" || a == "--patch");
    let has_all = args.iter().any(|a| a == "-A" || a == "--all");

    // -p / --patch: defer to external git — we can't do interactive staging natively
    if has_patch {
        let mut full_args = vec!["add".to_string()];
        full_args.extend_from_slice(args);
        return super::builtin_git_external(&full_args, runtime);
    }

    let mut index = repo
        .index()
        .map_err(|e| anyhow!("Failed to open index: {}", e))?;

    if has_all {
        // Stage everything: modified, deleted, new files
        stage_all(&repo, &mut index)?;
    } else {
        // Collect paths (skip flag arguments)
        let paths: Vec<&str> = args
            .iter()
            .filter(|a| !a.starts_with('-'))
            .map(String::as_str)
            .collect();

        if paths.is_empty() {
            return Ok(ExecutionResult::error(
                "Nothing specified, nothing added.\n".to_string(),
            ));
        }

        for path_str in &paths {
            if *path_str == "." {
                stage_all(&repo, &mut index)?;
            } else {
                stage_path(&repo, &mut index, cwd, path_str)?;
            }
        }
    }

    index
        .write()
        .map_err(|e| anyhow!("Failed to write index: {}", e))?;

    Ok(ExecutionResult::success(String::new()))
}

/// Stage all changes in the working tree (new, modified, deleted files).
fn stage_all(_repo: &Repository, index: &mut git2::Index) -> Result<()> {
    // Add all tracked and untracked changes
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| anyhow!("Failed to add all files: {}", e))?;

    // Also handle deletions (files removed from the working tree)
    index
        .update_all(["*"].iter(), None)
        .map_err(|e| anyhow!("Failed to update index for deletions: {}", e))?;

    Ok(())
}

/// Stage a single file or directory path relative to the repo root.
fn stage_path(
    repo: &Repository,
    index: &mut git2::Index,
    cwd: &Path,
    path_str: &str,
) -> Result<()> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow!("Repository has no working directory (bare repo?)"))?;

    // Resolve to an absolute path, then make it relative to the workdir.
    // Canonicalize to resolve symlinks (important on macOS where /var -> /private/var).
    let abs_path = if Path::new(path_str).is_absolute() {
        Path::new(path_str).to_path_buf()
    } else {
        cwd.join(path_str)
    };

    // For canonicalization, the path must exist; for deleted files we canonicalize the parent.
    let (canonical_abs, canonical_workdir) = if abs_path.exists() {
        let ca = abs_path.canonicalize().unwrap_or_else(|_| abs_path.clone());
        let cw = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());
        (ca, cw)
    } else {
        // File doesn't exist (deleted) — canonicalize the parent dir instead
        let parent = abs_path.parent().unwrap_or(abs_path.as_path());
        let ca_parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        let file_name = abs_path
            .file_name()
            .ok_or_else(|| anyhow!("Path '{}' has no file name", path_str))?;
        let ca = ca_parent.join(file_name);
        let cw = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());
        (ca, cw)
    };

    let rel_path = canonical_abs
        .strip_prefix(&canonical_workdir)
        .map_err(|_| anyhow!("Path '{}' is outside the repository", path_str))?
        .to_path_buf();

    // Check whether the path exists in the working tree
    if canonical_abs.exists() {
        if canonical_abs.is_dir() {
            // Stage all files under the directory
            let pattern = format!("{}/**", rel_path.display());
            index
                .add_all(
                    [pattern.as_str(), rel_path.to_str().unwrap_or("")].iter(),
                    git2::IndexAddOption::DEFAULT,
                    None,
                )
                .map_err(|e| anyhow!("Failed to add directory '{}': {}", path_str, e))?;
            index
                .update_all([pattern.as_str()].iter(), None)
                .map_err(|e| anyhow!("Failed to update index for '{}': {}", path_str, e))?;
        } else {
            index
                .add_path(&rel_path)
                .map_err(|e| anyhow!("Failed to add '{}': {}", path_str, e))?;
        }
    } else {
        // File deleted — remove it from the index (stage the deletion)
        index
            .remove_path(&rel_path)
            .map_err(|e| anyhow!("Failed to remove '{}' from index: {}", path_str, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn make_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .unwrap();

        dir
    }

    fn runtime_in(path: &std::path::Path) -> Runtime {
        let mut r = Runtime::new();
        r.set_cwd(path.to_path_buf());
        r
    }

    // ── not a repo ────────────────────────────────────────────────────────────

    #[test]
    fn test_git_add_not_a_repo() {
        let mut runtime = runtime_in(std::path::Path::new("/tmp"));
        let result = builtin_git_add(&["file.txt".to_string()], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("not a git repository"));
    }

    // ── no args ───────────────────────────────────────────────────────────────

    #[test]
    fn test_git_add_no_args() {
        let dir = make_git_repo();
        let mut runtime = runtime_in(dir.path());
        let result = builtin_git_add(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("Nothing specified"));
    }

    // ── stage a single file ───────────────────────────────────────────────────

    #[test]
    fn test_git_add_single_file() {
        let dir = make_git_repo();
        let path = dir.path();
        fs::write(path.join("hello.txt"), "hello\n").unwrap();

        let mut runtime = runtime_in(path);
        let result = builtin_git_add(&["hello.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        // Verify the file is staged
        let repo = Repository::discover(path).unwrap();
        let statuses = repo.statuses(None).unwrap();
        let entry = statuses
            .iter()
            .find(|e| e.path() == Some("hello.txt"))
            .expect("hello.txt should be in status");
        assert!(
            entry.status().contains(git2::Status::INDEX_NEW),
            "expected INDEX_NEW, got {:?}",
            entry.status()
        );
    }

    // ── stage dot (all) ───────────────────────────────────────────────────────

    #[test]
    fn test_git_add_dot() {
        let dir = make_git_repo();
        let path = dir.path();
        fs::write(path.join("a.txt"), "a\n").unwrap();
        fs::write(path.join("b.txt"), "b\n").unwrap();

        let mut runtime = runtime_in(path);
        let result = builtin_git_add(&[".".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let repo = Repository::discover(path).unwrap();
        let statuses = repo.statuses(None).unwrap();
        let staged: Vec<_> = statuses
            .iter()
            .filter(|e| e.status().contains(git2::Status::INDEX_NEW))
            .collect();
        assert_eq!(staged.len(), 2, "both files should be staged");
    }

    // ── stage -A (all including deletions) ───────────────────────────────────

    #[test]
    fn test_git_add_dash_a_stages_deletions() {
        let dir = make_git_repo();
        let path = dir.path();

        // Create initial commit with a file
        fs::write(path.join("tracked.txt"), "content\n").unwrap();
        Command::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();

        // Delete the tracked file and add a new one
        fs::remove_file(path.join("tracked.txt")).unwrap();
        fs::write(path.join("new.txt"), "new\n").unwrap();

        let mut runtime = runtime_in(path);
        let result = builtin_git_add(&["-A".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let repo = Repository::discover(path).unwrap();
        let statuses = repo.statuses(None).unwrap();

        let deleted = statuses
            .iter()
            .find(|e| e.path() == Some("tracked.txt"))
            .expect("tracked.txt should be in status");
        assert!(
            deleted.status().contains(git2::Status::INDEX_DELETED),
            "expected INDEX_DELETED, got {:?}",
            deleted.status()
        );

        let added = statuses
            .iter()
            .find(|e| e.path() == Some("new.txt"))
            .expect("new.txt should be in status");
        assert!(
            added.status().contains(git2::Status::INDEX_NEW),
            "expected INDEX_NEW, got {:?}",
            added.status()
        );
    }

    // ── stage deleted file individually ──────────────────────────────────────

    #[test]
    fn test_git_add_deleted_file() {
        let dir = make_git_repo();
        let path = dir.path();

        fs::write(path.join("del.txt"), "bye\n").unwrap();
        Command::new("git")
            .args(["add", "del.txt"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "add del.txt"])
            .current_dir(path)
            .output()
            .unwrap();

        fs::remove_file(path.join("del.txt")).unwrap();

        let mut runtime = runtime_in(path);
        let result = builtin_git_add(&["del.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);

        let repo = Repository::discover(path).unwrap();
        let statuses = repo.statuses(None).unwrap();
        let entry = statuses
            .iter()
            .find(|e| e.path() == Some("del.txt"))
            .expect("del.txt should be in status");
        assert!(
            entry.status().contains(git2::Status::INDEX_DELETED),
            "expected INDEX_DELETED, got {:?}",
            entry.status()
        );
    }
}
