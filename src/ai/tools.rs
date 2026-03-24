//! Tool helpers for the `?` agent: confirmation prompts, diff previews, and
//! shell command execution.

use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use nu_ansi_term::Color;
use std::io::{self, Write};

// ─── Confirmation ────────────────────────────────────────────────────────────

/// Ask the user "prompt [Y/n]" and return true if they press Y / Enter.
///
/// Uses raw mode so a single keypress is enough — no need to hit Enter.
/// Returns an error only if terminal setup fails.
pub fn confirm(prompt: &str) -> Result<bool> {
    // Skip confirmation if autorun is enabled via RUSH_AI_AUTORUN in .rushrc
    if matches!(
        std::env::var("RUSH_AI_AUTORUN").as_deref(),
        Ok("1" | "true" | "yes")
    ) {
        println!("  {} ✓", prompt);
        return Ok(true);
    }

    print!("  {} [Y/n] ", prompt);
    io::stdout().flush()?;

    if enable_raw_mode().is_err() {
        // Fallback: read a line
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        println!();
        let trimmed = line.trim().to_lowercase();
        return Ok(trimmed.is_empty() || trimmed == "y" || trimmed == "yes");
    }

    let result = loop {
        match event::read() {
            Ok(Event::Key(key)) => match (key.code, key.modifiers) {
                (KeyCode::Enter, _) | (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                    break Ok(true);
                }
                (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                    break Ok(false);
                }
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    break Ok(false);
                }
                _ => {}
            },
            Ok(_) => {}
            Err(e) => break Err(anyhow!("Input error: {}", e)),
        }
    };

    let _ = disable_raw_mode();
    println!();
    result
}

// ─── Edit preview ────────────────────────────────────────────────────────────

/// Print a simple diff-style view of an edit: red lines for removed text,
/// green lines for replacement text.  Each half is limited to 10 lines to
/// avoid flooding the terminal.
pub fn show_edit_preview(old_text: &str, new_text: &str) {
    const MAX_LINES: usize = 10;

    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let shown_old = old_lines.len().min(MAX_LINES);
    let shown_new = new_lines.len().min(MAX_LINES);

    for line in &old_lines[..shown_old] {
        println!("  {}", Color::Red.paint(format!("- {}", line)));
    }
    if old_lines.len() > MAX_LINES {
        println!(
            "  {}",
            Color::DarkGray.paint(format!("  … ({} more lines)", old_lines.len() - MAX_LINES))
        );
    }

    for line in &new_lines[..shown_new] {
        println!("  {}", Color::Green.paint(format!("+ {}", line)));
    }
    if new_lines.len() > MAX_LINES {
        println!(
            "  {}",
            Color::DarkGray.paint(format!("  … ({} more lines)", new_lines.len() - MAX_LINES))
        );
    }
}
