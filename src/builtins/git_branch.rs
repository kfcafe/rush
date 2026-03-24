use crate::executor::ExecutionResult;
use crate::git::GitContext;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use git2::{BranchType, Repository};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct BranchInfo {
    name: String,
    current: bool,
}

pub fn builtin_git_branch(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();
    let git_ctx = GitContext::new(cwd);

    if !git_ctx.is_git_repo() {
        return Ok(ExecutionResult::error(
            "fatal: not a git repository\n".to_string(),
        ));
    }

    let repo =
        Repository::discover(cwd).map_err(|e| anyhow!("Failed to open repository: {}", e))?;

    // Parse arguments
    let mut delete_branch: Option<String> = None;
    let mut rename_old: Option<String> = None;
    let mut rename_new: Option<String> = None;
    let mut new_branch: Option<String> = None;
    let mut json_output = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_output = true,
            "-d" | "--delete" => {
                i += 1;
                if i >= args.len() {
                    return Ok(ExecutionResult::error(
                        "fatal: branch name required\n".to_string(),
                    ));
                }
                delete_branch = Some(args[i].clone());
            }
            "-m" | "--move" => {
                i += 1;
                if i >= args.len() {
                    return Ok(ExecutionResult::error(
                        "fatal: branch name required\n".to_string(),
                    ));
                }
                rename_old = Some(args[i].clone());
                i += 1;
                if i >= args.len() {
                    return Ok(ExecutionResult::error(
                        "fatal: branch name required\n".to_string(),
                    ));
                }
                rename_new = Some(args[i].clone());
            }
            arg if !arg.starts_with('-') => {
                new_branch = Some(arg.to_string());
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

    // Delete branch
    if let Some(branch_name) = delete_branch {
        let mut branch = repo
            .find_branch(&branch_name, BranchType::Local)
            .map_err(|_| anyhow!("error: branch '{}' not found", branch_name))?;

        // Check we're not deleting the current branch
        if branch.is_head() {
            return Ok(ExecutionResult::error(format!(
                "error: Cannot delete branch '{}' checked out at '{}'\n",
                branch_name,
                cwd.display()
            )));
        }

        branch
            .delete()
            .map_err(|e| anyhow!("error: Failed to delete branch: {}", e))?;

        return Ok(ExecutionResult::success(format!(
            "Deleted branch {}.\n",
            branch_name
        )));
    }

    // Rename branch
    if let (Some(old), Some(new)) = (rename_old, rename_new) {
        let mut branch = repo
            .find_branch(&old, BranchType::Local)
            .map_err(|_| anyhow!("error: branch '{}' not found.", old))?;

        branch
            .rename(&new, false)
            .map_err(|e| anyhow!("error: Failed to rename branch: {}", e))?;

        return Ok(ExecutionResult::success(String::new()));
    }

    // Create new branch at HEAD
    if let Some(branch_name) = new_branch {
        let head = repo
            .head()
            .map_err(|e| anyhow!("Failed to get HEAD: {}", e))?;
        let head_commit = head
            .peel_to_commit()
            .map_err(|e| anyhow!("Failed to peel HEAD to commit: {}", e))?;

        repo.branch(&branch_name, &head_commit, false)
            .map_err(|e| anyhow!("error: Failed to create branch '{}': {}", branch_name, e))?;

        return Ok(ExecutionResult::success(String::new()));
    }

    // List branches
    let current = git_ctx.current_branch();
    let branches = repo
        .branches(Some(BranchType::Local))
        .map_err(|e| anyhow!("Failed to list branches: {}", e))?;

    let mut branch_list: Vec<BranchInfo> = Vec::new();
    for branch_result in branches {
        let (branch, _) = branch_result.map_err(|e| anyhow!("Failed to read branch: {}", e))?;
        if let Ok(Some(name)) = branch.name() {
            let is_current = current.as_deref() == Some(name);
            branch_list.push(BranchInfo {
                name: name.to_string(),
                current: is_current,
            });
        }
    }

    if json_output {
        let json_value = serde_json::to_value(&branch_list)?;
        return Ok(ExecutionResult {
            output: crate::executor::Output::Structured(json_value),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        });
    }

    let mut output = String::new();
    for b in &branch_list {
        if b.current {
            output.push_str(&format!("* {}\n", b.name));
        } else {
            output.push_str(&format!("  {}\n", b.name));
        }
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_repo_with_commit() -> (TempDir, std::path::PathBuf) {
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
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_git_branch_not_a_repo() {
        let mut runtime = Runtime::new();
        runtime.set_cwd(std::path::PathBuf::from("/tmp"));
        let result = builtin_git_branch(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
    }

    #[test]
    fn test_git_branch_list() {
        let (temp_dir, repo_path) = setup_repo_with_commit();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path);
        let result = builtin_git_branch(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        // Should list at least one branch with * marker
        assert!(result.stdout().contains('*'));
        drop(temp_dir);
    }

    #[test]
    fn test_git_branch_create() {
        let (temp_dir, repo_path) = setup_repo_with_commit();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.clone());
        let result = builtin_git_branch(&["feature".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Verify branch exists
        let list_result = builtin_git_branch(&[], &mut runtime).unwrap();
        assert!(list_result.stdout().contains("feature"));
        drop(temp_dir);
    }

    #[test]
    fn test_git_branch_delete() {
        let (temp_dir, repo_path) = setup_repo_with_commit();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.clone());

        // Create then delete a branch
        builtin_git_branch(&["to-delete".to_string()], &mut runtime).unwrap();
        let result =
            builtin_git_branch(&["-d".to_string(), "to-delete".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Deleted"));
        drop(temp_dir);
    }

    #[test]
    fn test_git_branch_rename() {
        let (temp_dir, repo_path) = setup_repo_with_commit();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.clone());

        // Create a branch, then rename it
        builtin_git_branch(&["old-name".to_string()], &mut runtime).unwrap();
        let result = builtin_git_branch(
            &[
                "-m".to_string(),
                "old-name".to_string(),
                "new-name".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        let list_result = builtin_git_branch(&[], &mut runtime).unwrap();
        assert!(list_result.stdout().contains("new-name"));
        assert!(!list_result.stdout().contains("old-name"));
        drop(temp_dir);
    }

    #[test]
    fn test_git_branch_json() {
        let (temp_dir, repo_path) = setup_repo_with_commit();
        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path);
        let result = builtin_git_branch(&["--json".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        let parsed: Vec<BranchInfo> = serde_json::from_str(&result.stdout()).unwrap();
        assert!(!parsed.is_empty());
        assert!(parsed.iter().any(|b| b.current));
        drop(temp_dir);
    }
}
