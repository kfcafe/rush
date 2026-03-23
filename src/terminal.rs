use anyhow::{anyhow, Result};
use nix::libc;
use nix::unistd::{getpgrp, Pid};
use std::io::Write;
use std::os::unix::io::RawFd;
use std::time::Duration;

/// Terminal control for managing foreground process groups
#[derive(Clone)]
pub struct TerminalControl {
    shell_pgid: Pid,
    terminal_fd: RawFd,
    is_interactive: bool,
}

impl TerminalControl {
    /// Create a new terminal control instance
    pub fn new() -> Self {
        let terminal_fd = 0; // stdin
        let shell_pgid = getpgrp();

        // Check if we're interactive by verifying:
        // 1. stdin is a terminal
        // 2. We're in the foreground process group
        let is_interactive = unsafe { libc::isatty(terminal_fd) } == 1
            && Self::tcgetpgrp_raw(terminal_fd)
                .map(|fg_pgid| fg_pgid == shell_pgid.as_raw())
                .unwrap_or(false);

        Self {
            shell_pgid,
            terminal_fd,
            is_interactive,
        }
    }

    /// Get the foreground process group using libc directly
    fn tcgetpgrp_raw(fd: RawFd) -> Result<i32> {
        let pgid = unsafe { libc::tcgetpgrp(fd) };
        if pgid < 0 {
            Err(anyhow!("tcgetpgrp failed"))
        } else {
            Ok(pgid)
        }
    }

    /// Set the foreground process group using libc directly
    fn tcsetpgrp_raw(fd: RawFd, pgid: i32) -> Result<()> {
        let result = unsafe { libc::tcsetpgrp(fd, pgid) };
        if result != 0 {
            Err(anyhow!("tcsetpgrp failed"))
        } else {
            Ok(())
        }
    }

    /// Check if the shell is running interactively with terminal control
    pub fn is_interactive(&self) -> bool {
        self.is_interactive
    }

    /// Give terminal control to the specified process group
    pub fn give_terminal_to(&self, pgid: Pid) -> Result<()> {
        if !self.is_interactive {
            return Ok(()); // Not interactive, nothing to do
        }

        Self::tcsetpgrp_raw(self.terminal_fd, pgid.as_raw()).map_err(|e| {
            anyhow!(
                "Failed to give terminal control to process group {}: {}",
                pgid,
                e
            )
        })
    }

    /// Reclaim terminal control for the shell
    pub fn reclaim_terminal(&self) -> Result<()> {
        if !self.is_interactive {
            return Ok(()); // Not interactive, nothing to do
        }

        Self::tcsetpgrp_raw(self.terminal_fd, self.shell_pgid.as_raw())
            .map_err(|e| anyhow!("Failed to reclaim terminal control: {}", e))
    }

    /// Get the current foreground process group
    pub fn get_foreground_pgid(&self) -> Result<Pid> {
        Self::tcgetpgrp_raw(self.terminal_fd)
            .map(Pid::from_raw)
            .map_err(|e| anyhow!("Failed to get foreground process group: {}", e))
    }
}

impl Default for TerminalControl {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Terminal escape sequences for interactive QoL
// ---------------------------------------------------------------------------

// All sequences write to stderr so they don't pollute piped stdout.

/// OSC 7 — report current working directory to the terminal emulator.
/// Uses `kitty-shell-cwd://` which Ghostty and Kitty recognize for
/// new-tab-in-same-directory. Falls back to `file://` for other terminals.
pub fn emit_osc7_cwd() {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return,
    };
    let hostname = get_hostname();
    let mut err = std::io::stderr().lock();
    // Detect terminal and emit the appropriate protocol.
    // Ghostty/Kitty use kitty-shell-cwd://, others use file://.
    if std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some() {
        let _ = write!(
            err,
            "\x1b]7;kitty-shell-cwd://{}{}\x07",
            hostname,
            cwd.display()
        );
    } else {
        let encoded = percent_encode_path(cwd.as_os_str().as_encoded_bytes());
        let _ = write!(err, "\x1b]7;file://{}{}\x07", hostname, encoded);
    }
    let _ = err.flush();
}

/// OSC 2 — set the terminal window/tab title.
pub fn set_terminal_title(title: &str) {
    let truncated = truncate_title(title, 64);
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\x1b]2;{}\x07", truncated);
    let _ = err.flush();
}

/// Set the terminal title to `rush: <cwd>` with home-directory shortening.
pub fn set_terminal_title_to_cwd() {
    let title = match std::env::current_dir() {
        Ok(cwd) => {
            let shortened = shorten_home(&cwd);
            format!("rush: {}", shortened)
        }
        Err(_) => "rush".to_string(),
    };
    set_terminal_title(&title);
}

/// OSC 133 markers — semantic prompt zones for modern terminals.
/// Enables Cmd+Up/Down jumping and click-to-select output in Ghostty.
pub fn mark_prompt_start() {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\x1b]133;A\x07");
    let _ = err.flush();
}

pub fn mark_command_start() {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\x1b]133;B\x07");
    let _ = err.flush();
}

pub fn mark_output_start() {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\x1b]133;C\x07");
    let _ = err.flush();
}

pub fn mark_command_finished(exit_code: i32) {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\x1b]133;D;{}\x07", exit_code);
    let _ = err.flush();
}

/// Ring the terminal bell if a command ran longer than the threshold.
pub fn bell_if_long(elapsed: Duration, threshold: Duration) {
    if !threshold.is_zero() && elapsed >= threshold {
        let _ = write!(std::io::stderr(), "\x07");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_hostname() -> String {
    let mut buf = [0i8; 256];
    unsafe {
        if libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 {
            if let Ok(s) = std::ffi::CStr::from_ptr(buf.as_ptr()).to_str() {
                return s.to_string();
            }
        }
    }
    "localhost".to_string()
}

/// Percent-encode a path for use in a file:// URI (RFC 3986).
fn percent_encode_path(bytes: &[u8]) -> String {
    const HEX: [u8; 16] = *b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 0x0f) as usize]));
            }
        }
    }
    out
}

/// Shorten a path by replacing the home directory prefix with `~`.
pub fn shorten_home(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(suffix) = path.strip_prefix(&home) {
            return if suffix.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", suffix.display())
            };
        }
    }
    path.display().to_string()
}

fn truncate_title(title: &str, max: usize) -> &str {
    if title.len() <= max {
        title
    } else {
        // Truncate at a char boundary
        let mut end = max;
        while end > 0 && !title.is_char_boundary(end) {
            end -= 1;
        }
        &title[..end]
    }
}

/// Read the current git branch from `.git/HEAD` by walking up from `start`.
/// Returns `None` if not in a git repo. Fast — just one file read, no subprocess.
pub fn git_branch_fast(start: &std::path::Path) -> Option<String> {
    let mut dir = start.to_path_buf();
    loop {
        let head = dir.join(".git/HEAD");
        if head.is_file() {
            let contents = std::fs::read_to_string(&head).ok()?;
            let contents = contents.trim();
            return if let Some(refname) = contents.strip_prefix("ref: refs/heads/") {
                Some(refname.to_string())
            } else if contents.len() >= 7 {
                // Detached HEAD — show short hash
                Some(contents[..7].to_string())
            } else {
                None
            };
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_control_creation() {
        let _terminal = TerminalControl::new();
    }

    #[test]
    fn test_terminal_control_clone() {
        let terminal = TerminalControl::new();
        let terminal2 = terminal.clone();
        assert_eq!(terminal.is_interactive(), terminal2.is_interactive());
    }

    #[test]
    fn test_reclaim_terminal() {
        let _terminal = TerminalControl::new();
    }

    // -- percent encoding --

    #[test]
    fn test_percent_encode_simple_path() {
        assert_eq!(percent_encode_path(b"/usr/local/bin"), "/usr/local/bin");
    }

    #[test]
    fn test_percent_encode_spaces() {
        assert_eq!(
            percent_encode_path(b"/Users/me/My Documents"),
            "/Users/me/My%20Documents"
        );
    }

    #[test]
    fn test_percent_encode_special_chars() {
        assert_eq!(
            percent_encode_path(b"/tmp/a (copy).txt"),
            "/tmp/a%20%28copy%29.txt"
        );
    }

    #[test]
    fn test_percent_encode_preserves_unreserved() {
        assert_eq!(
            percent_encode_path(b"/with-dashes_and.dots~tilde"),
            "/with-dashes_and.dots~tilde"
        );
    }

    // -- title truncation --

    #[test]
    fn test_truncate_title_short() {
        assert_eq!(truncate_title("hello", 64), "hello");
    }

    #[test]
    fn test_truncate_title_exact() {
        let s = "a".repeat(64);
        assert_eq!(truncate_title(&s, 64), s.as_str());
    }

    #[test]
    fn test_truncate_title_long() {
        let s = "a".repeat(100);
        assert_eq!(truncate_title(&s, 64).len(), 64);
    }

    // -- hostname --

    #[test]
    fn test_get_hostname_nonempty() {
        assert!(!get_hostname().is_empty());
    }

    // -- git branch --

    #[test]
    fn test_git_branch_in_repo() {
        // We're running in the rush repo, so this should find a branch
        if let Ok(cwd) = std::env::current_dir() {
            let branch = git_branch_fast(&cwd);
            // May be None in CI with detached HEAD, but should be Some locally
            if let Some(b) = branch {
                assert!(!b.is_empty());
            }
        }
    }

    #[test]
    fn test_git_branch_outside_repo() {
        assert!(git_branch_fast(std::path::Path::new("/")).is_none());
    }

    // -- OSC 133 format --

    #[test]
    fn test_osc133_format() {
        // Just verify the functions don't panic; actual output goes to stderr
        mark_prompt_start();
        mark_command_start();
        mark_output_start();
        mark_command_finished(0);
        mark_command_finished(1);
    }

    // -- bell --

    #[test]
    fn test_bell_if_long_below_threshold() {
        use std::time::Duration;
        // Should not panic or bell
        bell_if_long(Duration::from_secs(1), Duration::from_secs(10));
    }

    #[test]
    fn test_bell_if_long_zero_threshold_disabled() {
        use std::time::Duration;
        bell_if_long(Duration::from_secs(100), Duration::ZERO);
    }
}
