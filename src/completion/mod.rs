// Tab completion system
mod engine;

use crate::builtins::Builtins;
use crate::runtime::Runtime;
use engine::CompletionEngine;
use ignore::WalkBuilder;
use reedline::{Completer as ReedlineCompleter, Span, Suggestion};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache entry with timestamp for expiration
struct CacheEntry<T> {
    data: T,
    timestamp: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            timestamp: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.timestamp.elapsed() > ttl
    }
}

/// Tab completion system with context-aware suggestions
pub struct Completer {
    /// Reference to builtins for command completion
    #[allow(dead_code)]
    builtins: Arc<Builtins>,
    /// Reference to runtime for functions and variables
    runtime: Arc<RwLock<Runtime>>,
    /// Core completion engine with caching and fuzzy matching
    engine: CompletionEngine,
    /// Cached git branches
    git_branches_cache: Arc<RwLock<Option<CacheEntry<Vec<String>>>>>,
    /// Cache TTL
    cache_ttl: Duration,
    /// Common flags for builtins
    builtin_flags: HashMap<String, Vec<String>>,
}

impl Completer {
    pub fn new(builtins: Arc<Builtins>, runtime: Arc<RwLock<Runtime>>) -> Self {
        let mut builtin_flags = HashMap::new();

        // Define common flags for builtins
        builtin_flags.insert(
            "ls".to_string(),
            vec![
                "-l".to_string(),
                "-a".to_string(),
                "-h".to_string(),
                "-R".to_string(),
                "-t".to_string(),
                "-r".to_string(),
                "--long".to_string(),
                "--all".to_string(),
                "--human-readable".to_string(),
            ],
        );

        builtin_flags.insert(
            "grep".to_string(),
            vec![
                "-i".to_string(),
                "-r".to_string(),
                "-n".to_string(),
                "-v".to_string(),
                "-w".to_string(),
                "-E".to_string(),
                "--ignore-case".to_string(),
                "--recursive".to_string(),
                "--line-number".to_string(),
                "--invert-match".to_string(),
            ],
        );

        builtin_flags.insert(
            "find".to_string(),
            vec![
                "-name".to_string(),
                "-type".to_string(),
                "-size".to_string(),
                "-mtime".to_string(),
                "-exec".to_string(),
                "-print".to_string(),
            ],
        );

        builtin_flags.insert(
            "cat".to_string(),
            vec![
                "-n".to_string(),
                "-b".to_string(),
                "-s".to_string(),
                "--number".to_string(),
                "--number-nonblank".to_string(),
            ],
        );

        Self {
            builtins,
            runtime,
            engine: CompletionEngine::new(),
            git_branches_cache: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(300), // 5 minutes
            builtin_flags,
        }
    }

    /// Get all available commands (builtins + PATH + user functions)
    fn get_all_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();

        // Add builtin commands
        commands.extend(self.get_builtin_commands());

        // Add PATH executables (from engine cache)
        let path_commands: Vec<String> = self
            .engine
            .complete_commands("", 500)
            .into_iter()
            .map(|(cmd, _)| cmd)
            .collect();
        commands.extend(path_commands);

        // Add user-defined functions
        commands.extend(self.get_user_functions());

        commands.sort();
        commands.dedup();
        commands
    }

    /// Get builtin command names
    fn get_builtin_commands(&self) -> Vec<String> {
        vec![
            "cd".to_string(),
            "pwd".to_string(),
            "echo".to_string(),
            "exit".to_string(),
            "export".to_string(),
            "cat".to_string(),
            "find".to_string(),
            "ls".to_string(),
            "git-status".to_string(),
            "grep".to_string(),
        ]
    }

    /// Get PATH executables using engine
    fn get_path_executables(&self) -> Vec<String> {
        self.engine
            .complete_commands("", 500)
            .into_iter()
            .map(|(cmd, _)| cmd)
            .collect()
    }

    /// Scan PATH directories for executables (now handled by engine)

    /// Get user-defined function names
    fn get_user_functions(&self) -> Vec<String> {
        let runtime = self.runtime.read().unwrap();
        runtime.get_function_names()
    }

    /// Complete file/directory paths with gitignore support
    fn complete_path(&self, prefix: &str) -> Vec<String> {
        let (dir, partial) = if prefix.contains('/') {
            let path = Path::new(prefix);
            let dir = path.parent().unwrap_or(Path::new("."));
            let partial = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            (dir.to_path_buf(), partial)
        } else {
            (PathBuf::from("."), prefix)
        };

        // Expand tilde
        let dir = if dir.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                home.join(dir.strip_prefix("~").unwrap())
            } else {
                dir
            }
        } else {
            dir
        };

        // Get current working directory for relative paths
        let base_dir = if dir.is_absolute() {
            dir.clone()
        } else {
            let runtime = self.runtime.read().unwrap();
            runtime.get_cwd().join(&dir)
        };

        let mut matches = Vec::new();

        // Use ignore crate to respect .gitignore
        let walker = WalkBuilder::new(&base_dir)
            .max_depth(Some(1))
            .hidden(false) // Show hidden files
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.flatten() {
            if entry.path() == base_dir {
                continue; // Skip the directory itself
            }

            if let Some(filename) = entry.file_name().to_str() {
                if filename.starts_with(partial) {
                    let mut path_str = if prefix.contains('/') {
                        let dir_prefix = Path::new(prefix)
                            .parent()
                            .and_then(|p| p.to_str())
                            .unwrap_or("");
                        if dir_prefix.is_empty() {
                            filename.to_string()
                        } else {
                            format!("{}/{}", dir_prefix, filename)
                        }
                    } else {
                        filename.to_string()
                    };

                    // Add trailing slash for directories
                    if entry.path().is_dir() {
                        path_str.push('/');
                    }

                    matches.push(path_str);
                }
            }
        }

        matches.sort();
        matches
    }

    /// Get git branches with caching
    fn get_git_branches(&self) -> Vec<String> {
        // Check cache first
        {
            let cache = self.git_branches_cache.read().unwrap();
            if let Some(entry) = cache.as_ref() {
                if !entry.is_expired(self.cache_ttl) {
                    return entry.data.clone();
                }
            }
        }

        // Cache miss or expired, scan git branches
        let branches = self.scan_git_branches();

        // Update cache
        {
            let mut cache = self.git_branches_cache.write().unwrap();
            *cache = Some(CacheEntry::new(branches.clone()));
        }

        branches
    }

    /// Scan git branches in current repository
    #[cfg(feature = "git-builtins")]
    fn scan_git_branches(&self) -> Vec<String> {
        let runtime = self.runtime.read().unwrap();
        let cwd = runtime.get_cwd();

        if let Ok(repo) = git2::Repository::discover(cwd) {
            let mut branches = Vec::new();

            if let Ok(refs) = repo.branches(Some(git2::BranchType::Local)) {
                for (branch, _) in refs.flatten() {
                    if let Ok(Some(name)) = branch.name() {
                        branches.push(name.to_string());
                    }
                }
            }

            branches
        } else {
            Vec::new()
        }
    }

    /// Scan git branches (stub when git-builtins feature is disabled)
    #[cfg(not(feature = "git-builtins"))]
    fn scan_git_branches(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get cargo subcommands
    fn get_cargo_commands(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "check".to_string(),
            "clean".to_string(),
            "doc".to_string(),
            "test".to_string(),
            "bench".to_string(),
            "run".to_string(),
            "publish".to_string(),
            "install".to_string(),
            "update".to_string(),
            "search".to_string(),
            "add".to_string(),
            "remove".to_string(),
        ]
    }

    /// Get npm scripts from package.json
    fn get_npm_scripts(&self) -> Vec<String> {
        let runtime = self.runtime.read().unwrap();
        let package_json = runtime.get_cwd().join("package.json");

        if let Ok(content) = fs::read_to_string(package_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                    return scripts.keys().cloned().collect();
                }
            }
        }

        Vec::new()
    }

    /// Parse the input to determine completion context
    fn parse_context(&self, line: &str, pos: usize) -> CompletionContext {
        let before_cursor = &line[..pos];
        let words: Vec<&str> = before_cursor.split_whitespace().collect();

        if words.is_empty() {
            return CompletionContext::Command;
        }

        let last_word = words.last().copied().unwrap_or("");

        // Check if we're completing the command itself
        if words.len() == 1 && !before_cursor.ends_with(char::is_whitespace) {
            return CompletionContext::Command;
        }

        let command = words[0];
        let arg_index = words.len() - 1;

        // Context-aware completion based on command
        match command {
            "git" if arg_index >= 1 => {
                let subcommand = words.get(1).copied();
                match subcommand {
                    Some("checkout") | Some("merge") | Some("branch") | Some("rebase") => {
                        return CompletionContext::GitBranch;
                    }
                    _ if arg_index == 1 => {
                        return CompletionContext::GitSubcommand;
                    }
                    _ => {}
                }
            }
            "cargo" if arg_index == 1 => {
                return CompletionContext::CargoCommand;
            }
            "npm" if arg_index >= 1 => {
                if words.get(1) == Some(&"run") && arg_index == 2 {
                    return CompletionContext::NpmScript;
                }
            }
            "rustc" | "rustdoc" => {
                return CompletionContext::RustFile;
            }
            _ => {}
        }

        // Check if completing a flag
        if last_word.starts_with('-') {
            return CompletionContext::Flag(command.to_string());
        }

        // Default to path completion
        CompletionContext::Path
    }
}

#[derive(Debug, PartialEq)]
enum CompletionContext {
    Command,
    Path,
    GitBranch,
    GitSubcommand,
    CargoCommand,
    NpmScript,
    RustFile,
    Flag(String),
}

impl ReedlineCompleter for Completer {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let context = self.parse_context(line, pos);
        let before_cursor = &line[..pos];
        let last_word = before_cursor.split_whitespace().last().unwrap_or("");

        let candidates = match context {
            CompletionContext::Command => {
                let all_commands = self.get_all_commands();
                self.engine
                    .fuzzy_filter(&all_commands, last_word, 50)
                    .into_iter()
                    .map(|(cmd, _)| cmd)
                    .collect::<Vec<_>>()
            }
            CompletionContext::Path => {
                let paths = self.engine.complete_files(last_word, 50);
                paths.into_iter().map(|(path, _)| path).collect()
            }
            CompletionContext::GitBranch => self
                .get_git_branches()
                .into_iter()
                .filter(|branch| branch.starts_with(last_word))
                .collect(),
            CompletionContext::GitSubcommand => {
                let git_commands = vec![
                    "add", "branch", "checkout", "clone", "commit", "diff", "fetch", "log",
                    "merge", "pull", "push", "rebase", "reset", "status", "tag",
                ];
                git_commands
                    .into_iter()
                    .filter(|cmd| cmd.starts_with(last_word))
                    .map(|s| s.to_string())
                    .collect()
            }
            CompletionContext::CargoCommand => self
                .get_cargo_commands()
                .into_iter()
                .filter(|cmd| cmd.starts_with(last_word))
                .collect(),
            CompletionContext::NpmScript => self
                .get_npm_scripts()
                .into_iter()
                .filter(|script| script.starts_with(last_word))
                .collect(),
            CompletionContext::RustFile => self
                .complete_path(last_word)
                .into_iter()
                .filter(|path| path.ends_with(".rs") || path.ends_with('/'))
                .collect(),
            CompletionContext::Flag(cmd) => self
                .builtin_flags
                .get(&cmd)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|flag| flag.starts_with(last_word))
                .collect(),
        };

        // Convert to Suggestions
        let start = pos - last_word.len();
        candidates
            .into_iter()
            .map(|value| Suggestion {
                value,
                description: None,
                extra: None,
                span: Span::new(start, pos),
                append_whitespace: true,
                style: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::Builtins;
    use crate::runtime::Runtime;

    fn setup_completer() -> Completer {
        let builtins = Arc::new(Builtins::new());
        let runtime = Arc::new(RwLock::new(Runtime::new()));
        Completer::new(builtins, runtime)
    }

    #[test]
    fn test_command_completion() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("ec", 2);

        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.contains(&"echo".to_string()));
    }

    #[test]
    fn test_builtin_completion() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("pw", 2);

        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.contains(&"pwd".to_string()));
    }

    #[test]
    #[ignore = "Depends on current working directory having 'src' folder"]
    fn test_path_completion_current_dir() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("ls sr", 5);

        // Should suggest paths starting with "sr"
        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.iter().any(|v| v.starts_with("sr")));
    }

    #[test]
    fn test_git_branch_context() {
        let completer = setup_completer();
        let context = completer.parse_context("git checkout ", 13);
        assert_eq!(context, CompletionContext::GitBranch);
    }

    #[test]
    fn test_git_merge_context() {
        let completer = setup_completer();
        let context = completer.parse_context("git merge mai", 13);
        assert_eq!(context, CompletionContext::GitBranch);
    }

    #[test]
    fn test_cargo_command_context() {
        let completer = setup_completer();
        let context = completer.parse_context("cargo bu", 8);
        assert_eq!(context, CompletionContext::CargoCommand);
    }

    #[test]
    fn test_npm_run_context() {
        let completer = setup_completer();
        let context = completer.parse_context("npm run te", 10);
        assert_eq!(context, CompletionContext::NpmScript);
    }

    #[test]
    fn test_flag_completion() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("ls -", 4);

        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.contains(&"-l".to_string()));
        assert!(values.contains(&"-a".to_string()));
    }

    #[test]
    fn test_grep_flag_completion() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("grep -i", 7);

        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.contains(&"-i".to_string()));
    }

    #[test]
    fn test_rust_file_context() {
        let completer = setup_completer();
        let context = completer.parse_context("rustc ", 6);
        assert_eq!(context, CompletionContext::RustFile);
    }

    #[test]
    fn test_command_context_at_start() {
        let completer = setup_completer();
        let context = completer.parse_context("cat", 3);
        assert_eq!(context, CompletionContext::Command);
    }

    #[test]
    fn test_path_cache_expiry() {
        let completer = setup_completer();

        // First call should populate cache
        let executables1 = completer.get_path_executables();

        // Second call should use cache
        let executables2 = completer.get_path_executables();

        assert_eq!(executables1, executables2);
    }

    #[test]
    fn test_git_subcommand_completion() {
        let mut completer = setup_completer();
        let suggestions = completer.complete("git chec", 8);

        let values: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        assert!(values.contains(&"checkout".to_string()));
    }
}
