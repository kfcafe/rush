#![allow(dead_code, unused_imports)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod ai;
mod arithmetic;
mod banner;
mod benchmark;
mod builtins;
mod compat;
mod completion;
mod context;
mod correction;
mod daemon;
mod error;
mod executor;
#[cfg(feature = "git-builtins")]
mod git;
mod glob_expansion;
mod highlight;
mod history;
mod intent;
mod jobs;
mod lexer;
mod lua;
mod output;
mod parser;
mod progress;
mod run_api;
mod runtime;
mod signal;
mod stats;
mod terminal;
mod undo;
mod value;

use anyhow::Result;
use completion::Completer;
use executor::Executor;
use lexer::Lexer;
use libc;
use nix::unistd::{getpid, setpgid};
use parser::Parser;
use reedline::{Prompt, PromptHistorySearch, PromptHistorySearchStatus, Reedline, Signal};
use signal::SignalHandler;
use std::borrow::Cow;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, RwLock};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Fast path: detect -c flag early and skip all expensive initialization.
    // This avoids: process group setup, signal handler thread, daemon probe,
    // init_environment_variables, and whoami calls — saving ~5-8ms.
    let mut enable_profile = false;
    let mut profile_json = false;
    let mut max_output_str: Option<String> = None;
    {
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--profile" => {
                    enable_profile = true;
                    i += 1;
                }
                "--json" => {
                    profile_json = true;
                    i += 1;
                }
                "--max-output" if i + 1 < args.len() => {
                    max_output_str = Some(args[i + 1].clone());
                    i += 2;
                }
                "-c" if i + 1 < args.len() => {
                    fast_execute_c(
                        &args[i + 1],
                        enable_profile,
                        profile_json,
                        max_output_str.as_deref(),
                    );
                    // fast_execute_c never returns (calls process::exit)
                }
                "--check" if i + 1 < args.len() => {
                    // Handle compatibility check with optional --migrate and --fix flags
                    let mut show_migrate = false;
                    let mut apply_fix = false;
                    let mut j = i + 2;

                    while j < args.len() {
                        match args[j].as_str() {
                            "--migrate" => {
                                show_migrate = true;
                                j += 1;
                            }
                            "--fix" => {
                                apply_fix = true;
                                j += 1;
                            }
                            _ => break,
                        }
                    }

                    run_compatibility_check(&args[i + 1], show_migrate, apply_fix);
                    // run_compatibility_check calls process::exit
                }
                "--benchmark" if i + 1 < args.len() => {
                    // Handle benchmark mode
                    let mode = match args[i + 1].as_str() {
                        "quick" => benchmark::BenchmarkMode::Quick,
                        "full" => benchmark::BenchmarkMode::Full,
                        "compare" => benchmark::BenchmarkMode::Compare,
                        _ => {
                            eprintln!(
                                "Invalid benchmark mode: {}. Use 'quick', 'full', or 'compare'",
                                args[i + 1]
                            );
                            std::process::exit(1);
                        }
                    };
                    if let Err(e) = benchmark::run_benchmark(mode) {
                        eprintln!("Benchmark error: {}", e);
                        std::process::exit(1);
                    }
                    std::process::exit(0);
                }
                "--setup-ai" => {
                    // Run the interactive AI setup wizard
                    match rush::ai::setup_wizard() {
                        Ok(_) => std::process::exit(0),
                        Err(e) => {
                            eprintln!("Setup failed: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                "--info" => {
                    // Handle --info flag: show system stats
                    // Check for optional stat name or --json flag
                    let mut stat_name: Option<String> = None;
                    let mut json_output = false;
                    let mut j = i + 1;

                    while j < args.len() {
                        match args[j].as_str() {
                            "--json" => {
                                json_output = true;
                                j += 1;
                            }
                            arg if !arg.starts_with('-') => {
                                stat_name = Some(arg.to_string());
                                break; // Only take first non-flag arg
                            }
                            _ => break,
                        }
                    }

                    run_info_command(stat_name, json_output);
                    // run_info_command calls process::exit
                }
                "--login" | "-l" | "--no-rc" | "--norc" | "--no-config" => {
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }

    // Full initialization for interactive / script modes
    // Put the shell in its own process group for proper job control
    let shell_pid = getpid();
    if let Err(_e) = setpgid(shell_pid, shell_pid) {
        // Non-fatal warning - continue anyway (may fail if already session leader)
        // This is expected for login shells
    }

    // Take control of the terminal if we're interactive
    // This is critical for login shells - without it, reading from stdin
    // will cause SIGTTIN and the shell will hang
    if atty::is(atty::Stream::Stdin) {
        unsafe {
            // Shells must ignore SIGTTIN/SIGTTOU so they never stop when
            // doing terminal control operations. Keep these ignored permanently.
            libc::signal(libc::SIGTTIN, libc::SIG_IGN);
            libc::signal(libc::SIGTTOU, libc::SIG_IGN);

            // Take control of the terminal
            let stdin_fd = libc::STDIN_FILENO;
            let our_pgid = libc::getpgrp();
            if libc::tcsetpgrp(stdin_fd, our_pgid) < 0 {
                // Non-fatal - we may already be the foreground group
            }
            // Note: Do NOT restore SIGTTIN/SIGTTOU to default - shells must ignore them
        }
    }

    // Setup signal handlers early
    let signal_handler = SignalHandler::new();
    if let Err(e) = signal_handler.setup() {
        eprintln!("Warning: Failed to setup signal handlers: {}", e);
    }

    // Parse flags
    let mut is_login_shell = false;
    let mut skip_rc = false;
    let mut filtered_args = Vec::new();

    // Check if invoked as login shell (argv[0] starts with -)
    if let Some(arg0) = args.first() {
        if arg0.starts_with('-') || arg0.ends_with("/-rush") {
            is_login_shell = true;
        }
    }

    // Parse command-line flags
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--login" | "-l" => {
                is_login_shell = true;
                i += 1;
            }
            "--no-rc" | "--norc" | "--no-config" => {
                skip_rc = true;
                i += 1;
            }
            _ => {
                filtered_args.push(args[i].clone());
                i += 1;
            }
        }
    }

    // Show help for invalid usage
    if !filtered_args.is_empty() && (filtered_args[0] == "-h" || filtered_args[0] == "--help") {
        print_help();
        return Ok(());
    }

    // Check if a script file is provided
    if !filtered_args.is_empty() && !filtered_args[0].starts_with('-') {
        let script_path = &filtered_args[0];
        let script_args = filtered_args[1..].to_vec();
        return run_script(script_path, script_args, signal_handler);
    }

    // Run interactive mode (possibly as login shell)
    run_interactive_with_init(signal_handler, is_login_shell, skip_rc)
}

fn run_script(
    script_path: &str,
    script_args: Vec<String>,
    signal_handler: SignalHandler,
) -> Result<()> {
    // Initialize environment variables
    init_environment_variables()?;

    // Read the script file
    let script_content = fs::read_to_string(script_path)
        .map_err(|e| anyhow::anyhow!("Failed to read script '{}': {}", script_path, e))?;

    let mut executor = Executor::new_with_signal_handler(signal_handler.clone());

    // Set runtime variables from environment
    init_runtime_variables(executor.runtime_mut());

    // Set up positional parameters ($1, $2, etc.) and $#, $@, $*
    executor
        .runtime_mut()
        .set_positional_params(script_args.clone());

    // Set $0 to script name
    executor
        .runtime_mut()
        .set_variable("0".to_string(), script_path.to_string());

    // Strip shebang line if present
    let script_to_parse = if script_content.starts_with("#!") {
        // Find the first newline and skip the shebang line
        match script_content.find('\n') {
            Some(pos) => &script_content[pos + 1..],
            None => "", // Script is just a shebang line
        }
    } else {
        &script_content
    };

    // Tokenize the entire script
    let tokens = match Lexer::tokenize(script_to_parse) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {}", script_path, e);
            std::process::exit(2);
        }
    };

    // Parse the entire script into an AST
    let mut parser = Parser::new(tokens);
    let statements = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", script_path, e);
            std::process::exit(2);
        }
    };

    // Execute the AST
    match executor.execute(statements) {
        Ok(result) => {
            let stdout_text = result.stdout();
            if !stdout_text.is_empty() {
                print!("{}", stdout_text);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            std::process::exit(result.exit_code);
        }
        Err(e) => {
            // Check for exit signal (normal script termination via exit builtin)
            if let Some(exit_signal) = e.downcast_ref::<builtins::exit_builtin::ExitSignal>() {
                std::process::exit(exit_signal.exit_code);
            }
            eprintln!("{}: Error: {}", script_path, e);
            std::process::exit(1);
        }
    }
}

fn run_command(command: &str, signal_handler: SignalHandler) -> Result<()> {
    // Try to use daemon if available
    if let Ok(mut client) = rush::daemon::DaemonClient::new() {
        if client.is_daemon_running() {
            // Use daemon for execution
            let args = vec!["-c".to_string(), command.to_string()];
            match client.execute_command(&args) {
                Ok(exit_code) => {
                    std::process::exit(exit_code);
                }
                Err(e) => {
                    eprintln!("Daemon error: {}, falling back to direct execution", e);
                    // Fall through to direct execution
                }
            }
        }
    }

    // Fall back to direct execution
    // Initialize environment variables
    init_environment_variables()?;

    let mut executor = Executor::new_with_signal_handler(signal_handler.clone());

    // Set runtime variables from environment
    init_runtime_variables(executor.runtime_mut());

    match execute_line(command, &mut executor) {
        Ok(result) => {
            let stdout_text = result.stdout();
            if !stdout_text.is_empty() {
                print!("{}", stdout_text);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }

            // Check if interrupted by signal
            if signal_handler.should_shutdown() {
                std::process::exit(signal_handler.exit_code());
            }

            // Exit with the command's exit code
            std::process::exit(result.exit_code);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Interactive prompt showing cwd, git branch, and last exit code.
struct RushPrompt {
    last_exit_code: std::sync::atomic::AtomicI32,
}

impl RushPrompt {
    fn new() -> Self {
        Self {
            last_exit_code: std::sync::atomic::AtomicI32::new(0),
        }
    }

    fn set_exit_code(&self, code: i32) {
        self.last_exit_code
            .store(code, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_prompt_indicator(&self) -> String {
        // If RUSH_PROMPT is set, expand its tokens. Otherwise use the default.
        if let Ok(fmt) = env::var("RUSH_PROMPT") {
            return self.expand_prompt(&fmt);
        }

        let cwd_path = env::current_dir().ok();
        let cwd = cwd_path
            .as_deref()
            .map(terminal::shorten_home)
            .unwrap_or_else(|| "?".to_string());

        let mut prompt = cwd;

        // Git branch (fast — reads .git/HEAD, no subprocess)
        if let Some(ref path) = cwd_path {
            if let Some(branch) = terminal::git_branch_fast(path) {
                prompt.push_str(&format!(" \x1b[33m({})\x1b[0m", branch));
            }
        }

        // Non-zero exit code
        let code = self
            .last_exit_code
            .load(std::sync::atomic::Ordering::Relaxed);
        if code != 0 {
            prompt.push_str(&format!(" \x1b[31m[{}]\x1b[0m", code));
        }

        prompt.push_str("\x1b[36m>\x1b[0m ");
        prompt
    }

    /// Expand prompt format tokens:
    ///   {cwd}     — shortened working directory
    ///   {git}     — current branch or empty
    ///   {exit}    — last exit code or empty if 0
    ///   {user}    — username
    ///   {host}    — hostname
    ///   {time}    — HH:MM
    ///   {#rrggbb} — set foreground color
    ///   {bold}    — bold
    ///   {dim}     — dim
    ///   {reset}   — reset styling
    ///   {nl}      — newline
    fn expand_prompt(&self, fmt: &str) -> String {
        let cwd_path = env::current_dir().ok();
        let cwd = cwd_path
            .as_deref()
            .map(terminal::shorten_home)
            .unwrap_or_else(|| "?".to_string());

        let git_branch = cwd_path
            .as_ref()
            .and_then(|p| terminal::git_branch_fast(p))
            .unwrap_or_default();

        let exit_code = self
            .last_exit_code
            .load(std::sync::atomic::Ordering::Relaxed);

        let user = env::var("USER").unwrap_or_default();
        let host = env::var("HOSTNAME")
            .or_else(|_| env::var("HOST"))
            .unwrap_or_default();

        let now = chrono::Local::now();
        let time = now.format("%H:%M").to_string();

        let mut result = String::new();
        let mut chars = fmt.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '{' {
                let token: String = chars.by_ref().take_while(|&c| c != '}').collect();
                match token.as_str() {
                    "cwd" => result.push_str(&cwd),
                    "git" => result.push_str(&git_branch),
                    "exit" => {
                        if exit_code != 0 {
                            result.push_str(&exit_code.to_string());
                        }
                    }
                    "user" => result.push_str(&user),
                    "host" => result.push_str(&host),
                    "time" => result.push_str(&time),
                    "bold" => result.push_str("\x1b[1m"),
                    "dim" => result.push_str("\x1b[2m"),
                    "italic" => result.push_str("\x1b[3m"),
                    "reset" => result.push_str("\x1b[0m"),
                    "nl" => result.push('\n'),
                    s if s.starts_with('#') && s.len() == 7 => {
                        // Hex color: {#ff5f87}
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            u8::from_str_radix(&s[1..3], 16),
                            u8::from_str_radix(&s[3..5], 16),
                            u8::from_str_radix(&s[5..7], 16),
                        ) {
                            result.push_str(&format!("\x1b[38;2;{};{};{}m", r, g, b));
                        }
                    }
                    _ => {
                        // Unknown token — pass through literally
                        result.push('{');
                        result.push_str(&token);
                        result.push('}');
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }
}

impl Prompt for RushPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(self.get_prompt_indicator())
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        if let Ok(fmt) = env::var("RUSH_PROMPT_RIGHT") {
            Cow::Owned(self.expand_prompt(&fmt))
        } else {
            Cow::Borrowed("")
        }
    }

    fn render_prompt_indicator(&self, _prompt_mode: reedline::PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };

        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}

fn run_interactive_with_init(
    signal_handler: SignalHandler,
    is_login: bool,
    skip_rc: bool,
) -> Result<()> {
    // Initialize environment variables
    init_environment_variables()?;

    // Create executor early so we can source files
    let mut executor = Executor::new_with_signal_handler(signal_handler.clone());

    // Set runtime variables from environment
    init_runtime_variables(executor.runtime_mut());

    // Source profile files based on login shell and flags
    if is_login && !skip_rc {
        // Login shell: source ~/.rush_profile
        if let Some(home) = dirs::home_dir() {
            let profile = home.join(".rush_profile");
            if let Err(e) = executor.source_file(&profile) {
                eprintln!("Warning: Error sourcing ~/.rush_profile: {}", e);
            }
        }
    }

    // Interactive shell: source ~/.rushrc (unless --no-rc)
    if !skip_rc {
        if let Some(home) = dirs::home_dir() {
            let rushrc = home.join(".rushrc");
            if let Err(e) = executor.source_file(&rushrc) {
                eprintln!("Warning: Error sourcing ~/.rushrc: {}", e);
            }
        }
    }

    // Now run interactive mode, passing the executor so .rushrc
    // settings (aliases, functions, variables) are preserved.
    if atty::is(atty::Stream::Stdin) {
        run_interactive_with_reedline(signal_handler, executor)
    } else {
        run_non_interactive(signal_handler, executor)
    }
}

fn init_environment_variables() -> Result<()> {
    // Set $SHELL only if not already set (avoids expensive current_exe() readlink)
    if env::var("SHELL").is_err() {
        if let Ok(exe) = env::current_exe() {
            env::set_var("SHELL", exe);
        }
    }

    // Set $TERM if not already set
    if env::var("TERM").is_err() {
        env::set_var("TERM", "xterm-256color");
    }

    // Set $USER if not already set (avoids expensive whoami syscall)
    if env::var("USER").is_err() {
        if let Ok(user) = env::var("LOGNAME") {
            env::set_var("USER", user);
        } else if let Some(user) = whoami::username_os().to_str() {
            env::set_var("USER", user);
        }
    }

    // Set $HOME if not already set
    if env::var("HOME").is_err() {
        if let Some(home) = dirs::home_dir() {
            env::set_var("HOME", home);
        }
    }

    Ok(())
}

fn init_runtime_variables(runtime: &mut runtime::Runtime) {
    // Set runtime variables from environment
    if let Ok(shell) = env::var("SHELL") {
        runtime.set_variable("SHELL".to_string(), shell);
    }
    if let Ok(term) = env::var("TERM") {
        runtime.set_variable("TERM".to_string(), term);
    }
    if let Ok(user) = env::var("USER") {
        runtime.set_variable("USER".to_string(), user);
    }
    if let Ok(home) = env::var("HOME") {
        runtime.set_variable("HOME".to_string(), home);
    }

    // Set PATH from environment (required for command execution)
    if let Ok(path) = env::var("PATH") {
        runtime.set_variable("PATH".to_string(), path);
    }

    // Set PWD to current working directory
    if let Ok(pwd) = env::current_dir() {
        runtime.set_variable("PWD".to_string(), pwd.to_string_lossy().to_string());
    }

    // Set PPID (parent process ID) - readonly on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::process::parent_id;
        runtime.set_variable("PPID".to_string(), parent_id().to_string());
        runtime.mark_readonly("PPID".to_string());
    }

    // Set SHLVL (shell nesting level)
    // Read from environment, default to 0, then increment by 1
    let shlvl = env::var("SHLVL")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
        + 1;
    runtime.set_variable("SHLVL".to_string(), shlvl.to_string());
    // Also update environment variable for child processes
    env::set_var("SHLVL", shlvl.to_string());
}

/// Fetch stats from daemon for banner display
/// Returns None if daemon not running or stats fetch fails (graceful degradation)
fn fetch_banner_stats(requested_stats: &[String]) -> Option<banner::StatsData> {
    use daemon::client::DaemonClient;

    // Try to create client and check if daemon is running
    let mut client = match DaemonClient::new() {
        Ok(c) => c,
        Err(_) => return None,
    };

    if !client.is_daemon_running() {
        return None;
    }

    // Connect and fetch stats
    if client.connect().is_err() {
        return None;
    }

    match client.try_fetch_stats(requested_stats.to_vec()) {
        Some(response) => Some(banner::StatsData {
            builtin: response.builtin,
            custom: response.custom,
        }),
        None => None,
    }
}

fn run_interactive_with_reedline(
    signal_handler: SignalHandler,
    mut executor: Executor,
) -> Result<()> {
    // Report CWD immediately so Ghostty/iTerm2 can open new tabs here
    terminal::emit_osc7_cwd();

    // Load banner configuration from environment (set by .rushrc)
    let banner_config = banner::BannerConfig::from_env();

    // Increment RUSH_LEVEL for nested shell detection
    banner::increment_rush_level();

    // Fetch stats from daemon if configured and daemon is running
    let stats_data = if !banner_config.stats.is_empty() {
        fetch_banner_stats(&banner_config.stats)
    } else {
        None
    };

    // Display the banner with optional stats
    banner::display_banner(&banner_config, stats_data.as_ref());

    // Create completer with shared builtins and runtime
    let builtins = Arc::new(builtins::Builtins::new());
    let runtime = Arc::new(RwLock::new(runtime::Runtime::new()));
    let completer = Box::new(Completer::new(builtins.clone(), runtime.clone()));
    let highlighter = Box::new(highlight::RushHighlighter::new(builtins.clone()));

    let mut line_editor = Reedline::create()
        .with_completer(completer)
        .with_highlighter(highlighter);
    let prompt = RushPrompt::new();

    // Bell threshold for long-running commands (default 10s, 0 = disabled)
    let bell_threshold = std::time::Duration::from_secs(
        env::var("RUSH_BELL_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10),
    );

    // Track last command and exit code for intent context
    let mut last_command: Option<String> = None;
    let mut last_exit_code: Option<i32> = None;
    let mut command_history: Vec<String> = Vec::new();
    const MAX_HISTORY_FOR_CONTEXT: usize = 20;

    loop {
        // Check if stdin is still a valid terminal.
        // On macOS, closing a terminal tab "revokes" the PTY, leaving the
        // shell process alive with dead file descriptors.  Without this
        // guard, reedline enters a tight poll/read error loop and the
        // process never exits — accumulating CPU time and inflating
        // Activity Monitor memory counts.
        if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
            break;
        }

        // Check for signals before reading next line
        if signal_handler.should_shutdown() {
            // Use write(2) directly — println! panics on revoked stdout
            let _ =
                std::io::Write::write_all(&mut std::io::stderr(), b"\nExiting due to signal...\n");
            std::process::exit(signal_handler.exit_code());
        }

        // Check for SIGCHLD and reap any zombie processes
        if signal_handler.sigchld_received() {
            executor.runtime_mut().job_manager().reap_zombies();
            signal_handler.clear_sigchld();
        }

        // Update job statuses and cleanup completed jobs
        executor.runtime_mut().job_manager().update_all_jobs();

        // Print notifications for completed jobs
        let jobs = executor.runtime_mut().job_manager().list_jobs();
        for job in jobs {
            if job.status == jobs::JobStatus::Done {
                println!("[{}] Done\t\t{}", job.id, job.command);
            } else if job.status == jobs::JobStatus::Terminated {
                println!("[{}] Terminated\t{}", job.id, job.command);
            }
        }

        // Cleanup completed/terminated jobs
        executor.runtime_mut().job_manager().cleanup_jobs();

        // Sync stored cwd with OS (handles external renames while a child process was running)
        executor.runtime_mut().refresh_cwd();

        // Terminal integration: prompt marking, tab title, working directory
        terminal::mark_prompt_start();
        terminal::emit_osc7_cwd();
        terminal::set_terminal_title_to_cwd();

        let sig = line_editor.read_line(&prompt);

        match sig {
            Ok(Signal::Success(buffer)) => {
                let line = buffer.trim();

                if line.is_empty() {
                    continue;
                }

                // Check for intent query (? prefix)
                if intent::is_intent_query(line) {
                    let intent_text = intent::extract_intent(line);

                    if intent_text.is_empty() {
                        eprintln!("Usage: ? <natural language intent>");
                        eprintln!("Example: ? find all rust files modified today");
                        continue;
                    }

                    // Process the intent
                    let result = intent::process_intent(
                        intent_text,
                        last_command.as_deref(),
                        last_exit_code,
                        command_history.clone(),
                        &mut executor,
                    );

                    match result {
                        intent::IntentResult::Accept(command) => {
                            // Execute the suggested command
                            println!("Executing: {}", command);
                            match execute_line(&command, &mut executor) {
                                Ok(exec_result) => {
                                    let agent_mode = executor.runtime_mut().agent_mode();
                                    let rendered =
                                        executor::render_output(&exec_result.output, agent_mode);
                                    if !rendered.is_empty() {
                                        print!("{}", rendered);
                                    }
                                    if !exec_result.stderr.is_empty() {
                                        eprintln!("{}", exec_result.stderr);
                                    }
                                    // Update history with the executed command
                                    last_command = Some(command.clone());
                                    last_exit_code = Some(exec_result.exit_code);
                                    command_history.push(command);
                                    if command_history.len() > MAX_HISTORY_FOR_CONTEXT {
                                        command_history.remove(0);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    last_exit_code = Some(1);
                                }
                            }
                        }
                        intent::IntentResult::Edit(command) => {
                            // Show the command for the user to copy/edit
                            // In a more advanced implementation, we'd pre-fill the line editor
                            println!("Copy and edit this command:");
                            println!("  {}", command);
                        }
                        intent::IntentResult::Cancel => {
                            // User cancelled - do nothing
                        }
                        intent::IntentResult::Error(e) => {
                            eprintln!("Intent error: {}", e);
                        }
                    }
                    continue;
                }

                // Terminal: mark command start, show running command in tab title
                terminal::mark_command_start();
                terminal::mark_output_start();
                terminal::set_terminal_title(line);

                // Normal command execution (timed for bell notification)
                let cmd_start = std::time::Instant::now();
                match execute_line(line, &mut executor) {
                    Ok(result) => {
                        let elapsed = cmd_start.elapsed();
                        let agent_mode = executor.runtime_mut().agent_mode();
                        let rendered = executor::render_output(&result.output, agent_mode);
                        if !rendered.is_empty() {
                            print!("{}", rendered);
                        }
                        if !result.stderr.is_empty() {
                            eprintln!("{}", result.stderr);
                        }
                        terminal::mark_command_finished(result.exit_code);
                        terminal::bell_if_long(elapsed, bell_threshold);
                        prompt.set_exit_code(result.exit_code);
                        last_command = Some(line.to_string());
                        last_exit_code = Some(result.exit_code);
                        command_history.push(line.to_string());
                        if command_history.len() > MAX_HISTORY_FOR_CONTEXT {
                            command_history.remove(0);
                        }
                    }
                    Err(e) => {
                        let elapsed = cmd_start.elapsed();
                        eprintln!("Error: {}", e);
                        terminal::mark_command_finished(1);
                        terminal::bell_if_long(elapsed, bell_threshold);
                        prompt.set_exit_code(1);
                        last_exit_code = Some(1);
                    }
                }
            }
            Ok(Signal::CtrlC) => {
                // Reedline handles Ctrl-C in interactive mode
                // Reset signal handler state
                signal_handler.reset();
                continue;
            }
            Ok(Signal::CtrlD) => {
                break;
            }
            Err(e) => {
                // EINTR (interrupted system call) happens when signals arrive during read.
                // This is normal at shell startup - just retry.
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                // Use write(2) directly — eprintln! panics if stderr is revoked
                let msg = format!("Error reading line: {}\n", e);
                let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
                break;
            }
        }
    }

    Ok(())
}

fn run_non_interactive(signal_handler: SignalHandler, mut executor: Executor) -> Result<()> {
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());

    for line_result in reader.lines() {
        // Check for signals
        if signal_handler.should_shutdown() {
            eprintln!("\nInterrupted by signal");
            std::process::exit(signal_handler.exit_code());
        }

        // Check for SIGCHLD and reap any zombie processes
        if signal_handler.sigchld_received() {
            executor.runtime_mut().job_manager().reap_zombies();
            signal_handler.clear_sigchld();
        }

        // Handle EINTR - retry on interrupted system call
        let line = match line_result {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match execute_line(line, &mut executor) {
            Ok(result) => {
                let stdout_text = result.stdout();
                if !stdout_text.is_empty() {
                    print!("{}", stdout_text);
                }
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                // Continue on error in non-interactive mode
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        "Rush v{} - A Modern Shell in Rust",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("Usage:");
    println!("  rush                Start interactive shell");
    println!("  rush --login        Start as login shell (sources ~/.rush_profile)");
    println!("  rush --no-rc        Skip sourcing config files");
    println!("  rush --no-config    Skip sourcing config files (alias for --no-rc)");
    println!("  rush <script.rush>  Execute a Rush script file");
    println!("  rush -c <command>   Execute command and exit");
    println!("  rush --check <script.sh>             Check bash script compatibility");
    println!("  rush --profile -c <command>          Profile execution timing");
    println!("  rush --profile --json -c <command>   Profile as JSON for tooling");
    println!("  rush --benchmark <mode>              Run benchmarks (quick, full, compare)");
    println!("  rush --info                          Show system stats");
    println!("  rush --info <stat>                   Show single stat value (for scripting)");
    println!("  rush --info --json                   Show stats as JSON");
    println!("  rush -h, --help                      Show this help message");
    println!();
    println!("Examples:");
    println!("  rush script.rush");
    println!("  rush script.rush arg1 arg2");
    println!("  rush -c \"echo hello\"");
    println!("  rush -c \"ls -la\"");
    println!("  rush -c \"cat file.txt | grep pattern\"");
    println!("  rush --check my_script.sh            # Analyze bash script compatibility");
    println!("  rush --profile -c \"echo hello\"      # Profile with timing breakdown");
    println!("  rush --profile --json -c \"echo hello\" | jq  # Profile as JSON, parse with jq");
    println!("  rush --login                         # Start login shell");
    println!("  rush --benchmark quick               # Run quick benchmark");
    println!("  rush --benchmark full                # Run comprehensive benchmark");
    println!("  rush --info memory                   # Get single stat value");
    println!("  rush --info --json                   # Get all stats as JSON");
    println!();
    println!("Config Files:");
    println!("  ~/.rush_profile     Sourced on login shells");
    println!("  ~/.rushrc           Sourced on interactive shells");
}

fn execute_line(line: &str, executor: &mut Executor) -> Result<executor::ExecutionResult> {
    // Tokenize
    let tokens = Lexer::tokenize(line)?;

    // Parse
    let mut parser = Parser::new(tokens);
    let statements = parser.parse()?;

    // Execute — catch ExitSignal at top level so `exit` terminates the shell
    match executor.execute(statements) {
        Ok(result) => Ok(result),
        Err(e) => {
            if let Some(exit_signal) = e.downcast_ref::<builtins::exit_builtin::ExitSignal>() {
                std::process::exit(exit_signal.exit_code);
            }
            Err(e)
        }
    }
}

/// Fast path for `rush -c "command"` execution.
///
/// Skips all expensive initialization:
/// - NO daemon client probe (saves 2-4ms from UnixStream::connect)
/// - NO signal handler thread spawn (saves 0.5-1ms)
/// - NO process group setup via setpgid (saves 0.2-0.5ms)
/// - NO init_environment_variables (saves 0.3-0.5ms from whoami, current_exe)
///
/// This function never returns — it always calls std::process::exit.
fn fast_execute_c(
    cmd: &str,
    enable_profile: bool,
    profile_json: bool,
    max_output: Option<&str>,
) -> ! {
    // Reset SIGPIPE to default so piped commands work correctly.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    // Resolve max output bytes: --max-output flag takes priority, then RUSH_MAX_OUTPUT env var.
    let max_output_bytes: Option<usize> = max_output
        .and_then(|s| run_api::parse_max_output(s))
        .or_else(|| {
            env::var("RUSH_MAX_OUTPUT")
                .ok()
                .and_then(|v| run_api::parse_max_output(&v))
        });

    let tokens = match Lexer::tokenize(cmd) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("rush: {}", e);
            std::process::exit(2);
        }
    };

    let mut parser = Parser::new(tokens);
    let statements = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rush: {}", e);
            std::process::exit(2);
        }
    };

    let mut executor = Executor::new().with_profiling(enable_profile);

    // Minimal runtime init: just PATH and PWD so commands can be found
    if let Ok(path) = env::var("PATH") {
        executor
            .runtime_mut()
            .set_variable("PATH".to_string(), path);
    }
    if let Ok(pwd) = env::current_dir() {
        executor
            .runtime_mut()
            .set_variable("PWD".to_string(), pwd.to_string_lossy().to_string());
    }
    if let Ok(home) = env::var("HOME") {
        executor
            .runtime_mut()
            .set_variable("HOME".to_string(), home);
    }

    match executor.execute(statements) {
        Ok(result) => {
            let mut stdout_text = result.stdout();
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }

            // Apply output budget if set.
            if let Some(max_bytes) = max_output_bytes {
                if stdout_text.len() > max_bytes {
                    let bytes_written = stdout_text.len();
                    // Truncate at a UTF-8 char boundary.
                    let safe_end = {
                        let bytes = stdout_text.as_bytes();
                        let mut i = max_bytes;
                        while i > 0 && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
                            i -= 1;
                        }
                        i
                    };
                    stdout_text.truncate(safe_end);
                    stdout_text.push_str(&format!(
                        "\n[Output truncated: {} bytes, limit {} bytes]",
                        bytes_written, max_bytes
                    ));
                }
            }

            if !stdout_text.is_empty() {
                print!("{}", stdout_text);
            }

            // Print profiling output if enabled
            if enable_profile {
                if let Some(ref profile_data) = executor.profile_data {
                    if profile_json {
                        // Output as JSON for tooling integration
                        let json = executor::ProfileFormatter::format_json(profile_data);
                        if let Ok(json_str) = serde_json::to_string_pretty(&json) {
                            eprintln!("{}", json_str);
                        }
                    } else {
                        // Output as human-readable table
                        eprint!("{}", executor::ProfileFormatter::format(profile_data));
                    }
                }
            }

            std::process::exit(result.exit_code);
        }
        Err(e) => {
            if let Some(exit_signal) = e.downcast_ref::<builtins::exit_builtin::ExitSignal>() {
                std::process::exit(exit_signal.exit_code);
            }
            eprintln!("rush: {}", e);
            std::process::exit(1);
        }
    }
}

fn execute_line_with_context(
    line: &str,
    executor: &mut Executor,
    _script_path: &str,
    _line_num: usize,
) -> Result<executor::ExecutionResult> {
    execute_line(line, executor).map_err(|e| anyhow::anyhow!("{}", e))
}

/// Run compatibility check on a bash script
fn run_compatibility_check(script_path: &str, show_migrate: bool, apply_fix: bool) -> ! {
    use compat::{CompatibilityReport, MigrationEngine, ScriptAnalyzer};

    // Read the script file
    let script_content = match fs::read_to_string(script_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading script '{}': {}", script_path, e);
            std::process::exit(1);
        }
    };

    // Analyze the script
    let analyzer = ScriptAnalyzer::new(script_path.to_string());
    let analysis = analyzer.analyze(&script_content);

    // Generate and display report
    let report = CompatibilityReport::generate(script_path, &analysis);
    println!("{}", report.format_report());

    // Handle migration suggestions if requested
    if show_migrate && !report.migration_suggestions.is_empty() {
        if apply_fix {
            // Apply safe transformations and write to file
            let fixed_content =
                MigrationEngine::apply_fixes(&script_content, &report.migration_suggestions);
            let output_path = format!("{}.migrated", script_path);

            match fs::write(&output_path, fixed_content) {
                Ok(_) => {
                    println!("\nMigrated script written to: {}", output_path);
                    println!("Review the changes and replace the original if satisfied.");
                }
                Err(e) => {
                    eprintln!("Error writing migrated script: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    // Exit with appropriate code
    std::process::exit(report.exit_code());
}

/// Run --info command to show system stats
///
/// - `rush --info` - show all stats
/// - `rush --info <stat>` - show single stat value (for scripting)
/// - `rush --info --json` - machine-readable output
fn run_info_command(stat_name: Option<String>, json_output: bool) -> ! {
    use stats::StatsCollector;
    use std::collections::HashMap;

    // Type aliases for clarity
    type BuiltinStats = HashMap<String, String>;
    type CustomStats = HashMap<String, String>;
    type DaemonInfo = Option<serde_json::Value>;

    // Try to get stats from daemon first (instant)
    let (builtin_stats, custom_stats, daemon_info): (BuiltinStats, CustomStats, DaemonInfo) =
        if let Ok(mut client) = rush::daemon::DaemonClient::new() {
            if client.is_daemon_running() {
                // Try to fetch from daemon cache
                match fetch_stats_from_daemon(&mut client, stat_name.as_deref()) {
                    Ok((builtin, custom, daemon)) => (builtin, custom, Some(daemon)),
                    Err(_) => {
                        // Fallback to on-demand collection
                        (StatsCollector::collect_builtins(), HashMap::new(), None)
                    }
                }
            } else {
                // No daemon - collect on-demand, skip custom
                (StatsCollector::collect_builtins(), HashMap::new(), None)
            }
        } else {
            // Can't create client - collect on-demand
            (StatsCollector::collect_builtins(), HashMap::new(), None)
        };

    // Single stat mode
    if let Some(name) = stat_name {
        if let Some(value) = builtin_stats.get(&name) {
            if json_output {
                println!("{}", serde_json::json!({ &name: value }));
            } else {
                println!("{}", value);
            }
            std::process::exit(0);
        } else if let Some(value) = custom_stats.get(&name) {
            if json_output {
                println!("{}", serde_json::json!({ &name: value }));
            } else {
                println!("{}", value);
            }
            std::process::exit(0);
        } else {
            eprintln!("Unknown stat: {}", name);
            eprintln!(
                "Available built-in stats: {}",
                StatsCollector::builtin_names().join(", ")
            );
            std::process::exit(1);
        }
    }

    // Full output mode
    if json_output {
        // JSON output
        let mut output = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "builtin": builtin_stats,
        });

        if !custom_stats.is_empty() {
            output["custom"] = serde_json::json!(custom_stats);
        }

        if let Some(daemon) = daemon_info {
            output["daemon"] = daemon;
        } else {
            output["daemon"] = serde_json::json!({ "running": false });
        }

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        // Human-readable output with pretty 2-column table
        let cyan = "\x1b[36m";
        let bold = "\x1b[1m";
        let _dim = "\x1b[2m";
        let reset = "\x1b[0m";

        // Column width (content only, not including border and padding)
        const COL_W: usize = 28; // Width of each column
        const FULL_W: usize = 57; // Full width = COL_W * 2 + 1 (for middle border)

        // Helper to format a stat, returns None if empty/N/A
        let fmt_stat = |name: &str| -> Option<String> {
            builtin_stats.get(name).and_then(|v| {
                if v.is_empty() || v == "N/A" || v == "unknown" {
                    None
                } else {
                    Some(v.clone())
                }
            })
        };

        // Helper to make a cell with name and value, padded to exact width
        let make_cell = |name: &str, val: &str| -> String {
            let content = format!("{:<8} {}", name, val);
            // Truncate if too long, pad if too short
            if content.len() >= COL_W {
                content[..COL_W].to_string()
            } else {
                format!("{:<w$}", content, w = COL_W)
            }
        };

        // Helper for empty cell
        let empty_cell = || " ".repeat(COL_W);

        // Parse load into separate values
        let (load1, load5, load15) = if let Some(load) = builtin_stats.get("load") {
            let parts: Vec<&str> = load.split_whitespace().collect();
            (
                parts.get(0).map(|s| s.to_string()),
                parts.get(1).map(|s| s.to_string()),
                parts.get(2).map(|s| s.to_string()),
            )
        } else {
            (None, None, None)
        };

        // Header - manually pad to account for invisible ANSI codes
        let version = env!("CARGO_PKG_VERSION");
        let left_hdr = format!("{}rush{} v{}", bold, reset, version);
        let left_visible = format!("rush v{}", version);
        let left_pad = COL_W.saturating_sub(left_visible.len());

        let right_hdr = format!("{}Resources{}", bold, reset);
        let right_pad = COL_W.saturating_sub(9);

        println!("{}╭{:─<w$}┬{:─<w$}╮{}", cyan, "", "", reset, w = COL_W);
        println!(
            "{}│{}{}{:pad_l$}{}│{}{}{:pad_r$}{}│{}",
            cyan,
            reset,
            left_hdr,
            "",
            cyan,
            reset,
            right_hdr,
            "",
            cyan,
            reset,
            pad_l = left_pad,
            pad_r = right_pad
        );
        println!("{}├{:─<w$}┼{:─<w$}┤{}", cyan, "", "", reset, w = COL_W);

        // Build rows for left column (System) and right column (Resources)
        let left_stats: Vec<(&str, Option<String>)> = vec![
            ("host", fmt_stat("host")),
            ("os", fmt_stat("os")),
            ("kernel", fmt_stat("kernel")),
            ("arch", fmt_stat("arch")),
            ("cpu", fmt_stat("cpu")),
            ("cores", fmt_stat("cores")),
            ("uptime", fmt_stat("uptime")),
        ];

        let right_stats: Vec<(&str, Option<String>)> = vec![
            ("memory", fmt_stat("memory")),
            ("swap", fmt_stat("swap")),
            ("disk", fmt_stat("disk")),
            ("load 1m", load1),
            ("load 5m", load5),
            ("load 15m", load15),
            ("procs", fmt_stat("procs")),
        ];

        // Print paired rows
        let max_rows = left_stats.len().max(right_stats.len());
        for i in 0..max_rows {
            let left_cell = left_stats
                .get(i)
                .and_then(|(n, v)| v.as_ref().map(|val| make_cell(n, val)))
                .unwrap_or_else(empty_cell);
            let right_cell = right_stats
                .get(i)
                .and_then(|(n, v)| v.as_ref().map(|val| make_cell(n, val)))
                .unwrap_or_else(empty_cell);

            println!(
                "{}│{}{}{}│{}{}{}│{}",
                cyan, reset, left_cell, cyan, reset, right_cell, cyan, reset
            );
        }

        // Network & Time section
        let net_hdr = format!("{}Network{}", bold, reset);
        let net_pad = COL_W.saturating_sub(7);
        let time_hdr = format!("{}Time{}", bold, reset);
        let time_pad = COL_W.saturating_sub(4);

        println!("{}├{:─<w$}┼{:─<w$}┤{}", cyan, "", "", reset, w = COL_W);
        println!(
            "{}│{}{}{:pn$}{}│{}{}{:pt$}{}│{}",
            cyan,
            reset,
            net_hdr,
            "",
            cyan,
            reset,
            time_hdr,
            "",
            cyan,
            reset,
            pn = net_pad,
            pt = time_pad
        );
        println!("{}├{:─<w$}┼{:─<w$}┤{}", cyan, "", "", reset, w = COL_W);

        let net_stats: Vec<(&str, Option<String>)> = vec![
            ("ip", fmt_stat("ip")),
            ("wifi", fmt_stat("wifi")),
            ("power", fmt_stat("power")),
            ("battery", fmt_stat("battery")),
        ];

        let time_stats: Vec<(&str, Option<String>)> =
            vec![("time", fmt_stat("time")), ("date", fmt_stat("date"))];

        let net_filtered: Vec<_> = net_stats.iter().filter(|(_, v)| v.is_some()).collect();
        let time_filtered: Vec<_> = time_stats.iter().filter(|(_, v)| v.is_some()).collect();
        let max_rows2 = net_filtered.len().max(time_filtered.len());

        for i in 0..max_rows2 {
            let left_cell = net_filtered
                .get(i)
                .map(|(n, v)| make_cell(n, v.as_ref().unwrap()))
                .unwrap_or_else(empty_cell);
            let right_cell = time_filtered
                .get(i)
                .map(|(n, v)| make_cell(n, v.as_ref().unwrap()))
                .unwrap_or_else(empty_cell);

            println!(
                "{}│{}{}{}│{}{}{}│{}",
                cyan, reset, left_cell, cyan, reset, right_cell, cyan, reset
            );
        }

        // Custom stats (if any)
        if !custom_stats.is_empty() {
            let custom_hdr = format!("{}Custom{}", bold, reset);
            let custom_pad = FULL_W.saturating_sub(6);

            println!("{}├{:─<w$}┤{}", cyan, "", reset, w = FULL_W);
            println!(
                "{}│{}{}{:p$}{}│{}",
                cyan,
                reset,
                custom_hdr,
                "",
                cyan,
                reset,
                p = custom_pad
            );
            println!("{}├{:─<w$}┤{}", cyan, "", reset, w = FULL_W);
            for (name, value) in &custom_stats {
                let content = format!("{:<8} {}", name, value);
                let content = if content.len() >= FULL_W {
                    content[..FULL_W].to_string()
                } else {
                    format!("{:<w$}", content, w = FULL_W)
                };
                println!("{}│{}{}{}│{}", cyan, reset, content, cyan, reset);
            }
        }

        // Daemon status
        println!("{}├{:─<w$}┤{}", cyan, "", reset, w = FULL_W);
        let daemon_status = if let Some(daemon) = daemon_info {
            daemon
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            "not running".to_string()
        };
        let daemon_content = format!("{:<8} {}", "daemon", daemon_status);
        let daemon_content = if daemon_content.len() >= FULL_W {
            daemon_content[..FULL_W].to_string()
        } else {
            format!("{:<w$}", daemon_content, w = FULL_W)
        };
        println!("{}│{}{}{}│{}", cyan, reset, daemon_content, cyan, reset);

        println!("{}╰{:─<w$}╯{}", cyan, "", reset, w = FULL_W);
    }

    std::process::exit(0);
}

/// Fetch stats from daemon cache
fn fetch_stats_from_daemon(
    client: &mut rush::daemon::DaemonClient,
    _stat_name: Option<&str>,
) -> Result<(
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
    serde_json::Value,
)> {
    // Connect to daemon
    client.connect()?;

    // For now, return an error to trigger fallback to on-demand collection
    // TODO: Implement full daemon stats fetching when daemon StatsCache is ready (bean 5.3)
    Err(anyhow::anyhow!("Daemon stats not yet implemented"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_echo() {
        let mut executor = Executor::new();
        let result = execute_line("echo hello", &mut executor).unwrap();
        assert_eq!(result.stdout(), "hello\n");
    }

    #[test]
    fn test_execute_pwd() {
        let mut executor = Executor::new();
        let result = execute_line("pwd", &mut executor).unwrap();
        assert!(!result.stdout().is_empty());
    }

    #[test]
    fn test_script_arguments() {
        use std::fs;
        use std::io::Write;

        // Create a temporary script
        let script_path = "/tmp/rush_test_args.rush";
        let mut file = fs::File::create(script_path).unwrap();
        writeln!(file, "#!/usr/bin/env rush").unwrap();
        writeln!(file, "echo $1").unwrap();
        writeln!(file, "echo $2").unwrap();

        // Test would go here, but requires running the binary
        // This is more of an integration test

        // Cleanup
        fs::remove_file(script_path).ok();
    }

    #[test]
    fn test_execute_line_with_context() {
        let mut executor = Executor::new();
        let result = execute_line_with_context("echo test", &mut executor, "test.rush", 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().stdout(), "test\n");
    }
}
