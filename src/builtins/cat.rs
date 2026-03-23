use crate::executor::ExecutionResult;
use crate::executor::Output;
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use memmap2::Mmap;
use std::fs::File;
use std::io::{self, BufRead, Read};
use std::path::Path;

// Threshold for using memory-mapped files (1MB)
const MMAP_THRESHOLD: u64 = 1024 * 1024;

/// Options for the cat command
#[derive(Debug, Default)]
struct CatOptions {
    /// Show line numbers
    number_lines: bool,
    /// Files to concatenate
    files: Vec<String>,
}

impl CatOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut opts = CatOptions::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            if arg.starts_with('-') && arg.len() > 1 && arg != "-" {
                // Parse flags
                for ch in arg[1..].chars() {
                    match ch {
                        'n' => opts.number_lines = true,
                        _ => return Err(anyhow!("cat: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                // This is a file argument
                opts.files.push(arg.clone());
            }
            i += 1;
        }

        // If no files specified, read from stdin (represented as "-")
        if opts.files.is_empty() {
            opts.files.push("-".to_string());
        }

        Ok(opts)
    }
}

pub fn builtin_cat(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    let opts = match CatOptions::parse(args) {
        Ok(opts) => opts,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut output = String::new();
    let mut stderr_output = String::new();
    let mut line_number = 1;
    let mut exit_code = 0;

    for file_path in &opts.files {
        if file_path == "-" {
            // Read from stdin
            let stdin = io::stdin();
            let reader = stdin.lock();
            if let Err(e) =
                read_with_line_numbers(reader, &mut output, &mut line_number, opts.number_lines)
            {
                stderr_output.push_str(&e.to_string());
                stderr_output.push('\n');
                exit_code = 1;
            }
        } else {
            // Read from file
            if let Err(e) = read_file(file_path, &mut output, &mut line_number, opts.number_lines) {
                stderr_output.push_str(&e.to_string());
                stderr_output.push('\n');
                exit_code = 1;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: stderr_output,
        exit_code,
        error: None,
    })
}

/// Execute cat with provided stdin data (for pipelines)
pub fn builtin_cat_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let opts = CatOptions::parse(args)?;
    let mut output = String::new();
    let mut line_number = 1;

    // If no files specified or "-" is specified, read from stdin
    if opts.files.is_empty() || (opts.files.len() == 1 && opts.files[0] == "-") {
        let cursor = std::io::Cursor::new(stdin_data);
        read_with_line_numbers(cursor, &mut output, &mut line_number, opts.number_lines)?;
    } else {
        // Read from specified files
        for file_path in &opts.files {
            read_file(file_path, &mut output, &mut line_number, opts.number_lines)?;
        }
    }

    Ok(ExecutionResult::success(output))
}

/// Read a file using either memory-mapped I/O or buffered reading
fn read_file(
    path: &str,
    output: &mut String,
    line_number: &mut usize,
    number_lines: bool,
) -> Result<()> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(anyhow!("cat: {}: No such file or directory", path));
    }

    if !file_path.is_file() {
        return Err(anyhow!("cat: {}: Is a directory", path));
    }

    let file =
        File::open(file_path).with_context(|| format!("cat: {}: Failed to open file", path))?;

    let metadata = file
        .metadata()
        .with_context(|| format!("cat: {}: Failed to read metadata", path))?;

    let file_size = metadata.len();

    // Use memory-mapped I/O for large files
    if file_size > MMAP_THRESHOLD {
        read_mmap(&file, output, line_number, number_lines, path)?;
    } else {
        // Use buffered reading for small files
        read_small_file(file, output, line_number, number_lines, path)?;
    }

    Ok(())
}

/// Read a small file with optional line numbering (handles both text and binary)
fn read_small_file(
    mut file: File,
    output: &mut String,
    line_number: &mut usize,
    number_lines: bool,
    path: &str,
) -> Result<()> {
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("cat: {}: Failed to read file", path))?;

    // Check if the file contains null bytes (binary file detection)
    let is_binary = buffer.iter().take(8192.min(buffer.len())).any(|&b| b == 0);

    if is_binary {
        // For binary files, just output the raw bytes with replacement chars
        let content = String::from_utf8_lossy(&buffer);
        if number_lines {
            for line in content.lines() {
                output.push_str(&format!("{:6}\t{}\n", line_number, line));
                *line_number += 1;
            }
        } else {
            output.push_str(&content);
        }
    } else {
        // For text files, process normally
        let content =
            String::from_utf8(buffer).with_context(|| format!("cat: {}: Invalid UTF-8", path))?;

        if number_lines {
            for line in content.lines() {
                output.push_str(&format!("{:6}\t{}\n", line_number, line));
                *line_number += 1;
            }
        } else {
            output.push_str(&content);
        }
    }

    Ok(())
}

/// Read file using memory-mapped I/O (for large files)
fn read_mmap(
    file: &File,
    output: &mut String,
    line_number: &mut usize,
    number_lines: bool,
    path: &str,
) -> Result<()> {
    // Safety: We're reading a file that we just opened and verified exists.
    // The file descriptor is valid for the lifetime of the mmap.
    let mmap = unsafe {
        Mmap::map(file).with_context(|| format!("cat: {}: Failed to memory-map file", path))?
    };

    // Check if the file contains null bytes (binary file detection)
    let is_binary = mmap.iter().take(8192).any(|&b| b == 0);

    if is_binary {
        // For binary files, just output the raw bytes
        // Convert to String with replacement characters for invalid UTF-8
        let content = String::from_utf8_lossy(&mmap);
        if number_lines {
            for line in content.lines() {
                output.push_str(&format!("{:6}\t{}\n", line_number, line));
                *line_number += 1;
            }
        } else {
            output.push_str(&content);
        }
    } else {
        // For text files, process line by line efficiently
        if number_lines {
            let content = std::str::from_utf8(&mmap)
                .with_context(|| format!("cat: {}: Invalid UTF-8", path))?;

            for line in content.lines() {
                output.push_str(&format!("{:6}\t{}\n", line_number, line));
                *line_number += 1;
            }
            // Add final newline if the file doesn't end with one
            if !content.is_empty() && !content.ends_with('\n') {
                // Already added newline in the loop
            }
        } else {
            // Fast path: just convert to string and append
            let content = std::str::from_utf8(&mmap)
                .with_context(|| format!("cat: {}: Invalid UTF-8", path))?;
            output.push_str(content);
        }
    }

    Ok(())
}

/// Read from a buffered reader with optional line numbering
fn read_with_line_numbers<R: BufRead>(
    reader: R,
    output: &mut String,
    line_number: &mut usize,
    number_lines: bool,
) -> Result<()> {
    if number_lines {
        for line_result in reader.lines() {
            let line = line_result.context("cat: Failed to read line")?;
            output.push_str(&format!("{:6}\t{}\n", line_number, line));
            *line_number += 1;
        }
    } else {
        for line_result in reader.lines() {
            let line = line_result.context("cat: Failed to read line")?;
            output.push_str(&line);
            output.push('\n');
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    fn create_large_test_file(lines: usize) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..lines {
            writeln!(file, "This is line number {}", i).unwrap();
        }
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_cat_single_file() {
        let file = create_test_file("Hello, World!\n");
        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        assert_eq!(result.stdout(), "Hello, World!\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_multiple_files() {
        let file1 = create_test_file("First file\n");
        let file2 = create_test_file("Second file\n");

        let path1 = file1.path().to_str().unwrap();
        let path2 = file2.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path1.to_string(), path2.to_string()], &mut runtime).unwrap();

        assert_eq!(result.stdout(), "First file\nSecond file\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_with_line_numbers() {
        let file = create_test_file("Line 1\nLine 2\nLine 3\n");
        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&["-n".to_string(), path.to_string()], &mut runtime).unwrap();

        assert_eq!(
            result.stdout(),
            "     1\tLine 1\n     2\tLine 2\n     3\tLine 3\n"
        );
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_nonexistent_file() {
        let mut runtime = Runtime::new();
        let result = builtin_cat(&["/nonexistent/file.txt".to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("No such file or directory"));
    }

    #[test]
    fn test_cat_empty_file() {
        let file = create_test_file("");
        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        assert_eq!(result.stdout(), "");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_small_file_no_mmap() {
        // Create a file smaller than MMAP_THRESHOLD (1MB)
        let file = create_test_file("Small file content\n");
        let path = file.path().to_str().unwrap();
        let metadata = fs::metadata(path).unwrap();

        // Verify it's small enough to not use mmap
        assert!(metadata.len() < MMAP_THRESHOLD);

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        assert_eq!(result.stdout(), "Small file content\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_large_file_with_mmap() {
        // Create a file larger than MMAP_THRESHOLD (1MB)
        // Need about 50,000 lines to exceed 1MB
        let file = create_large_test_file(50_000);
        let path = file.path().to_str().unwrap();
        let metadata = fs::metadata(path).unwrap();

        // Verify it's large enough to use mmap
        assert!(metadata.len() > MMAP_THRESHOLD);

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        // Verify the output contains the expected content
        assert!(result.stdout().contains("This is line number 0"));
        assert!(result.stdout().contains("This is line number 49999"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_large_file_with_line_numbers() {
        // Create a large file
        let file = create_large_test_file(50_000);
        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&["-n".to_string(), path.to_string()], &mut runtime).unwrap();

        // Verify line numbers are present
        assert!(result.stdout().contains("     1\tThis is line number 0"));
        assert!(result
            .stdout()
            .contains(" 50000\tThis is line number 49999"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_binary_file() {
        let mut file = NamedTempFile::new().unwrap();
        // Write some binary data with null bytes
        let binary_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0x00, b'H', b'i', 0x00];
        file.write_all(&binary_data).unwrap();
        file.flush().unwrap();

        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        // Binary files should still produce output (with replacement chars for invalid UTF-8)
        assert!(!result.stdout().is_empty());
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_multiple_files_with_line_numbers() {
        let file1 = create_test_file("File 1 Line 1\nFile 1 Line 2\n");
        let file2 = create_test_file("File 2 Line 1\nFile 2 Line 2\n");

        let path1 = file1.path().to_str().unwrap();
        let path2 = file2.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(
            &["-n".to_string(), path1.to_string(), path2.to_string()],
            &mut runtime,
        )
        .unwrap();

        // Line numbers should be continuous across files
        assert!(result.stdout().contains("     1\tFile 1 Line 1"));
        assert!(result.stdout().contains("     2\tFile 1 Line 2"));
        assert!(result.stdout().contains("     3\tFile 2 Line 1"));
        assert!(result.stdout().contains("     4\tFile 2 Line 2"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_cat_invalid_option() {
        let file = create_test_file("test\n");
        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&["-x".to_string(), path.to_string()], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }

    #[test]
    fn test_cat_no_trailing_newline() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"No newline at end").unwrap();
        file.flush().unwrap();

        let path = file.path().to_str().unwrap();

        let mut runtime = Runtime::new();
        let result = builtin_cat(&[path.to_string()], &mut runtime).unwrap();

        assert_eq!(result.stdout(), "No newline at end");
        assert_eq!(result.exit_code, 0);
    }
}
