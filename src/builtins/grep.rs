use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use serde::Serialize;
use std::cell::Cell;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct GrepMatch {
    file: String,
    line_number: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(rename = "match")]
    match_text: String,
    full_line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_after: Option<Vec<String>>,
}

pub fn builtin_grep(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut config = parse_args(args)?;
    if runtime.agent_mode() {
        config.json_output = true;
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut json_matches = Vec::new();
    let mut found_any = false;

    // Build the regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(config.ignore_case)
        .build(&config.pattern)
        .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

    if config.json_output {
        // JSON mode
        if config.recursive {
            for path in &config.paths {
                let walker = WalkBuilder::new(path)
                    .git_ignore(config.respect_gitignore)
                    .hidden(!config.search_hidden)
                    .build();

                for entry in walker {
                    match entry {
                        Ok(entry) => {
                            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                                if search_file_json(
                                    &matcher,
                                    entry.path(),
                                    &config,
                                    &mut json_matches,
                                    &mut stderr,
                                )? {
                                    found_any = true;
                                }
                            }
                        }
                        Err(e) => {
                            writeln!(&mut stderr, "Error walking directory: {}", e)?;
                        }
                    }
                }
            }
        } else {
            for path in &config.paths {
                let path_buf = if path.is_absolute() {
                    path.clone()
                } else {
                    runtime.get_cwd().join(path)
                };

                if path_buf.is_dir() {
                    writeln!(&mut stderr, "grep: {}: Is a directory", path.display())?;
                    continue;
                }

                if !path_buf.exists() {
                    writeln!(
                        &mut stderr,
                        "grep: {}: No such file or directory",
                        path.display()
                    )?;
                    continue;
                }

                if search_file_json(&matcher, &path_buf, &config, &mut json_matches, &mut stderr)? {
                    found_any = true;
                }
            }
        }

        let exit_code = if found_any { 0 } else { 1 };
        let json_value = serde_json::to_value(&json_matches)
            .map_err(|e| anyhow!("Failed to serialize JSON: {}", e))?;
        return Ok(ExecutionResult {
            output: crate::executor::Output::Structured(json_value),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code,
            error: None,
        });
    }

    // Text mode (original behavior)
    if config.recursive {
        for path in &config.paths {
            let walker = WalkBuilder::new(path)
                .git_ignore(config.respect_gitignore)
                .hidden(!config.search_hidden)
                .build();

            for entry in walker {
                match entry {
                    Ok(entry) => {
                        if entry.file_type().is_some_and(|ft| ft.is_file())
                            && search_file(
                                &matcher,
                                entry.path(),
                                &config,
                                &mut stdout,
                                &mut stderr,
                            )?
                        {
                            found_any = true;
                        }
                    }
                    Err(e) => {
                        writeln!(&mut stderr, "Error walking directory: {}", e)?;
                    }
                }
            }
        }
    } else {
        for path in &config.paths {
            let path_buf = if path.is_absolute() {
                path.clone()
            } else {
                runtime.get_cwd().join(path)
            };

            if path_buf.is_dir() {
                writeln!(&mut stderr, "grep: {}: Is a directory", path.display())?;
                continue;
            }

            if !path_buf.exists() {
                writeln!(
                    &mut stderr,
                    "grep: {}: No such file or directory",
                    path.display()
                )?;
                continue;
            }

            if search_file(&matcher, &path_buf, &config, &mut stdout, &mut stderr)? {
                found_any = true;
            }
        }
    }

    let exit_code = if found_any { 0 } else { 1 };

    Ok(ExecutionResult {
        output: crate::executor::Output::Text(String::from_utf8_lossy(&stdout).to_string()),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_code,
        error: None,
    })
}

/// Execute grep with stdin data (for pipelines)
pub fn builtin_grep_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    let mut config = parse_args(args)?;
    if runtime.agent_mode() {
        config.json_output = true;
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut json_matches = Vec::new();

    // Build the regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(config.ignore_case)
        .build(&config.pattern)
        .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

    let found = if config.json_output {
        search_stdin_json(&matcher, stdin_data, &config, &mut json_matches)?
    } else {
        search_stdin(&matcher, stdin_data, &config, &mut stdout, &mut stderr)?
    };

    let exit_code = if found { 0 } else { 1 };

    if config.json_output {
        let json_value = serde_json::to_value(&json_matches)
            .map_err(|e| anyhow!("Failed to serialize JSON: {}", e))?;
        return Ok(ExecutionResult {
            output: crate::executor::Output::Structured(json_value),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code,
            error: None,
        });
    }

    Ok(ExecutionResult {
        output: crate::executor::Output::Text(String::from_utf8_lossy(&stdout).to_string()),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        exit_code,
        error: None,
    })
}

/// Search through stdin data
fn search_stdin(
    matcher: &impl Matcher,
    stdin_data: &[u8],
    config: &GrepConfig,
    stdout: &mut Vec<u8>,
    _stderr: &mut Vec<u8>,
) -> Result<bool> {
    let found = Cell::new(false);

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true) // Always track line numbers so lnum is valid
        .invert_match(config.invert_match)
        .build();

    let result = searcher.search_slice(
        matcher,
        stdin_data,
        UTF8(|lnum, line| {
            found.set(true);

            if config.show_line_numbers {
                write!(stdout, "{}:", lnum)?;
            }

            // Write line with color highlighting if enabled
            if config.color && !config.invert_match {
                if let Some(m) = matcher
                    .find(line.as_bytes())
                    .map_err(|e| std::io::Error::other(e.to_string()))?
                {
                    write_colored_line(stdout, line, m.start(), m.end())
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                } else {
                    write!(stdout, "{}", line)?;
                }
            } else {
                write!(stdout, "{}", line)?;
            }

            Ok(true)
        }),
    );

    match result {
        Ok(_) => Ok(found.get()),
        Err(e) => Err(anyhow!("Error searching stdin: {}", e)),
    }
}

fn search_file(
    matcher: &impl Matcher,
    path: &Path,
    config: &GrepConfig,
    stdout: &mut Vec<u8>,
    stderr: &mut Vec<u8>,
) -> Result<bool> {
    let found = Cell::new(false);

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true) // Always track line numbers so lnum is valid
        .invert_match(config.invert_match)
        .build();

    let result = searcher.search_path(
        matcher,
        path,
        UTF8(|lnum, line| {
            found.set(true);

            if config.show_line_numbers {
                write!(stdout, "{}:", lnum)?;
            }

            if config.show_filename {
                write!(stdout, "{}:", path.display())?;
            }

            // Write line with color highlighting if enabled (only for normal matches, not inverted)
            if config.color && !config.invert_match {
                // Find first match for highlighting
                let match_result = matcher
                    .find(line.as_bytes())
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                if let Some(m) = match_result {
                    write_colored_line(stdout, line, m.start(), m.end())
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                } else {
                    write!(stdout, "{}", line)?;
                }
            } else {
                write!(stdout, "{}", line)?;
            }

            Ok(true)
        }),
    );

    match result {
        Ok(_) => Ok(found.get()),
        Err(e) => {
            writeln!(stderr, "Error searching {}: {}", path.display(), e)?;
            Ok(false)
        }
    }
}

/// Search file and return JSON matches
fn search_file_json(
    matcher: &impl Matcher,
    path: &Path,
    config: &GrepConfig,
    matches: &mut Vec<GrepMatch>,
    stderr: &mut Vec<u8>,
) -> Result<bool> {
    let result = std::fs::read(path);
    let content = match result {
        Ok(content) => content,
        Err(e) => {
            writeln!(stderr, "Error reading {}: {}", path.display(), e)?;
            return Ok(false);
        }
    };

    let found = search_lines_json(
        matcher,
        &content,
        path.to_string_lossy().to_string(),
        config,
        matches,
    )?;
    Ok(found)
}

/// Search stdin and return JSON matches
fn search_stdin_json(
    matcher: &impl Matcher,
    stdin_data: &[u8],
    config: &GrepConfig,
    matches: &mut Vec<GrepMatch>,
) -> Result<bool> {
    search_lines_json(
        matcher,
        stdin_data,
        "(standard input)".to_string(),
        config,
        matches,
    )
}

/// Core JSON search logic with context support
fn search_lines_json(
    matcher: &impl Matcher,
    data: &[u8],
    file_name: String,
    config: &GrepConfig,
    matches: &mut Vec<GrepMatch>,
) -> Result<bool> {
    let content = String::from_utf8_lossy(data);
    let lines: Vec<&str> = content.lines().collect();

    let mut found = false;
    let mut context_buffer: VecDeque<String> = VecDeque::new();
    let mut pending_after_context: Vec<(usize, usize)> = Vec::new(); // (match_index, remaining_lines)

    for (line_num, line) in lines.iter().enumerate() {
        let line_number = line_num + 1;

        // Update context buffer (keep last N+1 lines: N for before-context + current line)
        if config.context_before > 0 {
            if context_buffer.len() > config.context_before {
                context_buffer.pop_front();
            }
            context_buffer.push_back(line.to_string());
        }

        // Check if this line matches
        let is_match = matcher
            .is_match(line.as_bytes())
            .map_err(|e| anyhow!("Matcher error: {}", e))?;

        let should_include = if config.invert_match {
            !is_match
        } else {
            is_match
        };

        if should_include {
            found = true;

            // Extract the match text
            let match_info = if !config.invert_match {
                matcher
                    .find(line.as_bytes())
                    .map_err(|e| anyhow!("Matcher error: {}", e))?
            } else {
                None
            };

            let (match_text, column) = if let Some(m) = match_info {
                (line[m.start()..m.end()].to_string(), Some(m.start()))
            } else {
                (String::new(), None)
            };

            // Build context_before from buffer
            let context_before = if config.context_before > 0 && context_buffer.len() > 1 {
                // Remove the current line from buffer
                let before_lines: Vec<String> = context_buffer
                    .iter()
                    .take(context_buffer.len() - 1)
                    .cloned()
                    .collect();
                if !before_lines.is_empty() {
                    Some(before_lines)
                } else {
                    None
                }
            } else {
                None
            };

            matches.push(GrepMatch {
                file: file_name.clone(),
                line_number: line_number as u64,
                column,
                match_text,
                full_line: line.to_string(),
                context_before,
                context_after: None, // Will be filled later
            });

            // Track that we need to collect after-context
            if config.context_after > 0 {
                pending_after_context.push((matches.len() - 1, config.context_after));
            }

            // Clear context buffer after match to avoid duplicates
            context_buffer.clear();
        } else {
            // This line doesn't match, but might be part of after-context
            let mut i = 0;
            while i < pending_after_context.len() {
                let (match_idx, remaining) = &mut pending_after_context[i];
                if *remaining > 0 {
                    // Add this line to the match's after context
                    if matches[*match_idx].context_after.is_none() {
                        matches[*match_idx].context_after = Some(Vec::new());
                    }
                    if let Some(ref mut after) = matches[*match_idx].context_after {
                        after.push(line.to_string());
                    }
                    *remaining -= 1;
                }

                if *remaining == 0 {
                    pending_after_context.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }

    Ok(found)
}

fn write_colored_line(
    stdout: &mut Vec<u8>,
    line: &str,
    match_start: usize,
    match_end: usize,
) -> Result<()> {
    // ANSI color codes for red highlighting
    const RED: &str = "\x1b[1;31m";
    const RESET: &str = "\x1b[0m";

    write!(
        stdout,
        "{}{}{}{}{}",
        &line[..match_start],
        RED,
        &line[match_start..match_end],
        RESET,
        &line[match_end..]
    )?;

    Ok(())
}

#[derive(Debug)]
struct GrepConfig {
    pattern: String,
    paths: Vec<PathBuf>,
    ignore_case: bool,
    show_line_numbers: bool,
    recursive: bool,
    invert_match: bool,
    respect_gitignore: bool,
    search_hidden: bool,
    show_filename: bool,
    color: bool,
    json_output: bool,
    context_before: usize,
    context_after: usize,
}

impl Default for GrepConfig {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            paths: vec![PathBuf::from(".")],
            ignore_case: false,
            show_line_numbers: true,
            recursive: false,
            invert_match: false,
            respect_gitignore: true,
            search_hidden: false,
            show_filename: false,
            color: true,
            json_output: false,
            context_before: 0,
            context_after: 0,
        }
    }
}

fn parse_args(args: &[String]) -> Result<GrepConfig> {
    let mut config = GrepConfig::default();
    let mut i = 0;

    // Parse flags
    while i < args.len() {
        let arg = &args[i];

        if !arg.starts_with('-') {
            break;
        }

        match arg.as_str() {
            "-i" | "--ignore-case" => {
                config.ignore_case = true;
            }
            "-n" | "--line-number" => {
                config.show_line_numbers = true;
            }
            "-N" | "--no-line-number" => {
                config.show_line_numbers = false;
            }
            "-r" | "-R" | "--recursive" => {
                config.recursive = true;
            }
            "-v" | "--invert-match" => {
                config.invert_match = true;
            }
            "-H" | "--with-filename" => {
                config.show_filename = true;
            }
            "-h" | "--no-filename" => {
                config.show_filename = false;
            }
            "--color" => {
                config.color = true;
            }
            "--no-color" => {
                config.color = false;
            }
            "--hidden" => {
                config.search_hidden = true;
            }
            "--no-ignore" => {
                config.respect_gitignore = false;
            }
            "--json" => {
                config.json_output = true;
            }
            "-C" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("grep: -C requires an argument"));
                }
                let lines: usize = args[i]
                    .parse()
                    .map_err(|_| anyhow!("grep: -C must be a non-negative integer"))?;
                config.context_before = lines;
                config.context_after = lines;
            }
            "-A" | "--after-context" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("grep: -A requires an argument"));
                }
                config.context_after = args[i]
                    .parse()
                    .map_err(|_| anyhow!("grep: -A must be a non-negative integer"))?;
            }
            "-B" | "--before-context" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("grep: -B requires an argument"));
                }
                config.context_before = args[i]
                    .parse()
                    .map_err(|_| anyhow!("grep: -B must be a non-negative integer"))?;
            }
            "--help" => {
                return Err(anyhow!(
                    "Usage: grep [OPTIONS] PATTERN [PATH...]\n\
                     \n\
                     OPTIONS:\n\
                     -i, --ignore-case       Case insensitive search\n\
                     -n, --line-number       Show line numbers (default)\n\
                     -N, --no-line-number    Don't show line numbers\n\
                     -r, --recursive         Recursively search directories\n\
                     -v, --invert-match      Select non-matching lines\n\
                     -H, --with-filename     Show filename (default for multiple files)\n\
                     -h, --no-filename       Don't show filename\n\
                     -C NUM                  Show NUM lines of context\n\
                     -A NUM, --after-context Show NUM lines after match\n\
                     -B NUM, --before-context Show NUM lines before match\n\
                     --color                 Colorize output (default)\n\
                     --no-color              Don't colorize output\n\
                     --hidden                Search hidden files\n\
                     --no-ignore             Don't respect .gitignore\n\
                     --json                  Output results in JSON format\n\
                     --help                  Show this help message"
                ));
            }
            _ => {
                return Err(anyhow!("Unknown option: {}", arg));
            }
        }

        i += 1;
    }

    // Get pattern
    if i >= args.len() {
        return Err(anyhow!("grep: missing pattern argument"));
    }
    config.pattern = args[i].clone();
    i += 1;

    // Get paths (default to current directory)
    if i < args.len() {
        config.paths = args[i..].iter().map(PathBuf::from).collect();

        // Auto-enable filename display for multiple files
        if config.paths.len() > 1 {
            config.show_filename = true;
        }
    }

    Ok(config)
}

// Tests are in tests/grep_integration_test.rs
