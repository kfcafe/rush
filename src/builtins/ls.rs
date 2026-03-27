use crate::executor::ExecutionResult;
use crate::executor::Output;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use nu_ansi_term::{Color, Style};
use serde::{Deserialize, Serialize};
use std::fs::{self, Metadata};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileEntry {
    name: String,
    path: String,
    #[serde(rename = "type")]
    file_type: FileType,
    size: u64,
    modified: String,
    modified_timestamp: i64,
    permissions: String,
    mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    symlink_target: Option<String>,
}

#[derive(Default)]
struct LsFlags {
    long: bool,  // -l: long format
    all: bool,   // -a: show hidden files
    human: bool, // -h: human-readable sizes
    color: bool, // default: color output
    json: bool,  // --json: JSON output
}

pub fn builtin_ls(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let (mut flags, paths) = parse_args(args)?;
    if runtime.agent_mode() {
        flags.json = true;
    }

    let targets = if paths.is_empty() {
        vec![runtime.get_cwd().clone()]
    } else {
        paths
            .into_iter()
            .map(|p| {
                let path = PathBuf::from(p);
                if path.is_absolute() {
                    path
                } else {
                    runtime.get_cwd().join(path)
                }
            })
            .collect()
    };

    if flags.json {
        // JSON output mode
        let mut all_entries: Vec<FileEntry> = Vec::new();
        let mut had_error = false;
        let mut stderr_output = String::new();

        for target in targets.iter() {
            match collect_entries(target, &flags) {
                Ok(entries) => all_entries.extend(entries),
                Err(e) => {
                    stderr_output.push_str(&format!("ls: {}: {}\n", target.display(), e));
                    had_error = true;
                }
            }
        }

        let json_value = serde_json::to_value(&all_entries)?;

        Ok(ExecutionResult {
            output: Output::Structured(json_value),
            stderr: stderr_output,
            exit_code: if had_error { 1 } else { 0 },
            error: None,
        })
    } else {
        // Text output mode (existing behavior)
        let mut output = String::new();
        let mut stderr_output = String::new();
        let mut had_error = false;

        for (idx, target) in targets.iter().enumerate() {
            match list_path(target, &flags) {
                Ok(result) => {
                    if targets.len() > 1 {
                        if idx > 0 {
                            output.push('\n');
                        }
                        output.push_str(&format!("{}:\n", target.display()));
                    }
                    output.push_str(&result);
                }
                Err(e) => {
                    stderr_output.push_str(&format!("ls: {}: {}\n", target.display(), e));
                    had_error = true;
                }
            }
        }

        Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: stderr_output,
            exit_code: if had_error { 1 } else { 0 },
            error: None,
        })
    }
}

fn parse_args(args: &[String]) -> Result<(LsFlags, Vec<String>)> {
    let mut flags = LsFlags {
        color: true, // Enable color by default
        ..Default::default()
    };
    let mut paths = Vec::new();

    for arg in args {
        if arg == "--json" {
            flags.json = true;
        } else if arg.starts_with('-') && arg != "-" {
            // Parse flags
            for ch in arg.chars().skip(1) {
                match ch {
                    'l' => flags.long = true,
                    'a' => flags.all = true,
                    'h' => flags.human = true,
                    _ => return Err(anyhow!("ls: invalid option: -{}", ch)),
                }
            }
        } else {
            paths.push(arg.clone());
        }
    }

    Ok((flags, paths))
}

fn collect_entries(path: &Path, flags: &LsFlags) -> Result<Vec<FileEntry>> {
    if !path.exists() {
        return Err(anyhow!(
            "cannot access '{}': No such file or directory",
            path.display()
        ));
    }

    if path.is_file() {
        // Return single file entry
        let metadata = fs::metadata(path)?;
        return Ok(vec![metadata_to_file_entry(path, &metadata)?]);
    }

    // Collect directory contents
    let mut entries: Vec<FileEntry> = Vec::new();

    let walker = WalkBuilder::new(path)
        .max_depth(Some(1))
        .hidden(!flags.all)
        .git_ignore(false)
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                let entry_path = entry.path();

                // Skip the root directory itself
                if entry_path == path {
                    continue;
                }

                // Skip hidden files if -a not specified
                if !flags.all {
                    if let Some(name) = entry_path.file_name() {
                        if name.to_string_lossy().starts_with('.') {
                            continue;
                        }
                    }
                }

                if let Ok(metadata) = entry.metadata() {
                    if let Ok(file_entry) = metadata_to_file_entry(entry_path, &metadata) {
                        entries.push(file_entry);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // Sort entries by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(entries)
}

fn metadata_to_file_entry(path: &Path, metadata: &Metadata) -> Result<FileEntry> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let file_type = if metadata.is_dir() {
        FileType::Directory
    } else if metadata.is_symlink() {
        FileType::Symlink
    } else if metadata.is_file() {
        FileType::File
    } else {
        FileType::Other
    };

    let (modified, modified_timestamp) = metadata
        .modified()
        .ok()
        .and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH).ok().map(|d| {
                let timestamp = d.as_secs() as i64;
                let iso_time = format_iso8601(timestamp);
                (iso_time, timestamp)
            })
        })
        .unwrap_or_else(|| ("1970-01-01T00:00:00Z".to_string(), 0));

    let permissions = format_permissions(metadata);
    let mode = metadata.permissions().mode();

    let symlink_target = if metadata.is_symlink() {
        fs::read_link(path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(FileEntry {
        name,
        path: path.to_string_lossy().to_string(),
        file_type,
        size: metadata.len(),
        modified,
        modified_timestamp,
        permissions,
        mode,
        symlink_target,
    })
}

fn format_iso8601(timestamp: i64) -> String {
    // Simple ISO 8601 formatting
    // For production use, consider using chrono crate
    let seconds_in_day = 86400;
    let seconds_in_hour = 3600;
    let seconds_in_minute = 60;

    let days_since_epoch = timestamp / seconds_in_day;
    let seconds_today = timestamp % seconds_in_day;

    // Calculate year (rough approximation)
    let mut year = 1970;
    let mut days_left = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days_left < days_in_year {
            break;
        }
        days_left -= days_in_year;
        year += 1;
    }

    // Calculate month and day
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    let mut day_of_month = days_left + 1;

    for days in days_in_months.iter() {
        if day_of_month <= *days {
            break;
        }
        day_of_month -= days;
        month += 1;
    }

    let hour = seconds_today / seconds_in_hour;
    let minute = (seconds_today % seconds_in_hour) / seconds_in_minute;
    let second = seconds_today % seconds_in_minute;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day_of_month, hour, minute, second
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn list_path(path: &Path, flags: &LsFlags) -> Result<String> {
    if !path.exists() {
        return Err(anyhow!(
            "cannot access '{}': No such file or directory",
            path.display()
        ));
    }

    if path.is_file() {
        // List single file
        let metadata = fs::metadata(path)?;
        return Ok(format_entry(path, &metadata, flags));
    }

    // List directory contents
    let mut entries: Vec<(PathBuf, Metadata)> = Vec::new();

    // Use ignore::WalkBuilder for fast, gitignore-aware traversal
    let walker = WalkBuilder::new(path)
        .max_depth(Some(1))
        .hidden(!flags.all)
        .git_ignore(false) // Don't use gitignore for ls
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                let entry_path = entry.path();

                // Skip the root directory itself
                if entry_path == path {
                    continue;
                }

                // Skip hidden files if -a not specified
                if !flags.all {
                    if let Some(name) = entry_path.file_name() {
                        if name.to_string_lossy().starts_with('.') {
                            continue;
                        }
                    }
                }

                if let Ok(metadata) = entry.metadata() {
                    entries.push((entry_path.to_path_buf(), metadata));
                }
            }
            Err(_) => continue, // Skip entries we can't read
        }
    }

    // Sort entries by name
    entries.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));

    let mut output = String::new();

    if flags.long {
        // Long format
        for (path, metadata) in entries {
            output.push_str(&format_entry(&path, &metadata, flags));
        }
    } else {
        // Simple columnar format
        let names: Vec<String> = entries
            .iter()
            .map(|(p, m)| format_name(p, m, flags.color))
            .collect();

        // Simple single-column output for now (can be enhanced with terminal width detection)
        for name in names {
            output.push_str(&name);
            output.push('\n');
        }
    }

    Ok(output)
}

fn format_entry(path: &Path, metadata: &Metadata, flags: &LsFlags) -> String {
    if flags.long {
        format_long_entry(path, metadata, flags)
    } else {
        format!("{}\n", format_name(path, metadata, flags.color))
    }
}

fn format_long_entry(path: &Path, metadata: &Metadata, flags: &LsFlags) -> String {
    let perms = format_permissions(metadata);
    let nlink = metadata.nlink();
    let size = if flags.human {
        format_human_size(metadata.len())
    } else {
        metadata.len().to_string()
    };

    let modified = metadata
        .modified()
        .ok()
        .and_then(format_time)
        .unwrap_or_else(|| "?".to_string());

    let name = format_name(path, metadata, flags.color);

    format!("{} {:>3} {:>8} {} {}\n", perms, nlink, size, modified, name)
}

fn format_permissions(metadata: &Metadata) -> String {
    let mode = metadata.permissions().mode();
    let file_type = if metadata.is_dir() {
        'd'
    } else if metadata.is_symlink() {
        'l'
    } else {
        '-'
    };

    let user = format_permission_triple((mode >> 6) & 0o7);
    let group = format_permission_triple((mode >> 3) & 0o7);
    let other = format_permission_triple(mode & 0o7);

    format!("{}{}{}{}", file_type, user, group, other)
}

fn format_permission_triple(perms: u32) -> String {
    format!(
        "{}{}{}",
        if perms & 0o4 != 0 { 'r' } else { '-' },
        if perms & 0o2 != 0 { 'w' } else { '-' },
        if perms & 0o1 != 0 { 'x' } else { '-' }
    )
}

fn format_human_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    let mut size = size as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{}{}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}

fn format_time(time: SystemTime) -> Option<String> {
    use std::time::UNIX_EPOCH;

    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    let timestamp = duration.as_secs();

    // Format as "MMM DD HH:MM" (simplified - a full implementation would use chrono)
    let datetime = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
    let now = SystemTime::now();

    // Simple format (this is simplified; real ls uses locale-aware formatting)
    if let Ok(duration_since) = now.duration_since(datetime) {
        if duration_since.as_secs() < 15_552_000 {
            // 180 days
            // Recent file: show time
            Some(format_timestamp(timestamp, false))
        } else {
            // Old file: show year
            Some(format_timestamp(timestamp, true))
        }
    } else {
        Some(format_timestamp(timestamp, false))
    }
}

fn format_timestamp(timestamp: u64, show_year: bool) -> String {
    // Simplified timestamp formatting
    // In a production system, you'd want to use chrono or similar
    const MONTH_NAMES: &[&str] = &[
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let seconds_in_day = 86400;
    let days_since_epoch = timestamp / seconds_in_day;
    let seconds_today = timestamp % seconds_in_day;

    // Rough calculation (not accounting for leap years perfectly)
    let year = 1970 + (days_since_epoch / 365);
    let day_of_year = days_since_epoch % 365;
    let month = (day_of_year / 30).min(11) as usize;
    let day = (day_of_year % 30) + 1;

    let hour = seconds_today / 3600;
    let minute = (seconds_today % 3600) / 60;

    if show_year {
        format!("{} {:>2}  {}", MONTH_NAMES[month], day, year)
    } else {
        format!(
            "{} {:>2} {:>02}:{:>02}",
            MONTH_NAMES[month], day, hour, minute
        )
    }
}

fn format_name(path: &Path, metadata: &Metadata, use_color: bool) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    if !use_color {
        return name;
    }

    let style = get_color_style(metadata);
    style.paint(&name).to_string()
}

fn get_color_style(metadata: &Metadata) -> Style {
    if metadata.is_dir() {
        Color::Blue.bold()
    } else if metadata.is_symlink() {
        Color::Cyan.bold()
    } else if is_executable(metadata) {
        Color::Green.bold()
    } else {
        Style::default()
    }
}

fn is_executable(metadata: &Metadata) -> bool {
    let mode = metadata.permissions().mode();
    // Check if any execute bit is set
    mode & 0o111 != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_args_no_flags() {
        let args = vec!["file.txt".to_string()];
        let (flags, paths) = parse_args(&args).unwrap();
        assert!(!flags.long);
        assert!(!flags.all);
        assert!(!flags.human);
        assert_eq!(paths, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_args_with_flags() {
        let args = vec!["-lah".to_string(), "dir".to_string()];
        let (flags, paths) = parse_args(&args).unwrap();
        assert!(flags.long);
        assert!(flags.all);
        assert!(flags.human);
        assert_eq!(paths, vec!["dir"]);
    }

    #[test]
    fn test_parse_args_invalid_flag() {
        let args = vec!["-x".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_format_human_size() {
        assert_eq!(format_human_size(500), "500B");
        assert_eq!(format_human_size(1024), "1.0K");
        assert_eq!(format_human_size(1536), "1.5K");
        assert_eq!(format_human_size(1048576), "1.0M");
        assert_eq!(format_human_size(1073741824), "1.0G");
    }

    #[test]
    fn test_format_permissions() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        let metadata = fs::metadata(&file_path).unwrap();
        let perms = format_permissions(&metadata);

        // Should start with '-' for regular file
        assert!(perms.starts_with('-'));
        assert_eq!(perms.len(), 10);
    }

    #[test]
    fn test_ls_empty_directory() {
        let dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        // Empty directory should produce empty output (or just whitespace)
        assert!(result.stdout().trim().is_empty());
    }

    #[test]
    fn test_ls_with_files() {
        let dir = TempDir::new().unwrap();

        // Create some test files
        File::create(dir.path().join("file1.txt")).unwrap();
        File::create(dir.path().join("file2.txt")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&[], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("file1.txt"));
        assert!(result.stdout().contains("file2.txt"));
    }

    #[test]
    fn test_ls_hidden_files() {
        let dir = TempDir::new().unwrap();

        File::create(dir.path().join("visible.txt")).unwrap();
        File::create(dir.path().join(".hidden.txt")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        // Without -a flag
        let result = builtin_ls(&[], &mut runtime).unwrap();
        assert!(result.stdout().contains("visible.txt"));
        assert!(!result.stdout().contains(".hidden.txt"));

        // With -a flag
        let result = builtin_ls(&["-a".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("visible.txt"));
        assert!(result.stdout().contains(".hidden.txt"));
    }

    #[test]
    fn test_ls_long_format() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Hello, Rush!").unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["-l".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Should contain permission string
        assert!(result.stdout().contains("rw") || result.stdout().contains("r-"));
        // Should contain filename
        assert!(result.stdout().contains("test.txt"));
    }

    #[test]
    fn test_ls_nonexistent_path() {
        let mut runtime = Runtime::new();
        let result = builtin_ls(
            &["/nonexistent/path/that/does/not/exist".to_string()],
            &mut runtime,
        )
        .unwrap();

        // Should have non-zero exit code and report the problem on stderr.
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("cannot access"));
    }

    #[test]
    fn test_ls_specific_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("specific.txt");
        File::create(&file_path).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["specific.txt".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("specific.txt"));
    }

    #[test]
    fn test_is_executable() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("script.sh");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"#!/bin/bash\necho test").unwrap();

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&file_path, perms).unwrap();
        }

        let metadata = fs::metadata(&file_path).unwrap();
        assert!(is_executable(&metadata));
    }

    #[test]
    fn test_parse_args_json_flag() {
        let args = vec!["--json".to_string()];
        let (flags, paths) = parse_args(&args).unwrap();
        assert!(flags.json);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_parse_args_json_with_other_flags() {
        let args = vec!["--json".to_string(), "-la".to_string(), "dir".to_string()];
        let (flags, paths) = parse_args(&args).unwrap();
        assert!(flags.json);
        assert!(flags.long);
        assert!(flags.all);
        assert_eq!(paths, vec!["dir"]);
    }

    #[test]
    fn test_ls_json_empty_directory() {
        let dir = TempDir::new().unwrap();
        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Parse JSON output
        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 0);
    }

    #[test]
    fn test_ls_json_with_files() {
        let dir = TempDir::new().unwrap();

        // Create test files
        File::create(dir.path().join("file1.txt")).unwrap();
        File::create(dir.path().join("file2.txt")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Parse JSON output
        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 2);

        // Verify file entries
        let names: Vec<&str> = json_output.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"file2.txt"));

        // Check file types
        for entry in &json_output {
            assert!(matches!(entry.file_type, FileType::File));
        }
    }

    #[test]
    fn test_ls_json_with_metadata() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Hello, Rush!").unwrap();
        drop(file);

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 1);

        let entry = &json_output[0];
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.size, 12); // "Hello, Rush!" is 12 bytes
        assert!(matches!(entry.file_type, FileType::File));
        assert!(!entry.permissions.is_empty());
        assert!(entry.modified_timestamp > 0);
        assert!(!entry.modified.is_empty());
    }

    #[test]
    fn test_ls_json_directory_type() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 1);

        let entry = &json_output[0];
        assert_eq!(entry.name, "subdir");
        assert!(matches!(entry.file_type, FileType::Directory));
    }

    #[test]
    fn test_ls_json_hidden_files() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("visible.txt")).unwrap();
        File::create(dir.path().join(".hidden.txt")).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        // Without -a flag
        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 1);
        assert_eq!(json_output[0].name, "visible.txt");

        // With -a flag
        let result = builtin_ls(&["--json".to_string(), "-a".to_string()], &mut runtime).unwrap();
        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 2);

        let names: Vec<&str> = json_output.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"visible.txt"));
        assert!(names.contains(&".hidden.txt"));
    }

    #[test]
    fn test_ls_json_specific_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("specific.txt");
        File::create(&file_path).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(
            &["--json".to_string(), "specific.txt".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);

        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();
        assert_eq!(json_output.len(), 1);
        assert_eq!(json_output[0].name, "specific.txt");
    }

    #[test]
    fn test_ls_json_permissions_format() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        let mut runtime = Runtime::new();
        runtime.set_cwd(dir.path().to_path_buf());

        let result = builtin_ls(&["--json".to_string()], &mut runtime).unwrap();
        let json_output: Vec<FileEntry> = serde_json::from_str(result.stdout().trim()).unwrap();

        let entry = &json_output[0];
        // Permissions should be in format like "-rw-r--r--"
        assert_eq!(entry.permissions.len(), 10);
        assert!(entry.permissions.starts_with('-') || entry.permissions.starts_with('d'));
        // Mode should be a valid Unix mode
        assert!(entry.mode > 0);
    }

    #[test]
    fn test_format_iso8601() {
        // Test epoch time
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");

        // Test a known timestamp: 2024-01-15 12:30:45 UTC (1705322445)
        let result = format_iso8601(1705322445);
        assert!(result.starts_with("2024-01-"));
        assert!(result.contains("T"));
        assert!(result.ends_with("Z"));
    }

    #[test]
    fn test_is_leap_year() {
        assert!(!is_leap_year(1970));
        assert!(!is_leap_year(2023));
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
    }
}
