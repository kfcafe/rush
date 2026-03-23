use git2::{Repository, StatusOptions};
use std::path::{Path, PathBuf};

pub struct GitContext {
    repo: Option<Repository>,
}

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: PathBuf,
    pub status: FileStatusType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatusType {
    Modified,
    Added,
    Deleted,
    Renamed,
    Typechange,
}

impl GitContext {
    pub fn new(path: &Path) -> Self {
        let repo = Repository::discover(path).ok();
        Self { repo }
    }

    pub fn is_git_repo(&self) -> bool {
        self.repo.is_some()
    }

    pub fn current_branch(&self) -> Option<String> {
        let repo = self.repo.as_ref()?;
        let head = repo.head().ok()?;
        let branch_name = head.shorthand()?;
        Some(branch_name.to_string())
    }

    pub fn is_dirty(&self) -> bool {
        if let Some(repo) = &self.repo {
            if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(true)))
            {
                return !statuses.is_empty();
            }
        }
        false
    }

    pub fn ahead_behind(&self) -> Option<(usize, usize)> {
        let repo = self.repo.as_ref()?;
        let head = repo.head().ok()?;
        let local_oid = head.target()?;

        // Get upstream branch
        let branch = repo
            .find_branch(head.shorthand()?, git2::BranchType::Local)
            .ok()?;
        let upstream = branch.upstream().ok()?;
        let upstream_oid = upstream.get().target()?;

        // Calculate ahead/behind
        let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid).ok()?;
        Some((ahead, behind))
    }

    pub fn status_summary(&self) -> GitStatus {
        if !self.is_git_repo() {
            return GitStatus::NotGit;
        }

        let branch = self.current_branch();
        let dirty = self.is_dirty();
        let ahead_behind = self.ahead_behind();

        GitStatus::InRepo {
            branch,
            dirty,
            ahead_behind,
        }
    }

    /// Get all file statuses in a single pass (optimized for git_status builtin)
    /// This is significantly faster than calling staged/unstaged/untracked separately
    pub fn all_file_statuses(
        &self,
    ) -> (Vec<FileStatus>, Vec<FileStatus>, Vec<PathBuf>, Vec<PathBuf>) {
        let repo = match &self.repo {
            Some(r) => r,
            None => return (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();
        let mut conflicted = Vec::new();

        if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(true))) {
            for entry in statuses.iter() {
                let status = entry.status();
                let path = entry.path().unwrap_or("");

                // Check staged files
                if status.is_index_new() {
                    staged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Added,
                    });
                } else if status.is_index_modified() {
                    staged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Modified,
                    });
                } else if status.is_index_deleted() {
                    staged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Deleted,
                    });
                } else if status.is_index_renamed() {
                    staged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Renamed,
                    });
                } else if status.is_index_typechange() {
                    staged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Typechange,
                    });
                }

                // Check unstaged files
                if status.is_wt_modified() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Modified,
                    });
                } else if status.is_wt_deleted() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Deleted,
                    });
                } else if status.is_wt_typechange() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Typechange,
                    });
                } else if status.is_wt_renamed() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(path),
                        status: FileStatusType::Renamed,
                    });
                }

                // Check untracked files
                if status.is_wt_new() {
                    untracked.push(PathBuf::from(path));
                }

                // Check conflicted files
                if status.is_conflicted() {
                    conflicted.push(PathBuf::from(path));
                }
            }
        }

        (staged, unstaged, untracked, conflicted)
    }

    pub fn staged_files(&self) -> Vec<FileStatus> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut staged = Vec::new();
        if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(false))) {
            for entry in statuses.iter() {
                let status = entry.status();
                let path = entry.path().unwrap_or("").to_string();

                if status.is_index_new() {
                    staged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Added,
                    });
                } else if status.is_index_modified() {
                    staged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Modified,
                    });
                } else if status.is_index_deleted() {
                    staged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Deleted,
                    });
                } else if status.is_index_renamed() {
                    staged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Renamed,
                    });
                } else if status.is_index_typechange() {
                    staged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Typechange,
                    });
                }
            }
        }
        staged
    }

    pub fn unstaged_files(&self) -> Vec<FileStatus> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut unstaged = Vec::new();
        if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(false))) {
            for entry in statuses.iter() {
                let status = entry.status();
                let path = entry.path().unwrap_or("").to_string();

                if status.is_wt_modified() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Modified,
                    });
                } else if status.is_wt_deleted() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Deleted,
                    });
                } else if status.is_wt_typechange() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Typechange,
                    });
                } else if status.is_wt_renamed() {
                    unstaged.push(FileStatus {
                        path: PathBuf::from(&path),
                        status: FileStatusType::Renamed,
                    });
                }
            }
        }
        unstaged
    }

    pub fn untracked_files(&self) -> Vec<PathBuf> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut untracked = Vec::new();
        if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(true))) {
            for entry in statuses.iter() {
                let status = entry.status();
                if status.is_wt_new() {
                    if let Some(path) = entry.path() {
                        untracked.push(PathBuf::from(path));
                    }
                }
            }
        }
        untracked
    }

    pub fn conflicted_files(&self) -> Vec<PathBuf> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut conflicted = Vec::new();
        if let Ok(statuses) = repo.statuses(Some(StatusOptions::new().include_untracked(false))) {
            for entry in statuses.iter() {
                let status = entry.status();
                if status.is_conflicted() {
                    if let Some(path) = entry.path() {
                        conflicted.push(PathBuf::from(path));
                    }
                }
            }
        }
        conflicted
    }

    pub fn tracking_branch(&self) -> Option<String> {
        let repo = self.repo.as_ref()?;
        let head = repo.head().ok()?;
        let branch = repo
            .find_branch(head.shorthand()?, git2::BranchType::Local)
            .ok()?;
        let upstream = branch.upstream().ok()?;
        upstream.name().ok()?.map(|s| s.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum GitStatus {
    NotGit,
    InRepo {
        branch: Option<String>,
        dirty: bool,
        ahead_behind: Option<(usize, usize)>,
    },
}

impl GitStatus {
    pub fn prompt_string(&self) -> String {
        match self {
            GitStatus::NotGit => String::new(),
            GitStatus::InRepo {
                branch,
                dirty,
                ahead_behind,
            } => {
                let mut parts = Vec::new();

                if let Some(branch_name) = branch {
                    parts.push(branch_name.clone());
                }

                if *dirty {
                    parts.push("✗".to_string());
                } else {
                    parts.push("✓".to_string());
                }

                if let Some((ahead, behind)) = ahead_behind {
                    if *ahead > 0 {
                        parts.push(format!("↑{}", ahead));
                    }
                    if *behind > 0 {
                        parts.push(format!("↓{}", behind));
                    }
                }

                format!("({})", parts.join(" "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_context_not_a_repo() {
        let ctx = GitContext::new(Path::new("/tmp"));
        // /tmp might or might not be a git repo, just test it doesn't crash
        let _ = ctx.is_git_repo();
    }

    #[test]
    fn test_status_prompt_not_git() {
        let status = GitStatus::NotGit;
        assert_eq!(status.prompt_string(), "");
    }

    #[test]
    fn test_status_prompt_clean() {
        let status = GitStatus::InRepo {
            branch: Some("main".to_string()),
            dirty: false,
            ahead_behind: None,
        };
        assert_eq!(status.prompt_string(), "(main ✓)");
    }

    #[test]
    fn test_status_prompt_dirty_with_ahead() {
        let status = GitStatus::InRepo {
            branch: Some("feature".to_string()),
            dirty: true,
            ahead_behind: Some((2, 0)),
        };
        assert_eq!(status.prompt_string(), "(feature ✗ ↑2)");
    }
}
