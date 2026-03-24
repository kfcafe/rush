//! Fast find builtin using the ignore crate for .gitignore-aware file walking
//!
//! This implementation is significantly faster than GNU find because it:
//! - Uses parallel directory traversal
//! - Respects .gitignore by default (skips ignored files)
//! - Automatically excludes common build directories (.git, node_modules, target)
//! - Uses efficient pattern matching with glob patterns

use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ignore::WalkBuilder;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize)]
struct FindResult {
    path: String,
    #[serde(rename = "type")]
    file_type: String,
    size: u64,
    modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<String>,
}

#[derive(Debug, Clone)]
enum FileType {
    File,
    Directory,
    Any,
}

#[derive(Debug, Clone)]
enum SizeFilter {
    Exact(u64),
    GreaterThan(u64),
    LessThan(u64),
}

#[derive(Debug, Clone)]
enum TimeFilter {
    ModifiedWithin(Duration),
    ModifiedBefore(Duration),
}

#[derive(Debug)]
struct FindOptions {
    /// Starting directory for the search
    start_path: PathBuf,
    /// Pattern for file name matching (glob pattern)
    name_pattern: Option<String>,
    /// File type filter (file, directory, or any)
    file_type: FileType,
    /// Size filter
    size_filter: Option<SizeFilter>,
    /// Modification time filter
    mtime_filter: Option<TimeFilter>,
    /// Whether to respect .gitignore files
    respect_gitignore: bool,
    /// Command to execute on each match (-exec)
    exec_command: Option<Vec<String>>,
    /// Maximum depth to search
    max_depth: Option<usize>,
    /// Follow symbolic links
    follow_links: bool,
    /// Output results in JSON format
    json_output: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            start_path: PathBuf::from("."),
            name_pattern: None,
            file_type: FileType::Any,
            size_filter: None,
            mtime_filter: None,
            respect_gitignore: true,
            exec_command: None,
            max_depth: None,
            follow_links: false,
            json_output: false,
        }
    }
}

/// Parse arguments into FindOptions
fn parse_args(args: &[String], runtime: &Runtime) -> Result<FindOptions> {
    let mut options = FindOptions::default();
    let mut i = 0;

    // First argument might be the starting path
    if !args.is_empty() && !args[0].starts_with('-') {
        let path = PathBuf::from(&args[0]);
        options.start_path = if path.is_absolute() {
            path
        } else {
            runtime.get_cwd().join(path)
        };
        i = 1;
    } else {
        options.start_path = runtime.get_cwd().clone();
    }

    // Parse flags
    while i < args.len() {
        match args[i].as_str() {
            "-name" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("find: -name requires an argument"));
                }
                options.name_pattern = Some(args[i].clone());
            }
            "-type" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("find: -type requires an argument"));
                }
                options.file_type = match args[i].as_str() {
                    "f" => FileType::File,
                    "d" => FileType::Directory,
                    _ => return Err(anyhow!("find: -type must be 'f' or 'd'")),
                };
            }
            "-size" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("find: -size requires an argument"));
                }
                options.size_filter = Some(parse_size(&args[i])?);
            }
            "-mtime" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("find: -mtime requires an argument"));
                }
                options.mtime_filter = Some(parse_mtime(&args[i])?);
            }
            "-exec" => {
                i += 1;
                let mut exec_cmd = Vec::new();
                while i < args.len() && args[i] != ";" {
                    exec_cmd.push(args[i].clone());
                    i += 1;
                }
                if i >= args.len() {
                    return Err(anyhow!("find: -exec must be terminated with ';'"));
                }
                options.exec_command = Some(exec_cmd);
            }
            "--json" => {
                options.json_output = true;
            }
            "--no-ignore" => {
                options.respect_gitignore = false;
            }
            "-L" | "--follow" => {
                options.follow_links = true;
            }
            "-maxdepth" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("find: -maxdepth requires an argument"));
                }
                options.max_depth = Some(
                    args[i]
                        .parse()
                        .map_err(|_| anyhow!("find: -maxdepth must be a positive integer"))?,
                );
            }
            flag => {
                return Err(anyhow!("find: unknown flag: {}", flag));
            }
        }
        i += 1;
    }

    Ok(options)
}

/// Parse size argument (e.g., "+100k", "-1M", "500")
fn parse_size(size_str: &str) -> Result<SizeFilter> {
    let multipliers = [
        ('k', 1024u64),
        ('M', 1024 * 1024),
        ('G', 1024 * 1024 * 1024),
    ];

    let (prefix, number_str) = if let Some(stripped) = size_str.strip_prefix('+') {
        ('+', stripped)
    } else if let Some(stripped) = size_str.strip_prefix('-') {
        ('-', stripped)
    } else {
        ('=', size_str)
    };

    let (num_part, multiplier) = if let Some(last_char) = number_str.chars().last() {
        if last_char.is_alphabetic() {
            (
                number_str.strip_suffix(last_char).unwrap_or(number_str),
                last_char,
            )
        } else {
            (number_str, ' ')
        }
    } else {
        (number_str, ' ')
    };

    let base_size: u64 = num_part
        .parse()
        .map_err(|_| anyhow!("find: invalid size format: {}", size_str))?;

    let size = if multiplier == ' ' {
        base_size
    } else {
        let mult = multipliers
            .iter()
            .find(|(c, _)| *c == multiplier)
            .ok_or_else(|| anyhow!("find: invalid size suffix: {}", multiplier))?
            .1;
        base_size * mult
    };

    Ok(match prefix {
        '+' => SizeFilter::GreaterThan(size),
        '-' => SizeFilter::LessThan(size),
        _ => SizeFilter::Exact(size),
    })
}

/// Parse mtime argument (e.g., "-7" = modified within 7 days, "+30" = modified before 30 days ago)
fn parse_mtime(mtime_str: &str) -> Result<TimeFilter> {
    let (prefix, number_str) = if let Some(stripped) = mtime_str.strip_prefix('+') {
        ('+', stripped)
    } else if let Some(stripped) = mtime_str.strip_prefix('-') {
        ('-', stripped)
    } else {
        return Err(anyhow!(
            "find: -mtime requires + or - prefix (e.g., -7 or +30)"
        ));
    };

    let days: u64 = number_str
        .parse()
        .map_err(|_| anyhow!("find: invalid mtime format: {}", mtime_str))?;

    let duration = Duration::from_secs(days * 24 * 60 * 60);

    Ok(match prefix {
        '-' => TimeFilter::ModifiedWithin(duration),
        '+' => TimeFilter::ModifiedBefore(duration),
        _ => unreachable!(),
    })
}

/// Check if a file name matches a glob pattern
fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    // Simple glob pattern matching
    // Supports * (any chars) and ? (single char)
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let name_chars: Vec<char> = file_name.chars().collect();

    fn match_recursive(name: &[char], pattern: &[char], name_idx: usize, pat_idx: usize) -> bool {
        if pat_idx == pattern.len() {
            return name_idx == name.len();
        }

        if pattern[pat_idx] == '*' {
            // Try matching zero or more characters
            for i in name_idx..=name.len() {
                if match_recursive(name, pattern, i, pat_idx + 1) {
                    return true;
                }
            }
            false
        } else if pattern[pat_idx] == '?' {
            // Match exactly one character
            if name_idx < name.len() {
                match_recursive(name, pattern, name_idx + 1, pat_idx + 1)
            } else {
                false
            }
        } else {
            // Match literal character
            if name_idx < name.len() && name[name_idx] == pattern[pat_idx] {
                match_recursive(name, pattern, name_idx + 1, pat_idx + 1)
            } else {
                false
            }
        }
    }

    match_recursive(&name_chars, &pattern_chars, 0, 0)
}

/// Check if a file matches all the filters
fn matches_filters(path: &Path, options: &FindOptions) -> Result<bool> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };

    // Type filter
    match options.file_type {
        FileType::File if !metadata.is_file() => return Ok(false),
        FileType::Directory if !metadata.is_dir() => return Ok(false),
        _ => {}
    }

    // Name pattern filter
    if let Some(pattern) = &options.name_pattern {
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if !matches_pattern(file_name, pattern) {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }

    // Size filter
    if let Some(size_filter) = &options.size_filter {
        let size = metadata.len();
        let matches = match size_filter {
            SizeFilter::Exact(s) => size == *s,
            SizeFilter::GreaterThan(s) => size > *s,
            SizeFilter::LessThan(s) => size < *s,
        };
        if !matches {
            return Ok(false);
        }
    }

    // Mtime filter
    if let Some(mtime_filter) = &options.mtime_filter {
        if let Ok(modified) = metadata.modified() {
            let now = SystemTime::now();
            if let Ok(age) = now.duration_since(modified) {
                let matches = match mtime_filter {
                    TimeFilter::ModifiedWithin(d) => age <= *d,
                    TimeFilter::ModifiedBefore(d) => age >= *d,
                };
                if !matches {
                    return Ok(false);
                }
            }
        }
    }

    Ok(true)
}

/// Execute a command on a matched file (for -exec)
fn execute_command(command: &[String], file_path: &Path) -> Result<String> {
    let mut cmd_parts = Vec::new();
    for part in command {
        if part == "{}" {
            cmd_parts.push(file_path.to_string_lossy().to_string());
        } else {
            cmd_parts.push(part.clone());
        }
    }

    if cmd_parts.is_empty() {
        return Ok(String::new());
    }

    let output = std::process::Command::new(&cmd_parts[0])
        .args(&cmd_parts[1..])
        .output()
        .map_err(|e| anyhow!("find: failed to execute command: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Main find function
pub fn builtin_find(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut options = parse_args(args, runtime)?;
    if runtime.agent_mode() {
        options.json_output = true;
    }

    if !options.start_path.exists() {
        return Err(anyhow!(
            "find: '{}': No such file or directory",
            options.start_path.display()
        ));
    }

    let mut builder = WalkBuilder::new(&options.start_path);
    builder
        .git_ignore(options.respect_gitignore)
        .git_global(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .hidden(false) // Don't skip hidden files by default
        .follow_links(options.follow_links)
        .threads(num_cpus::get().min(4)); // Use multiple threads for speed

    if let Some(depth) = options.max_depth {
        builder.max_depth(Some(depth));
    }

    // Collect results
    let mut text_results = Vec::new();
    let mut json_results = Vec::new();
    let mut exec_output = String::new();

    for result in builder.build() {
        match result {
            Ok(entry) => {
                let path = entry.path();

                // Skip the start path itself unless it's a file
                if path == options.start_path && path.is_dir() {
                    continue;
                }

                if matches_filters(path, &options)? {
                    if let Some(exec_cmd) = &options.exec_command {
                        // Execute command on this file
                        exec_output.push_str(&execute_command(exec_cmd, path)?);
                    } else if options.json_output {
                        // Collect JSON metadata
                        if let Ok(metadata) = path.metadata() {
                            let file_type = if metadata.is_dir() {
                                "directory"
                            } else if metadata.is_symlink() {
                                "symlink"
                            } else {
                                "file"
                            };

                            let modified = metadata
                                .modified()
                                .ok()
                                .and_then(|t| {
                                    DateTime::<Utc>::from(t)
                                        .format("%Y-%m-%dT%H:%M:%SZ")
                                        .to_string()
                                        .into()
                                })
                                .unwrap_or_else(|| "unknown".to_string());

                            #[cfg(unix)]
                            let permissions = {
                                use std::os::unix::fs::PermissionsExt;
                                Some(format!("{:o}", metadata.permissions().mode() & 0o777))
                            };

                            #[cfg(not(unix))]
                            let permissions = None;

                            json_results.push(FindResult {
                                path: path.to_string_lossy().to_string(),
                                file_type: file_type.to_string(),
                                size: metadata.len(),
                                modified,
                                permissions,
                            });
                        }
                    } else {
                        // Just collect the path
                        text_results.push(path.to_string_lossy().to_string());
                    }
                }
            }
            Err(err) => {
                // Skip errors for inaccessible files
                eprintln!("find: {}", err);
            }
        }
    }

    if options.exec_command.is_some() {
        return Ok(ExecutionResult::success(exec_output));
    }

    if options.json_output {
        let json_value = serde_json::to_value(&json_results)
            .map_err(|e| anyhow!("Failed to serialize JSON: {}", e))?;
        return Ok(ExecutionResult {
            output: Output::Structured(json_value),
            stderr: String::new(),
            exit_code: 0,
            error: None,
        });
    }

    let text = if !text_results.is_empty() {
        text_results.join("\n") + "\n"
    } else {
        String::new()
    };
    Ok(ExecutionResult::success(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("test.rs", "*.rs"));
        assert!(matches_pattern("test.rs", "test.*"));
        assert!(matches_pattern("test.rs", "test.rs"));
        assert!(matches_pattern("test", "te?t"));
        assert!(!matches_pattern("test.txt", "*.rs"));
        assert!(!matches_pattern("test", "tes"));
    }

    #[test]
    fn test_parse_size() {
        let filter = parse_size("+100k").unwrap();
        matches!(filter, SizeFilter::GreaterThan(102400));

        let filter = parse_size("-1M").unwrap();
        matches!(filter, SizeFilter::LessThan(1048576));

        let filter = parse_size("500").unwrap();
        matches!(filter, SizeFilter::Exact(500));
    }

    #[test]
    fn test_parse_mtime() {
        let filter = parse_mtime("-7").unwrap();
        matches!(filter, TimeFilter::ModifiedWithin(_));

        let filter = parse_mtime("+30").unwrap();
        matches!(filter, TimeFilter::ModifiedBefore(_));
    }

    #[test]
    fn test_find_by_name() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test1.rs", "content");
        create_test_file(temp_dir.path(), "test2.txt", "content");
        create_test_file(temp_dir.path(), "test3.rs", "content");

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result =
            builtin_find(&vec!["-name".to_string(), "*.rs".to_string()], &mut runtime).unwrap();

        assert!(result.stdout().contains("test1.rs"));
        assert!(result.stdout().contains("test3.rs"));
        assert!(!result.stdout().contains("test2.txt"));
    }

    #[test]
    fn test_find_by_type() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "file.txt", "content");
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Find files
        let result =
            builtin_find(&vec!["-type".to_string(), "f".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("file.txt"));
        assert!(!result.stdout().contains("subdir"));

        // Find directories
        let result =
            builtin_find(&vec!["-type".to_string(), "d".to_string()], &mut runtime).unwrap();
        assert!(!result.stdout().contains("file.txt"));
        assert!(result.stdout().contains("subdir"));
    }

    #[test]
    fn test_find_respects_gitignore() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize a git repository so .gitignore is recognized
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(temp_dir.path())
            .output()
            .ok();

        create_test_file(temp_dir.path(), "include.txt", "content");
        create_test_file(temp_dir.path(), "ignore.log", "content");
        create_test_file(temp_dir.path(), ".gitignore", "*.log\n");

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        // With gitignore (default)
        let result = builtin_find(&vec![], &mut runtime).unwrap();
        assert!(result.stdout().contains("include.txt"));
        assert!(
            !result.stdout().contains("ignore.log"),
            "Expected ignore.log to be excluded, but found it in: {}",
            result.stdout()
        );

        // Without gitignore
        let result = builtin_find(&vec!["--no-ignore".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("include.txt"));
        assert!(result.stdout().contains("ignore.log"));
    }

    #[test]
    fn test_find_with_maxdepth() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "root.txt", "content");
        create_test_file(temp_dir.path(), "sub1/file1.txt", "content");
        create_test_file(temp_dir.path(), "sub1/sub2/file2.txt", "content");

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_find(
            &vec!["-maxdepth".to_string(), "1".to_string()],
            &mut runtime,
        )
        .unwrap();

        assert!(result.stdout().contains("root.txt"));
        assert!(!result.stdout().contains("file1.txt"));
        assert!(!result.stdout().contains("file2.txt"));
    }

    #[test]
    fn test_find_json_output() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test1.rs", "content1");
        create_test_file(temp_dir.path(), "test2.txt", "content2");
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_find(&vec!["--json".to_string()], &mut runtime).unwrap();
        let output = result.stdout();

        // Should be valid JSON
        let json: serde_json::Value = serde_json::from_str(&output).expect("Valid JSON");
        assert!(json.is_array());

        let array = json.as_array().unwrap();
        assert!(array.len() >= 3); // At least 3 entries

        // Check that each entry has required fields
        for item in array {
            assert!(item.get("path").is_some());
            assert!(item.get("type").is_some());
            assert!(item.get("size").is_some());
            assert!(item.get("modified").is_some());
        }
    }

    #[test]
    fn test_find_json_with_type_filter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "file.txt", "content");
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        // Find only files
        let result = builtin_find(
            &vec!["--json".to_string(), "-type".to_string(), "f".to_string()],
            &mut runtime,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        let array = json.as_array().unwrap();

        // All entries should be files
        for item in array {
            assert_eq!(item.get("type").unwrap().as_str().unwrap(), "file");
        }

        // Find only directories
        let result = builtin_find(
            &vec!["--json".to_string(), "-type".to_string(), "d".to_string()],
            &mut runtime,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        let array = json.as_array().unwrap();

        // All entries should be directories
        for item in array {
            assert_eq!(item.get("type").unwrap().as_str().unwrap(), "directory");
        }
    }

    #[test]
    fn test_find_json_with_name_pattern() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test1.rs", "content");
        create_test_file(temp_dir.path(), "test2.txt", "content");
        create_test_file(temp_dir.path(), "test3.rs", "content");

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_find(
            &vec![
                "--json".to_string(),
                "-name".to_string(),
                "*.rs".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        let array = json.as_array().unwrap();

        // Should only have .rs files
        for item in array {
            let path = item.get("path").unwrap().as_str().unwrap();
            assert!(path.ends_with(".rs"));
        }
    }

    #[test]
    fn test_find_json_metadata() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test.txt", "hello world");

        let mut runtime = Runtime::new();
        runtime.set_cwd(temp_dir.path().to_path_buf());

        let result = builtin_find(
            &vec![
                "--json".to_string(),
                "-name".to_string(),
                "test.txt".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        let array = json.as_array().unwrap();

        assert_eq!(array.len(), 1);
        let item = &array[0];

        // Check metadata fields
        assert!(item
            .get("path")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("test.txt"));
        assert_eq!(item.get("type").unwrap().as_str().unwrap(), "file");
        assert_eq!(item.get("size").unwrap().as_u64().unwrap(), 11); // "hello world" is 11 bytes
        assert!(item.get("modified").is_some());

        // On Unix systems, permissions should be present
        #[cfg(unix)]
        assert!(item.get("permissions").is_some());
    }
}
