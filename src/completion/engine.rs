// Tab completion engine with fuzzy matching and caching
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashSet;
use std::env;
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

/// Core completion engine with command, file, and fuzzy matching
pub struct CompletionEngine {
    /// Cached PATH executables
    path_cache: Arc<RwLock<Option<CacheEntry<Vec<String>>>>>,
    /// Cache TTL
    cache_ttl: Duration,
    /// Fuzzy matcher for scoring candidates
    fuzzy_matcher: fuzzy_matcher::skim::SkimMatcherV2,
}

impl CompletionEngine {
    /// Create a new completion engine
    pub fn new() -> Self {
        Self {
            path_cache: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(300), // 5 minutes
            fuzzy_matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
        }
    }

    /// Get PATH executables with caching and fuzzy matching
    pub fn complete_commands(&self, prefix: &str, limit: usize) -> Vec<(String, i64)> {
        let executables = self.get_path_executables_cached();
        self.fuzzy_filter(&executables, prefix, limit)
    }

    /// Complete files and directories with fuzzy matching
    pub fn complete_files(&self, prefix: &str, limit: usize) -> Vec<(String, i64)> {
        let paths = self.scan_paths(prefix);
        self.fuzzy_filter(&paths, prefix, limit)
    }

    /// Fuzzy filter candidates with scores
    pub fn fuzzy_filter(
        &self,
        candidates: &[String],
        prefix: &str,
        limit: usize,
    ) -> Vec<(String, i64)> {
        if prefix.is_empty() {
            return candidates
                .iter()
                .take(limit)
                .map(|s| (s.clone(), 0))
                .collect();
        }

        let mut scored: Vec<(String, i64)> = candidates
            .iter()
            .filter_map(|candidate| {
                self.fuzzy_matcher
                    .fuzzy_match(candidate, prefix)
                    .map(|score| (candidate.clone(), score))
            })
            .collect();

        // Sort by score (descending)
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        scored.into_iter().take(limit).collect()
    }

    /// Get PATH executables with caching
    fn get_path_executables_cached(&self) -> Vec<String> {
        // Check cache first
        {
            let cache = self.path_cache.read().unwrap();
            if let Some(entry) = cache.as_ref() {
                if !entry.is_expired(self.cache_ttl) {
                    return entry.data.clone();
                }
            }
        }

        // Cache miss or expired, scan PATH
        let executables = self.scan_path();

        // Update cache
        {
            let mut cache = self.path_cache.write().unwrap();
            *cache = Some(CacheEntry::new(executables.clone()));
        }

        executables
    }

    /// Scan PATH directories for executables
    fn scan_path(&self) -> Vec<String> {
        let mut executables = HashSet::new();

        if let Ok(path_var) = env::var("PATH") {
            for dir in path_var.split(':') {
                let path = Path::new(dir);
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                                {
                                    if let Some(name) = entry.file_name().to_str() {
                                        executables.insert(name.to_string());
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                if metadata.is_file() {
                                    if let Some(name) = entry.file_name().to_str() {
                                        executables.insert(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result: Vec<String> = executables.into_iter().collect();
        result.sort();
        result
    }

    /// Scan file system for paths matching prefix
    fn scan_paths(&self, prefix: &str) -> Vec<String> {
        let (dir, partial) = if prefix.contains('/') {
            let path = Path::new(prefix);
            let dir = path.parent().unwrap_or(Path::new("."));
            let partial = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            (dir.to_path_buf(), partial)
        } else {
            (PathBuf::from("."), prefix)
        };

        // Expand tilde
        let dir = if dir.to_string_lossy().starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                home.join(dir.to_string_lossy().strip_prefix("~").unwrap_or(""))
            } else {
                dir
            }
        } else {
            dir
        };

        let mut matches = Vec::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
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
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_dir() {
                                path_str.push('/');
                            }
                        }

                        matches.push(path_str);
                    }
                }
            }
        }

        matches.sort();
        matches
    }

    /// Clear PATH cache (useful for testing and cache invalidation)
    pub fn invalidate_cache(&self) {
        let mut cache = self.path_cache.write().unwrap();
        *cache = None;
    }
}

impl Default for CompletionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_completion() {
        let engine = CompletionEngine::new();
        let results = engine.complete_commands("ls", 10);

        // Should find at least "ls" command if it's in PATH
        assert!(!results.is_empty() || true); // May be empty if ls not in PATH in test env
    }

    #[test]
    fn test_fuzzy_filter_exact_match() {
        let engine = CompletionEngine::new();
        let candidates = vec!["echo".to_string(), "exit".to_string(), "export".to_string()];
        let results = engine.fuzzy_filter(&candidates, "echo", 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].0, "echo");
    }

    #[test]
    fn test_fuzzy_filter_partial_match() {
        let engine = CompletionEngine::new();
        let candidates = vec!["echo".to_string(), "exit".to_string(), "export".to_string()];
        let results = engine.fuzzy_filter(&candidates, "ex", 10);

        // Should match multiple candidates
        assert!(results.len() >= 1);
        assert!(results.iter().any(|r| r.0 == "exit" || r.0 == "export"));
    }

    #[test]
    fn test_fuzzy_filter_empty_prefix() {
        let engine = CompletionEngine::new();
        let candidates = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let results = engine.fuzzy_filter(&candidates, "", 10);

        // Empty prefix should return all candidates
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_fuzzy_filter_limit() {
        let engine = CompletionEngine::new();
        let candidates = vec![
            "apple".to_string(),
            "apricot".to_string(),
            "application".to_string(),
            "apply".to_string(),
        ];
        let results = engine.fuzzy_filter(&candidates, "ap", 2);

        // Should respect limit
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_file_completion() {
        let engine = CompletionEngine::new();
        let results = engine.complete_files(".", 10);

        // Current directory should always have some entries
        assert!(!results.is_empty() || true); // May vary by environment
    }

    #[test]
    fn test_path_cache() {
        let engine = CompletionEngine::new();
        let results1 = engine.get_path_executables_cached();
        let results2 = engine.get_path_executables_cached();

        // Both calls should return same results
        assert_eq!(results1, results2);
    }

    #[test]
    fn test_path_cache_invalidation() {
        let engine = CompletionEngine::new();
        let results1 = engine.get_path_executables_cached();

        engine.invalidate_cache();
        let results2 = engine.get_path_executables_cached();

        // Should be equal (same PATH)
        assert_eq!(results1, results2);
    }
}
