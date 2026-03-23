use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use strsim::jaro_winkler;

/// Configuration for command suggestion behavior
#[derive(Debug, Clone)]
pub struct SuggestionConfig {
    /// Minimum score threshold for suggestions (0-100)
    pub min_threshold: i64,
    /// Maximum number of suggestions to show
    pub max_suggestions: usize,
    /// Whether suggestions are enabled at all
    pub enabled: bool,
    /// Whether to include history-based suggestions
    pub use_history: bool,
    /// Whether to include context-aware suggestions (e.g., git commands in git repos)
    pub use_context: bool,
}

impl Default for SuggestionConfig {
    fn default() -> Self {
        Self {
            min_threshold: 50,
            max_suggestions: 5,
            enabled: true,
            use_history: true,
            use_context: true,
        }
    }
}

impl SuggestionConfig {
    /// Create configuration from environment variables
    ///
    /// Supports:
    /// - RUSH_SUGGEST_ENABLED: "0" or "false" to disable (default: enabled)
    /// - RUSH_SUGGEST_THRESHOLD: minimum score 0-100 (default: 30)
    /// - RUSH_SUGGEST_MAX: maximum suggestions (default: 5)
    /// - RUSH_SUGGEST_HISTORY: "0" or "false" to disable history suggestions (default: enabled)
    /// - RUSH_SUGGEST_CONTEXT: "0" or "false" to disable context-aware suggestions (default: enabled)
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // RUSH_SUGGEST_ENABLED
        if let Ok(val) = env::var("RUSH_SUGGEST_ENABLED") {
            config.enabled = !matches!(val.to_lowercase().as_str(), "0" | "false" | "no" | "off");
        }

        // RUSH_SUGGEST_THRESHOLD
        if let Ok(val) = env::var("RUSH_SUGGEST_THRESHOLD") {
            if let Ok(threshold) = val.parse::<i64>() {
                config.min_threshold = threshold.clamp(0, 100);
            }
        }

        // RUSH_SUGGEST_MAX
        if let Ok(val) = env::var("RUSH_SUGGEST_MAX") {
            if let Ok(max) = val.parse::<usize>() {
                config.max_suggestions = max.max(1);
            }
        }

        // RUSH_SUGGEST_HISTORY
        if let Ok(val) = env::var("RUSH_SUGGEST_HISTORY") {
            config.use_history =
                !matches!(val.to_lowercase().as_str(), "0" | "false" | "no" | "off");
        }

        // RUSH_SUGGEST_CONTEXT
        if let Ok(val) = env::var("RUSH_SUGGEST_CONTEXT") {
            config.use_context =
                !matches!(val.to_lowercase().as_str(), "0" | "false" | "no" | "off");
        }

        config
    }
}

/// Suggests corrections for mistyped commands or paths
pub struct Corrector {
    matcher: SkimMatcherV2,
    config: SuggestionConfig,
}

impl Default for Corrector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Corrector {
    fn clone(&self) -> Self {
        // SkimMatcherV2 doesn't implement Clone, so create a new instance
        Self {
            matcher: SkimMatcherV2::default(),
            config: self.config.clone(),
        }
    }
}

impl Corrector {
    pub fn new() -> Self {
        Self::with_config(SuggestionConfig::default())
    }

    pub fn with_config(config: SuggestionConfig) -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            config,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Calculate a combined score using both edit distance and fuzzy matching
    fn calculate_score(&self, target: &str, input: &str) -> i64 {
        // Use Jaro-Winkler distance (0.0 to 1.0, higher is better)
        let edit_score = jaro_winkler(target, input);

        // Convert to a score similar to fuzzy matcher (0-100+ range)
        let normalized_score = (edit_score * 100.0) as i64;

        // Also try fuzzy matching for subsequence matches
        let fuzzy_score = self.matcher.fuzzy_match(target, input).unwrap_or(0);

        // Take the maximum of both approaches
        normalized_score.max(fuzzy_score)
    }

    /// Suggest corrections for a command that wasn't found
    /// Returns suggestions sorted by similarity score (best first)
    pub fn suggest_command(&self, input: &str, builtins: &[String]) -> Vec<Suggestion> {
        self.suggest_command_with_aliases(input, builtins, &[])
    }

    /// Suggest corrections for a command with aliases
    /// Returns suggestions from builtins, aliases, and PATH
    pub fn suggest_command_with_aliases(
        &self,
        input: &str,
        builtins: &[String],
        aliases: &[String],
    ) -> Vec<Suggestion> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut builtin_suggestions = Vec::new();
        let mut alias_suggestions = Vec::new();
        let mut path_suggestions = Vec::new();

        // Check builtins first
        for builtin in builtins {
            let score = self.calculate_score(builtin, input);
            if score > self.config.min_threshold {
                builtin_suggestions.push(Suggestion {
                    text: builtin.clone(),
                    score,
                    kind: SuggestionKind::Builtin,
                });
            }
        }

        // Check aliases
        for alias in aliases {
            let score = self.calculate_score(alias, input);
            if score > self.config.min_threshold {
                alias_suggestions.push(Suggestion {
                    text: alias.clone(),
                    score,
                    kind: SuggestionKind::Alias,
                });
            }
        }

        // Check PATH executables only if we don't have good builtin/alias matches
        let best_score = builtin_suggestions
            .iter()
            .chain(alias_suggestions.iter())
            .map(|s| s.score)
            .max()
            .unwrap_or(0);

        if best_score < 80 {
            if let Ok(path_var) = env::var("PATH") {
                for path_dir in path_var.split(':') {
                    if let Ok(entries) = fs::read_dir(path_dir) {
                        for entry in entries.flatten() {
                            if let Ok(file_name) = entry.file_name().into_string() {
                                let score = self.calculate_score(&file_name, input);
                                // Use a much higher threshold for PATH commands to avoid spurious matches
                                // PATH commands are searched only as a fallback, so be strict
                                if score > 75 {
                                    path_suggestions.push(Suggestion {
                                        text: file_name,
                                        score,
                                        kind: SuggestionKind::Executable,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort all lists by score
        builtin_suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        alias_suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        path_suggestions.sort_by(|a, b| b.score.cmp(&a.score));

        // Deduplicate suggestions by text
        let mut seen = std::collections::HashSet::new();
        let mut all_suggestions = Vec::new();

        // Priority order: aliases > builtins > path
        for suggestion in alias_suggestions
            .into_iter()
            .chain(builtin_suggestions)
            .chain(path_suggestions)
        {
            if seen.insert(suggestion.text.clone()) {
                all_suggestions.push(suggestion);
            }
        }

        // Re-sort by score after merging
        all_suggestions.sort_by(|a, b| b.score.cmp(&a.score));

        // Take top N based on config
        all_suggestions.truncate(self.config.max_suggestions);
        all_suggestions
    }

    /// Suggest corrections with full context including history and directory awareness
    ///
    /// This method provides the most comprehensive suggestions by combining:
    /// - Builtin commands
    /// - Aliases
    /// - Commands from PATH
    /// - Commands from user's history (if use_history is enabled)
    /// - Context-aware suggestions based on current directory (if use_context is enabled)
    pub fn suggest_command_with_context(
        &self,
        input: &str,
        builtins: &[String],
        aliases: &[String],
        history_commands: &[String],
        current_dir: &Path,
    ) -> Vec<Suggestion> {
        if !self.config.enabled {
            return Vec::new();
        }

        // Start with base suggestions from builtins, aliases, and PATH
        let mut all_suggestions = self.suggest_command_with_aliases(input, builtins, aliases);
        let mut seen: std::collections::HashSet<String> =
            all_suggestions.iter().map(|s| s.text.clone()).collect();

        // Add history-based suggestions if enabled
        if self.config.use_history {
            for cmd in history_commands {
                // Extract the command name (first word)
                let cmd_name = cmd.split_whitespace().next().unwrap_or(cmd);

                if seen.contains(cmd_name) {
                    continue;
                }

                let score = self.calculate_score(cmd_name, input);
                if score > self.config.min_threshold {
                    seen.insert(cmd_name.to_string());
                    all_suggestions.push(Suggestion {
                        text: cmd_name.to_string(),
                        score,
                        kind: SuggestionKind::History,
                    });
                }
            }
        }

        // Add context-aware suggestions if enabled
        if self.config.use_context {
            let context_suggestions = self.suggest_contextual_commands(input, current_dir);
            for suggestion in context_suggestions {
                if !seen.contains(&suggestion.text) {
                    seen.insert(suggestion.text.clone());
                    all_suggestions.push(suggestion);
                }
            }
        }

        // Re-sort by score after adding history and context suggestions
        all_suggestions.sort_by(|a, b| b.score.cmp(&a.score));

        // Take top N based on config
        all_suggestions.truncate(self.config.max_suggestions);
        all_suggestions
    }

    /// Suggest commands based on current directory context
    ///
    /// Returns context-aware suggestions such as:
    /// - git commands when in a git repository
    /// - cargo commands when Cargo.toml is present
    fn suggest_contextual_commands(&self, input: &str, current_dir: &Path) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Check if we're in a git repository
        let is_git_repo = current_dir.join(".git").exists()
            || Self::find_ancestor_with(current_dir, ".git").is_some();

        // Check if we're in a Rust/Cargo project
        let is_cargo_project = current_dir.join("Cargo.toml").exists()
            || Self::find_ancestor_with(current_dir, "Cargo.toml").is_some();

        // Git context suggestions
        if is_git_repo {
            let git_commands = ["git", "gh", "glab"];
            for &cmd in &git_commands {
                let score = self.calculate_score(cmd, input);
                if score > self.config.min_threshold {
                    suggestions.push(Suggestion {
                        text: cmd.to_string(),
                        score: score + 10, // Boost score for contextual relevance
                        kind: SuggestionKind::Context,
                    });
                }
            }
        }

        // Cargo/Rust context suggestions
        if is_cargo_project {
            let cargo_commands = ["cargo", "rustc", "rustfmt", "clippy"];
            for &cmd in &cargo_commands {
                let score = self.calculate_score(cmd, input);
                if score > self.config.min_threshold {
                    suggestions.push(Suggestion {
                        text: cmd.to_string(),
                        score: score + 10, // Boost score for contextual relevance
                        kind: SuggestionKind::Context,
                    });
                }
            }
        }

        // Sort by score
        suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        suggestions
    }

    /// Find an ancestor directory containing a specific file or directory
    fn find_ancestor_with(start: &Path, target: &str) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        loop {
            if current.join(target).exists() {
                return Some(current);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Suggest corrections for a path that doesn't exist
    /// Looks in the parent directory and current directory for similar names
    pub fn suggest_path(&self, input: &Path, current_dir: &Path) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Get the parent directory and file name
        let (search_dir, target_name) = if let Some(parent) = input.parent() {
            let name = input.file_name().and_then(|n| n.to_str()).unwrap_or("");
            (parent, name)
        } else {
            (Path::new("."), input.to_str().unwrap_or(""))
        };

        // Make search_dir absolute if it's relative
        let search_path = if search_dir.is_absolute() {
            search_dir.to_path_buf()
        } else {
            current_dir.join(search_dir)
        };

        // Search in the parent/current directory
        if let Ok(entries) = fs::read_dir(&search_path) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    let score = self.calculate_score(&file_name, target_name);
                    if score > 30 {
                        let full_path = if search_dir.as_os_str().is_empty() {
                            PathBuf::from(&file_name)
                        } else {
                            search_dir.join(&file_name)
                        };

                        suggestions.push(Suggestion {
                            text: full_path.to_string_lossy().to_string(),
                            score,
                            kind: if entry.path().is_dir() {
                                SuggestionKind::Directory
                            } else {
                                SuggestionKind::File
                            },
                        });
                    }
                }
            }
        }

        // Also check current directory if input was absolute
        if input.is_absolute() && search_path != current_dir {
            if let Ok(entries) = fs::read_dir(current_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_name) = entry.file_name().into_string() {
                        let score = self.calculate_score(&file_name, target_name);
                        if score > 30 {
                            suggestions.push(Suggestion {
                                text: current_dir.join(&file_name).to_string_lossy().to_string(),
                                score,
                                kind: SuggestionKind::Recent,
                            });
                        }
                    }
                }
            }
        }

        // Sort by score (descending) and take top 3
        suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        suggestions.truncate(3);
        suggestions
    }

    /// Suggest corrections for common flag typos
    /// Returns suggestions for flags that are similar to the mistyped flag
    pub fn suggest_flag(&self, input: &str, valid_flags: &[&str]) -> Vec<Suggestion> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut suggestions = Vec::new();

        // Strip leading dashes for comparison
        let input_stripped = input.trim_start_matches('-');

        for &flag in valid_flags {
            let flag_stripped = flag.trim_start_matches('-');
            let score = self.calculate_score(flag_stripped, input_stripped);

            if score > self.config.min_threshold {
                suggestions.push(Suggestion {
                    text: flag.to_string(),
                    score,
                    kind: SuggestionKind::Flag,
                });
            }
        }

        // Sort by score (descending)
        suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        suggestions.truncate(3);
        suggestions
    }

    /// Suggest corrections for git subcommands
    /// Returns suggestions for git subcommands that are similar to the mistyped command
    pub fn suggest_git_subcommand(&self, input: &str) -> Vec<Suggestion> {
        if !self.config.enabled {
            return Vec::new();
        }

        let git_subcommands = [
            "add",
            "am",
            "archive",
            "bisect",
            "blame",
            "branch",
            "bundle",
            "checkout",
            "cherry",
            "cherry-pick",
            "citool",
            "clean",
            "clone",
            "commit",
            "config",
            "describe",
            "diff",
            "difftool",
            "fetch",
            "format-patch",
            "gc",
            "grep",
            "gui",
            "help",
            "init",
            "instaweb",
            "log",
            "merge",
            "mergetool",
            "mv",
            "notes",
            "pull",
            "push",
            "range-diff",
            "rebase",
            "reflog",
            "remote",
            "repack",
            "replace",
            "request-pull",
            "reset",
            "restore",
            "revert",
            "rm",
            "send-email",
            "shortlog",
            "show",
            "show-branch",
            "sparse-checkout",
            "stash",
            "status",
            "submodule",
            "switch",
            "tag",
            "worktree",
        ];

        let mut suggestions = Vec::new();

        for &subcmd in &git_subcommands {
            let score = self.calculate_score(subcmd, input);

            if score > self.config.min_threshold {
                suggestions.push(Suggestion {
                    text: subcmd.to_string(),
                    score,
                    kind: SuggestionKind::GitSubcommand,
                });
            }
        }

        // Sort by score (descending)
        suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        suggestions.truncate(3);
        suggestions
    }

    /// Calculate similarity percentage for display
    pub fn similarity_percent(score: i64, text: &str) -> u8 {
        // Rough heuristic: score relative to string length
        let max_score = text.len() as i64 * 10; // Approximate max score
        let percent = ((score as f64 / max_score as f64) * 100.0).clamp(0.0, 100.0);
        percent as u8
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub score: i64,
    pub kind: SuggestionKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SuggestionKind {
    Builtin,
    Executable,
    Directory,
    File,
    Recent,
    Alias,
    Flag,
    GitSubcommand,
    /// Suggestion from command history
    History,
    /// Context-aware suggestion (e.g., git/cargo commands when in relevant repos)
    Context,
}

impl SuggestionKind {
    pub fn label(&self) -> &str {
        match self {
            SuggestionKind::Builtin => "builtin",
            SuggestionKind::Executable => "command",
            SuggestionKind::Directory => "directory",
            SuggestionKind::File => "file",
            SuggestionKind::Recent => "recent",
            SuggestionKind::Alias => "alias",
            SuggestionKind::Flag => "flag",
            SuggestionKind::GitSubcommand => "git command",
            SuggestionKind::History => "from history",
            SuggestionKind::Context => "contextual",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_suggestion() {
        let corrector = Corrector::new();
        let builtins = vec!["echo".to_string(), "grep".to_string(), "find".to_string()];

        let suggestions = corrector.suggest_command("ehco", &builtins);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].text, "echo");
    }

    #[test]
    fn test_command_suggestion_multiple() {
        let corrector = Corrector::new();
        let builtins = vec!["grep".to_string(), "git".to_string(), "get".to_string()];

        let suggestions = corrector.suggest_command("gre", &builtins);
        assert!(!suggestions.is_empty());
        // Should match both "grep" and "get" with decent scores
        assert!(suggestions.iter().any(|s| s.text == "grep"));
    }

    #[test]
    fn test_similarity_percent() {
        let score = 50;
        let text = "example";
        let percent = Corrector::similarity_percent(score, text);
        assert!(percent > 0 && percent <= 100);
    }

    #[test]
    fn test_path_suggestion_current_dir() {
        let corrector = Corrector::new();
        let current_dir = std::env::current_dir().unwrap();

        // Try to find suggestions for a likely misspelling
        let suggestions = corrector.suggest_path(Path::new("srcc"), &current_dir);

        // If src exists, we should find it. Otherwise just verify we get some suggestions.
        if current_dir.join("src").exists() {
            assert!(
                suggestions.iter().any(|s| s.text.contains("src")),
                "Expected to find 'src' in suggestions: {:?}",
                suggestions
            );
        } else {
            // Just verify the function runs without error
            // Suggestions may or may not be empty depending on directory contents
        }
    }

    #[test]
    fn test_alias_suggestions() {
        let corrector = Corrector::new();
        let builtins = vec!["echo".to_string(), "cd".to_string()];
        let aliases = vec!["ll".to_string(), "la".to_string(), "ls".to_string()];

        // Test suggesting alias when user types close to "ll"
        let suggestions = corrector.suggest_command_with_aliases("l", &builtins, &aliases);
        assert!(!suggestions.is_empty());
        // Should find at least one of the aliases
        assert!(suggestions.iter().any(|s| s.kind == SuggestionKind::Alias
            || s.text == "ll"
            || s.text == "la"
            || s.text == "ls"));
    }

    #[test]
    fn test_flag_suggestions() {
        let corrector = Corrector::new();
        let valid_flags = &["--help", "--version", "--verbose", "-h", "-v"];

        // Test common typos
        let suggestions = corrector.suggest_flag("--hlep", valid_flags);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.text == "--help"));

        let suggestions2 = corrector.suggest_flag("--verbo", valid_flags);
        assert!(!suggestions2.is_empty());
        assert!(suggestions2.iter().any(|s| s.text == "--verbose"));
    }

    #[test]
    fn test_git_subcommand_suggestions() {
        let corrector = Corrector::new();

        // Test common typos
        let suggestions = corrector.suggest_git_subcommand("staus");
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.text == "status"));

        let suggestions2 = corrector.suggest_git_subcommand("comit");
        assert!(!suggestions2.is_empty());
        assert!(suggestions2.iter().any(|s| s.text == "commit"));

        let suggestions3 = corrector.suggest_git_subcommand("pusH");
        assert!(!suggestions3.is_empty());
        assert!(suggestions3.iter().any(|s| s.text == "push"));
    }

    #[test]
    fn test_did_you_mean_echo_typo() {
        let corrector = Corrector::new();
        let builtins = vec!["echo".to_string(), "exit".to_string(), "export".to_string()];

        // Test various echo typos
        for typo in &["ehco", "ecoh", "echoo", "ech", "eccho"] {
            let suggestions = corrector.suggest_command(typo, &builtins);
            assert!(
                !suggestions.is_empty(),
                "Expected suggestions for '{}'",
                typo
            );
            assert_eq!(
                suggestions[0].text, "echo",
                "Expected 'echo' as top suggestion for '{}'",
                typo
            );
        }
    }

    #[test]
    fn test_did_you_mean_grep_typo() {
        let corrector = Corrector::new();
        let builtins = vec!["grep".to_string(), "git".to_string()];

        // Test various grep typos
        for typo in &["gerp", "grpe", "gre", "grepp"] {
            let suggestions = corrector.suggest_command(typo, &builtins);
            assert!(
                !suggestions.is_empty(),
                "Expected suggestions for '{}'",
                typo
            );
            assert_eq!(
                suggestions[0].text, "grep",
                "Expected 'grep' as top suggestion for '{}'",
                typo
            );
        }
    }

    #[test]
    fn test_did_you_mean_cargo_typo() {
        let corrector = Corrector::new();
        let builtins = vec!["cargo".to_string(), "echo".to_string(), "cd".to_string()];

        // Test various cargo typos
        for typo in &["carg", "cargi", "cargoo", "crago"] {
            let suggestions = corrector.suggest_command(typo, &builtins);
            assert!(
                !suggestions.is_empty(),
                "Expected suggestions for '{}'",
                typo
            );
            assert_eq!(
                suggestions[0].text, "cargo",
                "Expected 'cargo' as top suggestion for '{}'",
                typo
            );
        }
    }

    #[test]
    fn test_did_you_mean_multiple_suggestions() {
        let corrector = Corrector::new();
        let builtins = vec!["grep".to_string(), "git".to_string(), "get".to_string()];

        // Search for "gre" should suggest both grep and get
        let suggestions = corrector.suggest_command("gre", &builtins);
        assert!(!suggestions.is_empty());
        // Top suggestion should be one of grep/get
        assert!(suggestions[0].text == "grep" || suggestions[0].text == "get");
    }

    #[test]
    fn test_suggestion_kinds_labeled() {
        assert_eq!(SuggestionKind::Builtin.label(), "builtin");
        assert_eq!(SuggestionKind::Executable.label(), "command");
        assert_eq!(SuggestionKind::Alias.label(), "alias");
        assert_eq!(SuggestionKind::Flag.label(), "flag");
        assert_eq!(SuggestionKind::GitSubcommand.label(), "git command");
        assert_eq!(SuggestionKind::History.label(), "from history");
        assert_eq!(SuggestionKind::Context.label(), "contextual");
    }

    #[test]
    fn test_disabled_corrector() {
        let mut corrector = Corrector::new();
        corrector.set_enabled(false);

        let builtins = vec!["echo".to_string()];
        let suggestions = corrector.suggest_command("ehco", &builtins);
        assert!(
            suggestions.is_empty(),
            "Disabled corrector should return no suggestions"
        );
    }

    #[test]
    fn test_config_default_values() {
        let config = SuggestionConfig::default();
        assert_eq!(config.min_threshold, 50);
        assert_eq!(config.max_suggestions, 5);
        assert!(config.enabled);
        assert!(config.use_history);
        assert!(config.use_context);
    }

    #[test]
    fn test_config_from_env_threshold() {
        // Save original value
        let original = env::var("RUSH_SUGGEST_THRESHOLD").ok();

        // Test with valid threshold
        env::set_var("RUSH_SUGGEST_THRESHOLD", "50");
        let config = SuggestionConfig::from_env();
        assert_eq!(config.min_threshold, 50);

        // Test with clamping (too high)
        env::set_var("RUSH_SUGGEST_THRESHOLD", "200");
        let config = SuggestionConfig::from_env();
        assert_eq!(config.min_threshold, 100);

        // Test with clamping (negative)
        env::set_var("RUSH_SUGGEST_THRESHOLD", "-10");
        let config = SuggestionConfig::from_env();
        assert_eq!(config.min_threshold, 0);

        // Restore original
        match original {
            Some(val) => env::set_var("RUSH_SUGGEST_THRESHOLD", val),
            None => env::remove_var("RUSH_SUGGEST_THRESHOLD"),
        }
    }

    #[test]
    fn test_config_from_env_enabled() {
        let original = env::var("RUSH_SUGGEST_ENABLED").ok();

        // Test disabling
        env::set_var("RUSH_SUGGEST_ENABLED", "0");
        let config = SuggestionConfig::from_env();
        assert!(!config.enabled);

        env::set_var("RUSH_SUGGEST_ENABLED", "false");
        let config = SuggestionConfig::from_env();
        assert!(!config.enabled);

        env::set_var("RUSH_SUGGEST_ENABLED", "no");
        let config = SuggestionConfig::from_env();
        assert!(!config.enabled);

        // Test enabling
        env::set_var("RUSH_SUGGEST_ENABLED", "1");
        let config = SuggestionConfig::from_env();
        assert!(config.enabled);

        // Restore
        match original {
            Some(val) => env::set_var("RUSH_SUGGEST_ENABLED", val),
            None => env::remove_var("RUSH_SUGGEST_ENABLED"),
        }
    }

    #[test]
    fn test_config_from_env_max_suggestions() {
        let original = env::var("RUSH_SUGGEST_MAX").ok();

        env::set_var("RUSH_SUGGEST_MAX", "10");
        let config = SuggestionConfig::from_env();
        assert_eq!(config.max_suggestions, 10);

        // Test minimum enforcement
        env::set_var("RUSH_SUGGEST_MAX", "0");
        let config = SuggestionConfig::from_env();
        assert_eq!(config.max_suggestions, 1);

        // Restore
        match original {
            Some(val) => env::set_var("RUSH_SUGGEST_MAX", val),
            None => env::remove_var("RUSH_SUGGEST_MAX"),
        }
    }

    #[test]
    fn test_config_from_env_history_and_context() {
        let original_history = env::var("RUSH_SUGGEST_HISTORY").ok();
        let original_context = env::var("RUSH_SUGGEST_CONTEXT").ok();

        // Disable both
        env::set_var("RUSH_SUGGEST_HISTORY", "off");
        env::set_var("RUSH_SUGGEST_CONTEXT", "false");
        let config = SuggestionConfig::from_env();
        assert!(!config.use_history);
        assert!(!config.use_context);

        // Enable both
        env::set_var("RUSH_SUGGEST_HISTORY", "yes");
        env::set_var("RUSH_SUGGEST_CONTEXT", "1");
        let config = SuggestionConfig::from_env();
        assert!(config.use_history);
        assert!(config.use_context);

        // Restore
        match original_history {
            Some(val) => env::set_var("RUSH_SUGGEST_HISTORY", val),
            None => env::remove_var("RUSH_SUGGEST_HISTORY"),
        }
        match original_context {
            Some(val) => env::set_var("RUSH_SUGGEST_CONTEXT", val),
            None => env::remove_var("RUSH_SUGGEST_CONTEXT"),
        }
    }

    #[test]
    fn test_suggest_command_with_context_history() {
        let corrector = Corrector::new();
        let builtins = vec!["echo".to_string(), "exit".to_string()];
        let aliases = vec!["ll".to_string()];
        let history = vec![
            "mycustomcmd --verbose".to_string(),
            "anothercommand".to_string(),
        ];
        let current_dir = std::env::current_dir().unwrap();

        // Search for something similar to history command
        let suggestions = corrector.suggest_command_with_context(
            "mycustom",
            &builtins,
            &aliases,
            &history,
            &current_dir,
        );

        // Should find the history command
        assert!(
            suggestions
                .iter()
                .any(|s| s.text == "mycustomcmd" && s.kind == SuggestionKind::History),
            "Expected to find 'mycustomcmd' from history in suggestions: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_suggest_command_with_context_disabled_history() {
        let mut config = SuggestionConfig::default();
        config.use_history = false;
        let corrector = Corrector::with_config(config);

        let builtins = vec!["echo".to_string()];
        let aliases = vec![];
        let history = vec!["mycustomcmd".to_string()];
        let current_dir = std::env::current_dir().unwrap();

        let suggestions = corrector.suggest_command_with_context(
            "mycustom",
            &builtins,
            &aliases,
            &history,
            &current_dir,
        );

        // Should NOT find history commands when history is disabled
        assert!(
            !suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::History),
            "Should not have history suggestions when use_history is false"
        );
    }

    #[test]
    fn test_suggest_contextual_commands_git_repo() {
        let corrector = Corrector::new();
        let current_dir = std::env::current_dir().unwrap();

        // If we're in a git repo, should suggest git
        if current_dir.join(".git").exists() {
            let suggestions =
                corrector.suggest_command_with_context("gi", &[], &[], &[], &current_dir);

            // Should find git as contextual suggestion
            assert!(
                suggestions.iter().any(|s| s.text == "git"),
                "Expected 'git' in suggestions when in git repo: {:?}",
                suggestions
            );
        }
    }

    #[test]
    fn test_suggest_contextual_commands_cargo_project() {
        let corrector = Corrector::new();
        let current_dir = std::env::current_dir().unwrap();

        // If we're in a Cargo project, should suggest cargo
        if current_dir.join("Cargo.toml").exists() {
            let suggestions =
                corrector.suggest_command_with_context("carg", &[], &[], &[], &current_dir);

            // Should find cargo as contextual suggestion
            assert!(
                suggestions.iter().any(|s| s.text == "cargo"),
                "Expected 'cargo' in suggestions when in Cargo project: {:?}",
                suggestions
            );
        }
    }

    #[test]
    fn test_find_ancestor_with() {
        let current_dir = std::env::current_dir().unwrap();

        // Should find .git if we're in a git repo
        let result = Corrector::find_ancestor_with(&current_dir, ".git");
        // Just verify the function works without panicking
        // The result depends on whether we're in a git repo
        let _ = result;
    }
}
