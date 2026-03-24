use crate::executor::ExecutionResult;
use crate::git::{FileStatusType, GitContext};
use crate::runtime::Runtime;
use anyhow::Result;
use nu_ansi_term::Color;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct GitStatusOutput {
    branch: Option<String>,
    tracking: Option<String>,
    ahead: usize,
    behind: usize,
    state: RepoState,
    staged: Vec<FileStatusEntry>,
    unstaged: Vec<FileStatusEntry>,
    untracked: Vec<String>,
    conflicted: Vec<String>,
    summary: StatusSummary,
}

#[derive(Serialize, Deserialize)]
struct FileStatusEntry {
    path: String,
    status: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RepoState {
    Clean,
    Dirty,
}

#[derive(Serialize, Deserialize)]
struct StatusSummary {
    staged_count: usize,
    unstaged_count: usize,
    untracked_count: usize,
    conflicted_count: usize,
}

impl FileStatusType {
    fn as_str(&self) -> &'static str {
        match self {
            FileStatusType::Modified => "modified",
            FileStatusType::Added => "added",
            FileStatusType::Deleted => "deleted",
            FileStatusType::Renamed => "renamed",
            FileStatusType::Typechange => "typechange",
        }
    }
}

pub fn builtin_git_status(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();
    let git_ctx = GitContext::new(cwd);

    if !git_ctx.is_git_repo() {
        return Ok(ExecutionResult::error(
            "fatal: not a git repository\n".to_string(),
        ));
    }

    // Parse arguments
    let json_output = args.iter().any(|arg| arg == "--json") || runtime.agent_mode();

    if json_output {
        return output_json(&git_ctx);
    }

    let mut output = String::new();

    // Branch info
    if let Some(branch) = git_ctx.current_branch() {
        output.push_str(&format!("On branch {}\n", Color::Cyan.paint(&branch)));

        // Ahead/behind info
        if let Some((ahead, behind)) = git_ctx.ahead_behind() {
            if ahead > 0 && behind > 0 {
                output.push_str(&format!(
                    "Your branch and 'origin/{}' have diverged,\n",
                    branch
                ));
                output.push_str(&format!(
                    "and have {} and {} different commits each, respectively.\n",
                    ahead, behind
                ));
            } else if ahead > 0 {
                output.push_str(&format!(
                    "Your branch is ahead of 'origin/{}' by {} commit{}.\n",
                    branch,
                    ahead,
                    if ahead == 1 { "" } else { "s" }
                ));
                output.push_str("  (use \"git push\" to publish your local commits)\n");
            } else if behind > 0 {
                output.push_str(&format!(
                    "Your branch is behind 'origin/{}' by {} commit{}.\n",
                    branch,
                    behind,
                    if behind == 1 { "" } else { "s" }
                ));
                output.push_str("  (use \"git pull\" to update your local branch)\n");
            } else {
                output.push_str(&format!(
                    "Your branch is up to date with 'origin/{}'.\n",
                    branch
                ));
            }
        }
    }

    output.push('\n');

    // Get file statuses
    let staged = git_ctx.staged_files();
    let unstaged = git_ctx.unstaged_files();
    let untracked = git_ctx.untracked_files();

    // Staged changes
    if !staged.is_empty() {
        output.push_str(
            &Color::Green
                .bold()
                .paint("Changes to be committed:\n")
                .to_string(),
        );
        output.push_str("  (use \"git restore --staged <file>...\" to unstage)\n\n");
        for file in &staged {
            let status_text = match file.status {
                FileStatusType::Modified => "modified:",
                FileStatusType::Added => "new file:",
                FileStatusType::Deleted => "deleted:",
                FileStatusType::Renamed => "renamed:",
                FileStatusType::Typechange => "typechange:",
            };
            output.push_str(&format!(
                "\t{} {}\n",
                Color::Green.paint(status_text),
                Color::Green.paint(file.path.display().to_string())
            ));
        }
        output.push('\n');
    }

    // Unstaged changes
    if !unstaged.is_empty() {
        output.push_str("Changes not staged for commit:\n");
        output.push_str("  (use \"git add <file>...\" to update what will be committed)\n");
        output.push_str(
            "  (use \"git restore <file>...\" to discard changes in working directory)\n\n",
        );
        for file in &unstaged {
            let status_text = match file.status {
                FileStatusType::Modified => "modified:",
                FileStatusType::Added => "new file:",
                FileStatusType::Deleted => "deleted:",
                FileStatusType::Renamed => "renamed:",
                FileStatusType::Typechange => "typechange:",
            };
            output.push_str(&format!(
                "\t{} {}\n",
                Color::Red.paint(status_text),
                Color::Red.paint(file.path.display().to_string())
            ));
        }
        output.push('\n');
    }

    // Untracked files
    if !untracked.is_empty() {
        output.push_str("Untracked files:\n");
        output.push_str("  (use \"git add <file>...\" to include in what will be committed)\n\n");
        for file in &untracked {
            output.push_str(&format!(
                "\t{}\n",
                Color::Red.paint(file.display().to_string())
            ));
        }
        output.push('\n');
    }

    // Clean state
    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() {
        output.push_str("nothing to commit, working tree clean\n");
    }

    Ok(ExecutionResult::success(output))
}

fn output_json(git_ctx: &GitContext) -> Result<ExecutionResult> {
    let branch = git_ctx.current_branch();
    let tracking = git_ctx.tracking_branch();
    let (ahead, behind) = git_ctx.ahead_behind().unwrap_or((0, 0));

    // Optimized: Get all statuses in a single pass instead of 4 separate calls
    let (staged, unstaged, untracked, conflicted) = git_ctx.all_file_statuses();

    let state = if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() {
        RepoState::Clean
    } else {
        RepoState::Dirty
    };

    let staged_entries: Vec<FileStatusEntry> = staged
        .iter()
        .map(|f| FileStatusEntry {
            path: f.path.display().to_string(),
            status: f.status.as_str().to_string(),
        })
        .collect();

    let unstaged_entries: Vec<FileStatusEntry> = unstaged
        .iter()
        .map(|f| FileStatusEntry {
            path: f.path.display().to_string(),
            status: f.status.as_str().to_string(),
        })
        .collect();

    let untracked_strings: Vec<String> =
        untracked.iter().map(|p| p.display().to_string()).collect();

    let conflicted_strings: Vec<String> =
        conflicted.iter().map(|p| p.display().to_string()).collect();

    let output = GitStatusOutput {
        branch,
        tracking,
        ahead,
        behind,
        state,
        staged: staged_entries,
        unstaged: unstaged_entries,
        untracked: untracked_strings.clone(),
        conflicted: conflicted_strings.clone(),
        summary: StatusSummary {
            staged_count: staged.len(),
            unstaged_count: unstaged.len(),
            untracked_count: untracked_strings.len(),
            conflicted_count: conflicted_strings.len(),
        },
    };

    let json_value = serde_json::to_value(&output)?;
    Ok(ExecutionResult {
        output: crate::executor::Output::Structured(json_value),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_git_status_not_a_repo() {
        let mut runtime = Runtime::new();
        // Set to a non-git directory
        runtime.set_cwd(std::path::PathBuf::from("/tmp"));
        let result = builtin_git_status(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("not a git repository"));
    }

    #[test]
    fn test_git_status_clean_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Configure git to avoid warnings
        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test\n").unwrap();
        Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.to_path_buf());
        let result = builtin_git_status(&[], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result
            .stdout()
            .contains("nothing to commit, working tree clean"));
    }

    #[test]
    fn test_git_status_json_output() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test\n").unwrap();
        Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.to_path_buf());
        let result = builtin_git_status(&["--json".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);

        // Verify JSON is valid
        let json: GitStatusOutput = serde_json::from_str(&result.stdout()).unwrap();
        assert_eq!(json.state, RepoState::Clean);
        assert_eq!(json.summary.staged_count, 0);
        assert_eq!(json.summary.unstaged_count, 0);
        assert_eq!(json.summary.untracked_count, 0);
    }

    #[test]
    fn test_git_status_with_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test\n").unwrap();
        Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Make changes
        fs::write(repo_path.join("README.md"), "# Test Modified\n").unwrap();
        fs::write(repo_path.join("new_file.txt"), "New content\n").unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.to_path_buf());
        let result = builtin_git_status(&[], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("modified"));
        assert!(result.stdout().contains("README.md"));
        assert!(result.stdout().contains("Untracked files"));
        assert!(result.stdout().contains("new_file.txt"));
    }

    #[test]
    fn test_git_status_json_with_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("README.md"), "# Test\n").unwrap();
        Command::new("git")
            .args(&["add", "README.md"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Make and stage changes
        fs::write(repo_path.join("staged.txt"), "Staged content\n").unwrap();
        Command::new("git")
            .args(&["add", "staged.txt"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Make unstaged changes
        fs::write(repo_path.join("README.md"), "# Test Modified\n").unwrap();

        // Create untracked file
        fs::write(repo_path.join("untracked.txt"), "Untracked\n").unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(repo_path.to_path_buf());
        let result = builtin_git_status(&["--json".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);

        // Verify JSON structure
        let json: GitStatusOutput = serde_json::from_str(&result.stdout()).unwrap();
        assert_eq!(json.state, RepoState::Dirty);
        assert_eq!(json.summary.staged_count, 1);
        assert_eq!(json.summary.unstaged_count, 1);
        assert_eq!(json.summary.untracked_count, 1);

        // Verify file entries
        assert_eq!(json.staged[0].path, "staged.txt");
        assert_eq!(json.staged[0].status, "added");
        assert!(json
            .unstaged
            .iter()
            .any(|f| f.path == "README.md" && f.status == "modified"));
        assert!(json.untracked.contains(&"untracked.txt".to_string()));
    }
}
