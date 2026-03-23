use crate::executor::ExecutionResult;
use crate::git::GitContext;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use git2::Repository;

pub fn builtin_git_commit(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();
    let git_ctx = GitContext::new(cwd);

    if !git_ctx.is_git_repo() {
        return Ok(ExecutionResult::error(
            "fatal: not a git repository\n".to_string(),
        ));
    }

    // Parse arguments
    let mut message: Option<String> = None;
    let mut stage_all = false; // -a flag
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-m" => {
                i += 1;
                if i >= args.len() {
                    return Ok(ExecutionResult::error(
                        "error: switch `m' requires a value\n".to_string(),
                    ));
                }
                message = Some(args[i].clone());
            }
            "-a" => stage_all = true,
            // Support combined flags: -am or -ma
            "-am" | "-ma" => {
                stage_all = true;
                i += 1;
                if i >= args.len() {
                    return Ok(ExecutionResult::error(
                        "error: switch `m' requires a value\n".to_string(),
                    ));
                }
                message = Some(args[i].clone());
            }
            arg => {
                return Ok(ExecutionResult::error(format!(
                    "error: unknown switch `{}'\n",
                    arg
                )));
            }
        }
        i += 1;
    }

    let message = match message {
        Some(m) => m,
        None => {
            return Ok(ExecutionResult::error(
                "Aborting commit due to empty commit message.\n".to_string(),
            ));
        }
    };

    if message.trim().is_empty() {
        return Ok(ExecutionResult::error(
            "Aborting commit due to empty commit message.\n".to_string(),
        ));
    }

    let repo =
        Repository::discover(cwd).map_err(|e| anyhow!("Failed to open repository: {}", e))?;

    // Stage all tracked modified/deleted files if -a
    if stage_all {
        let mut index = repo
            .index()
            .map_err(|e| anyhow!("Failed to get index: {}", e))?;
        index
            .update_all(["*"].iter(), None)
            .map_err(|e| anyhow!("Failed to stage tracked files: {}", e))?;
        index
            .write()
            .map_err(|e| anyhow!("Failed to write index: {}", e))?;
    }

    // Check if there's anything staged to commit
    let staged = git_ctx.staged_files();
    if staged.is_empty() {
        // After staging all, check again via index
        if !stage_all {
            let unstaged = git_ctx.unstaged_files();
            if !unstaged.is_empty() {
                return Ok(ExecutionResult::error(
                    "error: nothing added to commit but untracked files present (use \"git add\" to track)\n".to_string(),
                ));
            }
            return Ok(ExecutionResult::success(
                "nothing to commit, working tree clean\n".to_string(),
            ));
        }
    }

    // Write tree from index
    let mut index = repo
        .index()
        .map_err(|e| anyhow!("Failed to get index: {}", e))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| anyhow!("Failed to write tree: {}", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| anyhow!("Failed to find tree: {}", e))?;

    // Get author/committer signature from repo config (uses user.name/user.email)
    let sig = repo.signature().map_err(|e| {
        anyhow!(
            "Failed to get signature: {}. Set user.name and user.email in git config.",
            e
        )
    })?;

    // Get parent commit (if HEAD exists)
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = head_commit.iter().collect();

    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
        .map_err(|e| anyhow!("Failed to create commit: {}", e))?;

    let short_hash = format!("{:.7}", commit_oid);
    let branch = git_ctx
        .current_branch()
        .unwrap_or_else(|| "HEAD".to_string());
    let first_line = message.lines().next().unwrap_or("").to_string();

    let output = format!("[{} {}] {}\n", branch, short_hash, first_line);
    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_repo_with_staged_file() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        fs::write(repo_path.join("file.txt"), "hello\n").unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_git_commit_not_a_repo() {
        let mut runtime = Runtime::new();
        runtime.set_cwd(std::path::PathBuf::from("/tmp"));
        let result =
            builtin_git_commit(&["-m".to_string(), "msg".to_string()], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
    }

    #[test]
    fn test_git_commit_missing_message() {
        let (temp_dir, repo_path) = setup_repo_with_staged_file();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path);
        let result = builtin_git_commit(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        drop(temp_dir);
    }

    #[test]
    fn test_git_commit_success() {
        let (temp_dir, repo_path) = setup_repo_with_staged_file();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path);
        let result = builtin_git_commit(
            &["-m".to_string(), "Initial commit".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Initial commit"));
        drop(temp_dir);
    }

    #[test]
    fn test_git_commit_a_flag() {
        let (temp_dir, repo_path) = setup_repo_with_staged_file();

        // Create initial commit first
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Modify tracked file without staging
        fs::write(repo_path.join("file.txt"), "modified\n").unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path);
        let result = builtin_git_commit(
            &["-a".to_string(), "-m".to_string(), "update".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("update"));
        drop(temp_dir);
    }
}
