use crate::executor::ExecutionResult;
use crate::git::GitContext;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use git2::{Commit as Git2Commit, DiffOptions, Repository};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct Commit {
    hash: String,
    short_hash: String,
    author: String,
    author_email: String,
    date: String, // ISO 8601
    timestamp: i64,
    message: String,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
}

pub fn builtin_git_log(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let cwd = runtime.get_cwd();
    let git_ctx = GitContext::new(cwd);

    if !git_ctx.is_git_repo() {
        return Ok(ExecutionResult::error(
            "fatal: not a git repository\n".to_string(),
        ));
    }

    // Parse arguments
    let mut json_output = runtime.agent_mode();
    let mut limit: Option<usize> = None;
    let mut since: Option<String> = None;
    let mut until: Option<String> = None;
    let mut grep_pattern: Option<String> = None;
    let mut path_filter: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_output = true,
            "-n" => {
                if i + 1 < args.len() {
                    limit = Some(
                        args[i + 1]
                            .parse()
                            .map_err(|_| anyhow!("Invalid number for -n"))?,
                    );
                    i += 1;
                } else {
                    return Err(anyhow!("-n requires an argument"));
                }
            }
            "--since" => {
                if i + 1 < args.len() {
                    since = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err(anyhow!("--since requires an argument"));
                }
            }
            "--until" => {
                if i + 1 < args.len() {
                    until = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err(anyhow!("--until requires an argument"));
                }
            }
            "--grep" => {
                if i + 1 < args.len() {
                    grep_pattern = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err(anyhow!("--grep requires an argument"));
                }
            }
            arg if !arg.starts_with('-') => {
                // This is a path filter
                path_filter = Some(arg.to_string());
            }
            _ => return Err(anyhow!("Unknown option: {}", args[i])),
        }
        i += 1;
    }

    // Open the repository
    let repo =
        Repository::discover(cwd).map_err(|e| anyhow!("Failed to open repository: {}", e))?;

    // Get HEAD commit
    let head = repo
        .head()
        .map_err(|e| anyhow!("Failed to get HEAD: {}", e))?;
    let head_oid = head.target().ok_or_else(|| anyhow!("HEAD has no target"))?;

    // Walk the commits
    let mut revwalk = repo
        .revwalk()
        .map_err(|e| anyhow!("Failed to create revwalk: {}", e))?;
    revwalk
        .push(head_oid)
        .map_err(|e| anyhow!("Failed to push HEAD: {}", e))?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .map_err(|e| anyhow!("Failed to set sorting: {}", e))?;

    let mut commits = Vec::new();
    let limit = limit.unwrap_or(100); // Default limit to prevent very long output
    let mut count = 0;

    // Parse --since and --until if provided
    let since_timestamp = if let Some(since_str) = &since {
        parse_relative_time(since_str)?
    } else {
        None
    };
    let until_timestamp = if let Some(until_str) = &until {
        parse_relative_time(until_str)?
    } else {
        None
    };

    for oid_result in revwalk {
        if count >= limit {
            break;
        }

        let oid = oid_result.map_err(|e| anyhow!("Failed to get commit OID: {}", e))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| anyhow!("Failed to find commit: {}", e))?;

        // Apply --since filter (commits must be newer than since_ts)
        if let Some(since_ts) = since_timestamp {
            if commit.time().seconds() < since_ts {
                continue;
            }
        }

        // Apply --until filter (commits must be older than until_ts)
        if let Some(until_ts) = until_timestamp {
            if commit.time().seconds() > until_ts {
                continue;
            }
        }

        // Apply --grep filter
        if let Some(pattern) = &grep_pattern {
            let message = commit.message().unwrap_or("");
            if !message.contains(pattern.as_str()) {
                continue;
            }
        }

        // Apply path filter
        if let Some(path) = &path_filter {
            if !commit_affects_path(&repo, &commit, path)? {
                continue;
            }
        }

        // Compute diff stats
        let (files_changed, insertions, deletions) = compute_diff_stats(&repo, &commit)?;

        let author = commit.author();
        let timestamp = commit.time().seconds();
        let datetime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);
        let date_str = format_datetime(datetime);

        let commit_info = Commit {
            hash: format!("{}", oid),
            short_hash: format!("{:.7}", oid),
            author: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            date: date_str,
            timestamp,
            message: commit.message().unwrap_or("").trim().to_string(),
            files_changed,
            insertions,
            deletions,
        };

        commits.push(commit_info);
        count += 1;
    }

    // Generate output
    let output = if json_output {
        serde_json::to_string_pretty(&commits)
            .map_err(|e| anyhow!("Failed to serialize to JSON: {}", e))?
            + "\n"
    } else {
        // Human-readable output (like git log --oneline)
        commits
            .iter()
            .map(|c| {
                let message_line = c.message.lines().next().unwrap_or("");
                format!("{} {}", c.short_hash, message_line)
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };

    Ok(ExecutionResult::success(output))
}

fn parse_relative_time(time_str: &str) -> Result<Option<i64>> {
    // Simple parsing for common patterns
    // Full implementation would use a datetime parser
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if time_str.ends_with(" days ago") || time_str.ends_with(" day ago") {
        let parts: Vec<&str> = time_str.split_whitespace().collect();
        if let Ok(days) = parts[0].parse::<i64>() {
            return Ok(Some(now - days * 86400));
        }
    } else if time_str.ends_with(" weeks ago") || time_str.ends_with(" week ago") {
        let parts: Vec<&str> = time_str.split_whitespace().collect();
        if let Ok(weeks) = parts[0].parse::<i64>() {
            return Ok(Some(now - weeks * 7 * 86400));
        }
    } else if time_str.ends_with(" months ago") || time_str.ends_with(" month ago") {
        let parts: Vec<&str> = time_str.split_whitespace().collect();
        if let Ok(months) = parts[0].parse::<i64>() {
            return Ok(Some(now - months * 30 * 86400));
        }
    }

    // Try ISO 8601 format (basic support)
    // For production, use chrono or similar
    Ok(None)
}

fn format_datetime(datetime: SystemTime) -> String {
    // Use chrono for proper formatting
    use chrono::{DateTime, Utc};
    let dt: DateTime<Utc> = datetime.into();
    dt.to_rfc3339()
}

fn commit_affects_path(repo: &Repository, commit: &Git2Commit, path: &str) -> Result<bool> {
    // Get the tree for this commit
    let commit_tree = commit
        .tree()
        .map_err(|e| anyhow!("Failed to get commit tree: {}", e))?;

    // Get parent commit if it exists
    if commit.parent_count() == 0 {
        // Initial commit - check if path exists in this commit
        return Ok(commit_tree.get_path(std::path::Path::new(path)).is_ok());
    }

    let parent = commit
        .parent(0)
        .map_err(|e| anyhow!("Failed to get parent commit: {}", e))?;
    let parent_tree = parent
        .tree()
        .map_err(|e| anyhow!("Failed to get parent tree: {}", e))?;

    // Create diff between parent and current commit
    let mut diff_opts = DiffOptions::new();
    diff_opts.pathspec(path);

    let diff = repo
        .diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), Some(&mut diff_opts))
        .map_err(|e| anyhow!("Failed to create diff: {}", e))?;

    Ok(diff.deltas().len() > 0)
}

fn compute_diff_stats(repo: &Repository, commit: &Git2Commit) -> Result<(usize, usize, usize)> {
    let commit_tree = commit
        .tree()
        .map_err(|e| anyhow!("Failed to get commit tree: {}", e))?;

    // For initial commit, count files in the tree
    if commit.parent_count() == 0 {
        let mut file_count = 0;
        let mut insertions = 0;

        commit_tree
            .walk(git2::TreeWalkMode::PreOrder, |_, entry| {
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    file_count += 1;
                    // Try to get blob size as insertion count
                    if let Ok(object) = entry.to_object(repo) {
                        if let Some(blob) = object.as_blob() {
                            // Count lines in the blob
                            let content = blob.content();
                            let lines = content.iter().filter(|&&b| b == b'\n').count();
                            insertions += lines;
                        }
                    }
                }
                git2::TreeWalkResult::Ok
            })
            .ok();

        return Ok((file_count, insertions, 0));
    }

    // Get parent and compute diff
    let parent = commit
        .parent(0)
        .map_err(|e| anyhow!("Failed to get parent commit: {}", e))?;
    let parent_tree = parent
        .tree()
        .map_err(|e| anyhow!("Failed to get parent tree: {}", e))?;

    let diff = repo
        .diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)
        .map_err(|e| anyhow!("Failed to create diff: {}", e))?;

    let stats = diff
        .stats()
        .map_err(|e| anyhow!("Failed to get diff stats: {}", e))?;

    Ok((stats.files_changed(), stats.insertions(), stats.deletions()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_log_not_a_repo() {
        let mut runtime = Runtime::new();
        runtime.set_cwd(std::path::PathBuf::from("/tmp"));
        let result = builtin_git_log(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("not a git repository"));
    }

    #[test]
    fn test_parse_relative_time_days() {
        let result = parse_relative_time("7 days ago").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_relative_time_weeks() {
        let result = parse_relative_time("2 weeks ago").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_relative_time_months() {
        let result = parse_relative_time("3 months ago").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_relative_time_invalid() {
        let result = parse_relative_time("invalid").unwrap();
        assert!(result.is_none());
    }
}
