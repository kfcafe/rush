//! Rush banner system with configurable stats display
//!
//! Displays an ASCII art banner at shell startup with optional system stats.
//! Stats are fetched from the daemon cache for near-zero latency.

use std::collections::HashMap;
use std::env;

/// Banner display style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerStyle {
    /// Block ASCII art
    Block,
    /// Single line
    Line,
    /// Minimal (version only)
    Minimal,
    /// No banner
    #[default]
    None,
}

impl BannerStyle {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "block" => BannerStyle::Block,
            "line" => BannerStyle::Line,
            "minimal" => BannerStyle::Minimal,
            "none" => BannerStyle::None,
            _ => BannerStyle::Block,
        }
    }
}

/// Banner color
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerColor {
    #[default]
    Cyan,
    Green,
    Yellow,
    Magenta,
    Blue,
    White,
    None,
}

impl BannerColor {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cyan" => BannerColor::Cyan,
            "green" => BannerColor::Green,
            "yellow" => BannerColor::Yellow,
            "magenta" => BannerColor::Magenta,
            "blue" => BannerColor::Blue,
            "white" => BannerColor::White,
            "none" => BannerColor::None,
            _ => BannerColor::Cyan,
        }
    }

    /// Get ANSI color code
    pub fn ansi_code(&self) -> &'static str {
        match self {
            BannerColor::Cyan => "\x1b[36m",
            BannerColor::Green => "\x1b[32m",
            BannerColor::Yellow => "\x1b[33m",
            BannerColor::Magenta => "\x1b[35m",
            BannerColor::Blue => "\x1b[34m",
            BannerColor::White => "\x1b[37m",
            BannerColor::None => "",
        }
    }

    /// Get ANSI reset code
    pub fn reset_code(&self) -> &'static str {
        if *self == BannerColor::None {
            ""
        } else {
            "\x1b[0m"
        }
    }
}

/// When to show the banner
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerShow {
    /// Show banner every time
    #[default]
    Always,
    /// Show banner only on first shell (no parent rush process)
    First,
    /// Never show banner
    Never,
}

impl BannerShow {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "always" => BannerShow::Always,
            "first" => BannerShow::First,
            "never" => BannerShow::Never,
            _ => BannerShow::Always,
        }
    }
}

/// Banner configuration loaded from .rushrc
#[derive(Debug, Clone)]
pub struct BannerConfig {
    /// Display style
    pub style: BannerStyle,
    /// Color for ASCII art
    pub color: BannerColor,
    /// When to show the banner
    pub show: BannerShow,
    /// Stats to display (empty = no stats)
    pub stats: Vec<String>,
}

impl Default for BannerConfig {
    fn default() -> Self {
        Self {
            style: BannerStyle::Block,
            color: BannerColor::Cyan,
            show: BannerShow::Always,
            stats: Vec::new(),
        }
    }
}

impl BannerConfig {
    /// Load banner config from environment variables (set by sourcing .rushrc)
    pub fn from_env() -> Self {
        let style = env::var("RUSH_BANNER_STYLE")
            .map(|s| BannerStyle::from_str(&s))
            .unwrap_or_default();

        let color = env::var("RUSH_BANNER_COLOR")
            .map(|s| BannerColor::from_str(&s))
            .unwrap_or_default();

        let show = env::var("RUSH_BANNER_SHOW")
            .map(|s| BannerShow::from_str(&s))
            .unwrap_or_default();

        let stats = env::var("RUSH_BANNER_STATS")
            .map(|s| s.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Self {
            style,
            color,
            show,
            stats,
        }
    }

    /// Check if banner should be shown based on config
    pub fn should_show(&self) -> bool {
        match self.show {
            BannerShow::Always => true,
            BannerShow::Never => false,
            BannerShow::First => {
                // Check if parent process is rush (nested shell)
                // Look for RUSH_LEVEL environment variable
                env::var("RUSH_LEVEL")
                    .map(|v| v.parse::<i32>().unwrap_or(0) == 0)
                    .unwrap_or(true)
            }
        }
    }
}

/// Stats response from daemon
#[derive(Debug, Clone, Default)]
pub struct StatsData {
    /// Built-in stats (host, os, uptime, memory, etc.)
    pub builtin: HashMap<String, String>,
    /// Custom stats defined by user
    pub custom: HashMap<String, String>,
}

/// Display the startup banner with optional stats
pub fn display_banner(config: &BannerConfig, stats: Option<&StatsData>) {
    if !config.should_show() {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");
    let color = config.color.ansi_code();
    let reset = config.color.reset_code();

    match config.style {
        BannerStyle::Block => {
            eprintln!("{} █▀▄ █ █ █▀▀ █ █{}", color, reset);
            eprintln!("{} █   █ █ ▀▀█ █▀█{}  v{}", color, reset, version);
            eprintln!("{} ▀   ▀▀▀ ▀▀▀ ▀ ▀{}", color, reset);
        }
        BannerStyle::Line => {
            eprintln!("{}rush{} v{}", color, reset, version);
        }
        BannerStyle::Minimal => {
            eprintln!("v{}", version);
        }
        BannerStyle::None => {
            return; // No banner at all
        }
    }

    // Display stats if configured and available
    if !config.stats.is_empty() {
        if let Some(stats) = stats {
            display_stats(config, stats);
        }
    }

    // Add blank line after banner
    eprintln!();
}

/// Display configured stats below the banner
fn display_stats(config: &BannerConfig, stats: &StatsData) {
    if config.stats.is_empty() {
        return;
    }

    // Print separator line
    eprintln!(" ─────────────────────────");

    // Collect stat values, pairing them for two-column display
    let mut stat_values: Vec<(&str, String)> = Vec::new();

    for stat_name in &config.stats {
        let value = stats
            .builtin
            .get(stat_name)
            .or_else(|| stats.custom.get(stat_name))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "--".to_string());

        stat_values.push((stat_name, value));
    }

    // Display stats in two columns
    let mut i = 0;
    while i < stat_values.len() {
        let (name1, val1) = &stat_values[i];

        if i + 1 < stat_values.len() {
            let (name2, val2) = &stat_values[i + 1];
            // Two column format with padding
            eprintln!("  {:<12} {:<10}  {:<12} {}", name1, val1, name2, val2);
            i += 2;
        } else {
            // Single stat on last line
            eprintln!("  {:<12} {}", name1, val1);
            i += 1;
        }
    }
}

/// Increment the RUSH_LEVEL environment variable for nested shell detection
pub fn increment_rush_level() {
    let level = env::var("RUSH_LEVEL")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    env::set_var("RUSH_LEVEL", (level + 1).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_style_from_str() {
        assert_eq!(BannerStyle::from_str("block"), BannerStyle::Block);
        assert_eq!(BannerStyle::from_str("BLOCK"), BannerStyle::Block);
        assert_eq!(BannerStyle::from_str("line"), BannerStyle::Line);
        assert_eq!(BannerStyle::from_str("minimal"), BannerStyle::Minimal);
        assert_eq!(BannerStyle::from_str("none"), BannerStyle::None);
        assert_eq!(BannerStyle::from_str("invalid"), BannerStyle::Block);
    }

    #[test]
    fn test_banner_color_from_str() {
        assert_eq!(BannerColor::from_str("cyan"), BannerColor::Cyan);
        assert_eq!(BannerColor::from_str("GREEN"), BannerColor::Green);
        assert_eq!(BannerColor::from_str("yellow"), BannerColor::Yellow);
        assert_eq!(BannerColor::from_str("none"), BannerColor::None);
    }

    #[test]
    fn test_banner_show_from_str() {
        assert_eq!(BannerShow::from_str("always"), BannerShow::Always);
        assert_eq!(BannerShow::from_str("FIRST"), BannerShow::First);
        assert_eq!(BannerShow::from_str("never"), BannerShow::Never);
    }

    #[test]
    fn test_banner_config_default() {
        let config = BannerConfig::default();
        assert_eq!(config.style, BannerStyle::Block);
        assert_eq!(config.color, BannerColor::Cyan);
        assert_eq!(config.show, BannerShow::Always);
        assert!(config.stats.is_empty());
    }

    #[test]
    fn test_color_ansi_codes() {
        assert_eq!(BannerColor::Cyan.ansi_code(), "\x1b[36m");
        assert_eq!(BannerColor::None.ansi_code(), "");
        assert_eq!(BannerColor::Cyan.reset_code(), "\x1b[0m");
        assert_eq!(BannerColor::None.reset_code(), "");
    }
}
