//! Universal variables — shell variables that persist across all sessions.
//!
//! Inspired by fish shell's universal variables, these are stored in
//! `~/.config/rush/universal_vars` and shared between all running shells.
//! They differ from regular shell variables in that changes are immediately
//! visible to every concurrent session (future work: inotify/kqueue watching).
//!
//! File format (one variable per line):
//!   NAME=VALUE
//! Lines starting with '#' are comments. Empty lines are ignored.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A universal variable — persisted to disk and shared across sessions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniversalVar {
    pub name: String,
    pub value: String,
}

impl UniversalVar {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Storage layer for universal variables.
///
/// Reads and writes `~/.config/rush/universal_vars`. Loads eagerly at shell
/// startup; individual writes flush the full file atomically via a temp-file
/// rename to avoid corruption from concurrent writes.
#[derive(Clone, Default)]
pub struct UniversalVarStore {
    vars: HashMap<String, String>,
    path: Option<PathBuf>,
}

impl UniversalVarStore {
    /// Create a store backed by the default path (`~/.config/rush/universal_vars`).
    pub fn new() -> Self {
        let path = dirs::home_dir().map(|h| h.join(".config").join("rush").join("universal_vars"));
        let mut store = Self {
            vars: HashMap::new(),
            path,
        };
        let _ = store.load(); // best-effort; missing file is fine
        store
    }

    /// Create a store backed by an explicit path (used in tests).
    pub fn with_path(path: PathBuf) -> Self {
        let mut store = Self {
            vars: HashMap::new(),
            path: Some(path),
        };
        let _ = store.load();
        store
    }

    /// Load variables from disk. Silently ignores a missing file.
    pub fn load(&mut self) -> Result<()> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        if !path.exists() {
            return Ok(());
        }
        let contents = fs::read_to_string(&path)?;
        self.vars.clear();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                self.vars.insert(k.trim().to_string(), v.to_string());
            }
        }
        Ok(())
    }

    /// Persist all variables to disk atomically.
    pub fn save(&self) -> Result<()> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines: Vec<String> = self
            .vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        lines.sort();
        let contents = format!(
            "# Rush universal variables — do not edit while rush is running\n{}\n",
            lines.join("\n")
        );
        // Atomic write via temp file
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &contents)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Set a universal variable and persist immediately.
    pub fn set_universal_var(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<()> {
        self.vars.insert(name.into(), value.into());
        self.save()
    }

    /// Get the value of a universal variable, if set.
    pub fn get_universal_var(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(|v| v.as_str())
    }

    /// Remove a universal variable and persist immediately.
    pub fn remove_universal_var(&mut self, name: &str) -> Result<bool> {
        let removed = self.vars.remove(name).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Return all universal variables as an iterator of `(name, value)` pairs.
    pub fn all(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp_store() -> (UniversalVarStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("universal_vars");
        (UniversalVarStore::with_path(path), dir)
    }

    #[test]
    fn test_set_and_get_universal_var() {
        let (mut store, _dir) = tmp_store();
        store.set_universal_var("MY_VAR", "hello").unwrap();
        assert_eq!(store.get_universal_var("MY_VAR"), Some("hello"));
    }

    #[test]
    fn test_remove_universal_var() {
        let (mut store, _dir) = tmp_store();
        store.set_universal_var("X", "1").unwrap();
        assert!(store.remove_universal_var("X").unwrap());
        assert_eq!(store.get_universal_var("X"), None);
        assert!(!store.remove_universal_var("X").unwrap());
    }

    #[test]
    fn test_persist_and_reload() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("universal_vars");
        {
            let mut store = UniversalVarStore::with_path(path.clone());
            store.set_universal_var("PERSIST", "value").unwrap();
        }
        let store2 = UniversalVarStore::with_path(path);
        assert_eq!(store2.get_universal_var("PERSIST"), Some("value"));
    }
}
