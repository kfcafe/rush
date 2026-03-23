// Undo capability for file operations
// Tracks file operations and allows reverting them

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_UNDO_OPERATIONS: usize = 100;
const UNDO_DIR: &str = ".rush_undo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Create { path: PathBuf },
    Delete { path: PathBuf, backup_path: PathBuf },
    Modify { path: PathBuf, backup_path: PathBuf },
    Move { from: PathBuf, to: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    pub operation: FileOperation,
    pub timestamp: SystemTime,
    pub description: String,
}

pub struct UndoManager {
    operations: VecDeque<UndoEntry>,
    undo_dir: PathBuf,
    enabled: bool,
}

impl Clone for UndoManager {
    fn clone(&self) -> Self {
        Self {
            operations: self.operations.clone(),
            undo_dir: self.undo_dir.clone(),
            enabled: self.enabled,
        }
    }
}

impl UndoManager {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        let undo_dir = home.join(UNDO_DIR);
        Self::with_undo_dir(undo_dir)
    }

    /// Create a disabled UndoManager that silently ignores all operations.
    /// Used as a last-resort fallback when neither home nor temp directories
    /// are writable.
    pub fn new_disabled() -> Self {
        Self {
            operations: VecDeque::new(),
            undo_dir: PathBuf::from("/dev/null"),
            enabled: false,
        }
    }

    /// Create UndoManager with a custom undo directory (primarily for testing)
    pub fn with_undo_dir(undo_dir: PathBuf) -> Result<Self> {
        // Create undo directory if it doesn't exist
        if !undo_dir.exists() {
            fs::create_dir_all(&undo_dir)?;
        }

        Ok(Self {
            operations: VecDeque::new(),
            undo_dir,
            enabled: true,
        })
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Track a file creation (can be undone by deleting)
    pub fn track_create(&mut self, path: PathBuf, description: String) {
        if !self.enabled {
            return;
        }

        let entry = UndoEntry {
            operation: FileOperation::Create { path },
            timestamp: SystemTime::now(),
            description,
        };

        self.add_operation(entry);
    }

    /// Track a file deletion (backs up file first)
    pub fn track_delete(&mut self, path: &Path, description: String) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Create backup before deletion
        let backup_path = self.create_backup(path)?;

        let entry = UndoEntry {
            operation: FileOperation::Delete {
                path: path.to_path_buf(),
                backup_path,
            },
            timestamp: SystemTime::now(),
            description,
        };

        self.add_operation(entry);
        Ok(())
    }

    /// Track a file modification (backs up original first)
    pub fn track_modify(&mut self, path: &Path, description: String) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Create backup before modification
        let backup_path = self.create_backup(path)?;

        let entry = UndoEntry {
            operation: FileOperation::Modify {
                path: path.to_path_buf(),
                backup_path,
            },
            timestamp: SystemTime::now(),
            description,
        };

        self.add_operation(entry);
        Ok(())
    }

    /// Track a file move/rename
    pub fn track_move(&mut self, from: PathBuf, to: PathBuf, description: String) {
        if !self.enabled {
            return;
        }

        let entry = UndoEntry {
            operation: FileOperation::Move { from, to },
            timestamp: SystemTime::now(),
            description,
        };

        self.add_operation(entry);
    }

    /// Undo the last operation
    pub fn undo(&mut self) -> Result<String> {
        let entry = self
            .operations
            .pop_back()
            .ok_or_else(|| anyhow!("No operations to undo"))?;

        match entry.operation {
            FileOperation::Create { path } => {
                if path.exists() {
                    if path.is_dir() {
                        fs::remove_dir(&path)?;
                    } else {
                        fs::remove_file(&path)?;
                    }
                }
                Ok(format!(
                    "Undone: {} (deleted {:?})",
                    entry.description, path
                ))
            }
            FileOperation::Delete { path, backup_path } => {
                if backup_path.exists() {
                    fs::copy(&backup_path, &path)?;
                    fs::remove_file(&backup_path)?;
                }
                Ok(format!(
                    "Undone: {} (restored {:?})",
                    entry.description, path
                ))
            }
            FileOperation::Modify { path, backup_path } => {
                if backup_path.exists() {
                    fs::copy(&backup_path, &path)?;
                    fs::remove_file(&backup_path)?;
                }
                Ok(format!(
                    "Undone: {} (restored {:?})",
                    entry.description, path
                ))
            }
            FileOperation::Move { from, to } => {
                if to.exists() {
                    fs::rename(&to, &from)?;
                }
                Ok(format!(
                    "Undone: {} (moved {:?} back to {:?})",
                    entry.description, to, from
                ))
            }
        }
    }

    /// List recent operations that can be undone
    pub fn list_operations(&self, limit: usize) -> Vec<&UndoEntry> {
        self.operations.iter().rev().take(limit).collect()
    }

    /// Clear all undo history
    pub fn clear(&mut self) -> Result<()> {
        // Remove all backup files
        for entry in &self.operations {
            match &entry.operation {
                FileOperation::Delete { backup_path, .. }
                | FileOperation::Modify { backup_path, .. } => {
                    if backup_path.exists() {
                        fs::remove_file(backup_path).ok();
                    }
                }
                _ => {}
            }
        }

        self.operations.clear();
        Ok(())
    }

    fn add_operation(&mut self, entry: UndoEntry) {
        self.operations.push_back(entry);

        // Keep only last MAX_UNDO_OPERATIONS
        while self.operations.len() > MAX_UNDO_OPERATIONS {
            if let Some(old_entry) = self.operations.pop_front() {
                // Clean up old backups
                match old_entry.operation {
                    FileOperation::Delete { backup_path, .. }
                    | FileOperation::Modify { backup_path, .. } => {
                        fs::remove_file(&backup_path).ok();
                    }
                    _ => {}
                }
            }
        }
    }

    fn create_backup(&self, path: &Path) -> Result<PathBuf> {
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("Invalid file path"))?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        let backup_name = format!("{}_{}", timestamp, file_name.to_string_lossy());
        let backup_path = self.undo_dir.join(backup_name);

        fs::copy(path, &backup_path)?;
        Ok(backup_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_track_and_undo_create() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content")?;

        undo_manager.track_create(test_file.clone(), "create test.txt".to_string());
        assert!(test_file.exists());

        undo_manager.undo()?;
        assert!(!test_file.exists());

        Ok(())
    }

    #[test]
    fn test_track_and_undo_delete() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "original content")?;

        undo_manager.track_delete(&test_file, "delete test.txt".to_string())?;
        fs::remove_file(&test_file)?;
        assert!(!test_file.exists());

        undo_manager.undo()?;
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file)?, "original content");

        Ok(())
    }

    #[test]
    fn test_track_and_undo_modify() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "original")?;

        undo_manager.track_modify(&test_file, "modify test.txt".to_string())?;
        fs::write(&test_file, "modified")?;
        assert_eq!(fs::read_to_string(&test_file)?, "modified");

        undo_manager.undo()?;
        assert_eq!(fs::read_to_string(&test_file)?, "original");

        Ok(())
    }

    #[test]
    fn test_track_and_undo_move() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        let from_path = temp_dir.path().join("from.txt");
        let to_path = temp_dir.path().join("to.txt");

        fs::write(&from_path, "content")?;

        undo_manager.track_move(from_path.clone(), to_path.clone(), "move file".to_string());
        fs::rename(&from_path, &to_path)?;

        assert!(!from_path.exists());
        assert!(to_path.exists());

        undo_manager.undo()?;
        assert!(from_path.exists());
        assert!(!to_path.exists());

        Ok(())
    }

    #[test]
    fn test_list_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        undo_manager.track_create(PathBuf::from("file1.txt"), "create 1".to_string());
        undo_manager.track_create(PathBuf::from("file2.txt"), "create 2".to_string());
        undo_manager.track_create(PathBuf::from("file3.txt"), "create 3".to_string());

        let ops = undo_manager.list_operations(2);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].description, "create 3");
        assert_eq!(ops[1].description, "create 2");

        Ok(())
    }

    #[test]
    fn test_max_operations_limit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        // Add more than MAX_UNDO_OPERATIONS
        for i in 0..MAX_UNDO_OPERATIONS + 10 {
            undo_manager.track_create(
                PathBuf::from(format!("file{}.txt", i)),
                format!("create {}", i),
            );
        }

        assert_eq!(undo_manager.operations.len(), MAX_UNDO_OPERATIONS);

        Ok(())
    }

    #[test]
    fn test_enable_disable() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let undo_dir = temp_dir.path().join("undo");
        let mut undo_manager = UndoManager::with_undo_dir(undo_dir)?;

        assert!(undo_manager.is_enabled());

        undo_manager.disable();
        assert!(!undo_manager.is_enabled());

        undo_manager.track_create(PathBuf::from("file.txt"), "create".to_string());
        assert_eq!(undo_manager.operations.len(), 0);

        undo_manager.enable();
        undo_manager.track_create(PathBuf::from("file.txt"), "create".to_string());
        assert_eq!(undo_manager.operations.len(), 1);

        Ok(())
    }
}
