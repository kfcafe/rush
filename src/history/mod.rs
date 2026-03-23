// Command history and search with persistence and fuzzy matching

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Configuration for history behavior
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    /// Maximum number of entries to keep in history
    pub max_size: usize,
    /// Whether to deduplicate across entire history (not just consecutive)
    pub deduplicate_all: bool,
    /// Whether to show timestamps in history output
    pub show_timestamps: bool,
    /// Patterns to ignore (commands starting with these won't be saved)
    pub ignore_patterns: Vec<String>,
    /// Ignore commands starting with space (bash HISTIGNORE behavior)
    pub ignore_space: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_size: 10_000,
            deduplicate_all: false,
            show_timestamps: false,
            ignore_patterns: Vec::new(),
            ignore_space: true,
        }
    }
}

/// A single history entry with timestamp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: DateTime<Utc>,
}

impl HistoryEntry {
    pub fn new(command: String) -> Self {
        Self {
            command,
            timestamp: Utc::now(),
        }
    }

    pub fn with_timestamp(command: String, timestamp: DateTime<Utc>) -> Self {
        Self { command, timestamp }
    }
}

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: HistoryEntry,
    pub score: i64,
}

/// Command history manager with persistence and fuzzy search
pub struct History {
    entries: Vec<HistoryEntry>,
    config: HistoryConfig,
    history_file: PathBuf,
    matcher: SkimMatcherV2,
}

impl Clone for History {
    fn clone(&self) -> Self {
        // SkimMatcherV2 doesn't implement Clone, so create a new instance
        Self {
            entries: self.entries.clone(),
            config: self.config.clone(),
            history_file: self.history_file.clone(),
            matcher: SkimMatcherV2::default(),
        }
    }
}

impl History {
    /// Create a new history instance with default configuration
    pub fn new() -> Self {
        Self::with_config(HistoryConfig::default())
    }

    /// Create a new history instance with custom configuration
    pub fn with_config(config: HistoryConfig) -> Self {
        let history_file = Self::default_history_file();
        Self {
            entries: Vec::new(),
            config,
            history_file,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Create a new history instance with custom file path
    pub fn with_file<P: AsRef<Path>>(path: P, config: HistoryConfig) -> Self {
        Self {
            entries: Vec::new(),
            config,
            history_file: path.as_ref().to_path_buf(),
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Get the default history file path (~/.rush_history)
    pub fn default_history_file() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".rush_history")
    }

    /// Load history from file
    pub fn load(&mut self) -> Result<()> {
        if !self.history_file.exists() {
            return Ok(());
        }

        let file = File::open(&self.history_file).context("Failed to open history file")?;
        let reader = BufReader::new(file);

        self.entries.clear();

        for line in reader.lines() {
            let line = line.context("Failed to read line from history file")?;
            if line.is_empty() {
                continue;
            }

            // Try to parse as JSON first (new format with timestamps)
            if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
                self.entries.push(entry);
            } else {
                // Fallback to plain text (old format)
                self.entries.push(HistoryEntry::new(line));
            }
        }

        // Enforce max size
        self.trim_to_max_size();

        Ok(())
    }

    /// Save history to file
    pub fn save(&self) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.history_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create history directory")?;
        }

        let mut file = File::create(&self.history_file).context("Failed to create history file")?;

        for entry in &self.entries {
            let json = serde_json::to_string(entry).context("Failed to serialize history entry")?;
            writeln!(file, "{}", json).context("Failed to write to history file")?;
        }

        Ok(())
    }

    /// Append a single command to the history file (incremental save)
    pub fn append_to_file(&self, entry: &HistoryEntry) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.history_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create history directory")?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)
            .context("Failed to open history file for appending")?;

        let json = serde_json::to_string(entry).context("Failed to serialize history entry")?;
        writeln!(file, "{}", json).context("Failed to append to history file")?;

        Ok(())
    }

    /// Add a command to history
    pub fn add(&mut self, command: String) -> Result<()> {
        // Check if command should be ignored
        if self.should_ignore(&command) {
            return Ok(());
        }

        // Check for consecutive duplicates
        if let Some(last) = self.entries.last() {
            if last.command == command {
                return Ok(());
            }
        }

        // Check for duplicates across entire history if configured
        if self.config.deduplicate_all {
            if let Some(pos) = self.entries.iter().position(|e| e.command == command) {
                self.entries.remove(pos);
            }
        }

        let entry = HistoryEntry::new(command);

        // Append to file
        if let Err(e) = self.append_to_file(&entry) {
            eprintln!("Warning: Failed to append to history file: {}", e);
        }

        // Add to in-memory entries
        self.entries.push(entry);

        // Trim if necessary
        self.trim_to_max_size();

        Ok(())
    }

    /// Check if a command should be ignored based on configuration
    fn should_ignore(&self, command: &str) -> bool {
        // Ignore empty commands
        if command.trim().is_empty() {
            return true;
        }

        // Ignore commands starting with space
        if self.config.ignore_space && command.starts_with(' ') {
            return true;
        }

        // Ignore commands matching patterns
        for pattern in &self.config.ignore_patterns {
            if command.starts_with(pattern) {
                return true;
            }
        }

        false
    }

    /// Trim history to max size
    fn trim_to_max_size(&mut self) {
        if self.entries.len() > self.config.max_size {
            let excess = self.entries.len() - self.config.max_size;
            self.entries.drain(0..excess);
        }
    }

    /// Get an entry by index
    pub fn get(&self, index: usize) -> Option<&HistoryEntry> {
        self.entries.get(index)
    }

    /// Get all entries
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Get the last N entries
    pub fn last_n(&self, n: usize) -> &[HistoryEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Fuzzy search for commands
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = self
            .entries
            .iter()
            .filter_map(|entry| {
                self.matcher
                    .fuzzy_match(&entry.command, query)
                    .map(|score| SearchResult {
                        entry: entry.clone(),
                        score,
                    })
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.cmp(&a.score));

        // Take top N results
        results.truncate(max_results);

        results
    }

    /// Search for exact substring matches
    pub fn search_substring(&self, query: &str, max_results: usize) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .rev() // Search from most recent
            .filter(|entry| entry.command.contains(query))
            .take(max_results)
            .collect()
    }

    /// Clear all history
    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the configuration
    pub fn config(&self) -> &HistoryConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: HistoryConfig) {
        self.config = config;
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_history() -> (History, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let history_file = temp_dir.path().join("test_history");
        let config = HistoryConfig::default();
        let history = History::with_file(history_file, config);
        (history, temp_dir)
    }

    #[test]
    fn test_add_command() {
        let (mut history, _temp) = create_test_history();
        history.add("echo hello".to_string()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().command, "echo hello");
    }

    #[test]
    fn test_consecutive_duplicate_prevention() {
        let (mut history, _temp) = create_test_history();
        history.add("ls".to_string()).unwrap();
        history.add("ls".to_string()).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_non_consecutive_duplicates_allowed_by_default() {
        let (mut history, _temp) = create_test_history();
        history.add("ls".to_string()).unwrap();
        history.add("pwd".to_string()).unwrap();
        history.add("ls".to_string()).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_deduplicate_all() {
        let (mut history, _temp) = create_test_history();
        let mut config = HistoryConfig::default();
        config.deduplicate_all = true;
        history.set_config(config);

        history.add("ls".to_string()).unwrap();
        history.add("pwd".to_string()).unwrap();
        history.add("ls".to_string()).unwrap();

        // Should only have 2 entries: pwd and ls (moved to end)
        assert_eq!(history.len(), 2);
        assert_eq!(history.get(0).unwrap().command, "pwd");
        assert_eq!(history.get(1).unwrap().command, "ls");
    }

    #[test]
    fn test_ignore_space() {
        let (mut history, _temp) = create_test_history();
        history.add(" secret command".to_string()).unwrap();
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_ignore_patterns() {
        let (mut history, _temp) = create_test_history();
        let mut config = HistoryConfig::default();
        config.ignore_patterns = vec!["history".to_string(), "exit".to_string()];
        history.set_config(config);

        history.add("history".to_string()).unwrap();
        history.add("exit".to_string()).unwrap();
        history.add("ls".to_string()).unwrap();

        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().command, "ls");
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let history_file = temp_dir.path().join("test_history");

        // Create and populate history
        {
            let mut history = History::with_file(&history_file, HistoryConfig::default());
            history.add("echo test".to_string()).unwrap();
            history.add("ls -la".to_string()).unwrap();
            history.save().unwrap();
        }

        // Load in new instance
        {
            let mut history = History::with_file(&history_file, HistoryConfig::default());
            history.load().unwrap();
            assert_eq!(history.len(), 2);
            assert_eq!(history.get(0).unwrap().command, "echo test");
            assert_eq!(history.get(1).unwrap().command, "ls -la");
        }
    }

    #[test]
    fn test_max_size_enforcement() {
        let (mut history, _temp) = create_test_history();
        let mut config = HistoryConfig::default();
        config.max_size = 5;
        history.set_config(config);

        for i in 0..10 {
            history.add(format!("command {}", i)).unwrap();
        }

        assert_eq!(history.len(), 5);
        assert_eq!(history.get(0).unwrap().command, "command 5");
        assert_eq!(history.get(4).unwrap().command, "command 9");
    }

    #[test]
    fn test_fuzzy_search() {
        let (mut history, _temp) = create_test_history();
        history.add("git status".to_string()).unwrap();
        history.add("git commit -m 'test'".to_string()).unwrap();
        history.add("git push origin main".to_string()).unwrap();
        history.add("echo hello".to_string()).unwrap();

        let results = history.search("git", 10);
        assert_eq!(results.len(), 3);

        // All results should contain "git"
        for result in &results {
            assert!(result.entry.command.contains("git"));
        }

        let results = history.search("commit", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.command, "git commit -m 'test'");
    }

    #[test]
    fn test_fuzzy_search_ranking() {
        let (mut history, _temp) = create_test_history();
        history.add("echo test".to_string()).unwrap();
        history.add("test".to_string()).unwrap();
        history.add("testing 123".to_string()).unwrap();

        let results = history.search("test", 10);

        // All should match
        assert_eq!(results.len(), 3);

        // "test" exact match should be in results
        assert!(results.iter().any(|r| r.entry.command == "test"));

        // Exact match should have a positive score
        let exact_match = results.iter().find(|r| r.entry.command == "test").unwrap();
        assert!(exact_match.score > 0);
    }

    #[test]
    fn test_substring_search() {
        let (mut history, _temp) = create_test_history();
        history.add("cargo build".to_string()).unwrap();
        history.add("cargo test".to_string()).unwrap();
        history.add("cargo run".to_string()).unwrap();
        history.add("echo test".to_string()).unwrap();

        let results = history.search_substring("cargo", 10);
        assert_eq!(results.len(), 3);

        // Results should be in reverse order (most recent first)
        assert_eq!(results[0].command, "cargo run");
        assert_eq!(results[1].command, "cargo test");
        assert_eq!(results[2].command, "cargo build");
    }

    #[test]
    fn test_last_n() {
        let (mut history, _temp) = create_test_history();
        for i in 0..10 {
            history.add(format!("command {}", i)).unwrap();
        }

        let last_5 = history.last_n(5);
        assert_eq!(last_5.len(), 5);
        assert_eq!(last_5[0].command, "command 5");
        assert_eq!(last_5[4].command, "command 9");

        // Request more than available
        let last_20 = history.last_n(20);
        assert_eq!(last_20.len(), 10);
    }

    #[test]
    fn test_timestamps() {
        let (mut history, _temp) = create_test_history();

        let before = Utc::now();
        thread::sleep(Duration::from_millis(10));

        history.add("test command".to_string()).unwrap();

        thread::sleep(Duration::from_millis(10));
        let after = Utc::now();

        let entry = history.get(0).unwrap();
        assert!(entry.timestamp > before);
        assert!(entry.timestamp < after);
    }

    #[test]
    fn test_clear() {
        let (mut history, _temp) = create_test_history();
        history.add("test 1".to_string()).unwrap();
        history.add("test 2".to_string()).unwrap();

        assert_eq!(history.len(), 2);

        history.clear().unwrap();
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_empty_command_ignored() {
        let (mut history, _temp) = create_test_history();
        history.add("".to_string()).unwrap();
        history.add("   ".to_string()).unwrap();

        assert_eq!(history.len(), 0);
    }
}
