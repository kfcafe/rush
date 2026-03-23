//! Daemon configuration parsing from .rushrc
//!
//! Parses banner settings and custom stat definitions:
//! - RUSH_BANNER_STYLE (block, line, minimal, none)
//! - RUSH_BANNER_COLOR (cyan, green, etc.)
//! - RUSH_BANNER_SHOW (always, first, never)
//! - RUSH_BANNER_STATS (space-separated stat names)
//! - RUSH_STAT_<name>="command"
//! - RUSH_STAT_<name>_INTERVAL=seconds
//! - RUSH_STAT_<name>_TIMEOUT=seconds

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Banner display style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerStyle {
    #[default]
    Block,
    Line,
    Minimal,
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

/// Banner display condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerShow {
    #[default]
    Always,
    First,
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

/// Banner configuration from .rushrc
#[derive(Debug, Clone, Default)]
pub struct BannerConfig {
    /// Display style (block, line, minimal, none)
    pub style: BannerStyle,
    /// Color (cyan, green, etc.)
    pub color: String,
    /// When to show (always, first, never)
    pub show: BannerShow,
    /// Stats to display in banner (space-separated names)
    pub stats: Vec<String>,
}

/// Custom stat definition from .rushrc
#[derive(Debug, Clone)]
pub struct CustomStatConfig {
    /// Stat name (from RUSH_STAT_<name>)
    pub name: String,
    /// Shell command to execute
    pub command: String,
    /// Refresh interval (default 30s)
    pub interval: Duration,
    /// Command timeout (default 2s)
    pub timeout: Duration,
}

impl Default for CustomStatConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(2),
        }
    }
}

/// Complete daemon configuration
#[derive(Debug, Clone, Default)]
pub struct DaemonConfig {
    pub banner: BannerConfig,
    pub custom_stats: Vec<CustomStatConfig>,
}

impl DaemonConfig {
    /// Parse configuration from .rushrc file
    pub fn from_rushrc() -> Self {
        let rushrc_path = Self::rushrc_path();
        Self::from_file(&rushrc_path).unwrap_or_default()
    }

    /// Get the path to .rushrc
    pub fn rushrc_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".rushrc")
    }

    /// Parse configuration from a specific file
    pub fn from_file(path: &PathBuf) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        Some(Self::parse(&content))
    }

    /// Parse configuration from content string
    pub fn parse(content: &str) -> Self {
        let mut config = DaemonConfig::default();
        let mut stat_commands: HashMap<String, String> = HashMap::new();
        let mut stat_intervals: HashMap<String, u64> = HashMap::new();
        let mut stat_timeouts: HashMap<String, u64> = HashMap::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse variable assignments (handle both = and export)
            let line = line.strip_prefix("export ").unwrap_or(line);

            if let Some((key, value)) = parse_assignment(line) {
                let value = unquote(&value);

                match key.as_str() {
                    "RUSH_BANNER_STYLE" => {
                        config.banner.style = BannerStyle::from_str(&value);
                    }
                    "RUSH_BANNER_COLOR" => {
                        config.banner.color = value;
                    }
                    "RUSH_BANNER_SHOW" => {
                        config.banner.show = BannerShow::from_str(&value);
                    }
                    "RUSH_BANNER_STATS" => {
                        config.banner.stats =
                            value.split_whitespace().map(|s| s.to_string()).collect();
                    }
                    _ if key.starts_with("RUSH_STAT_") => {
                        let suffix = &key["RUSH_STAT_".len()..];

                        if let Some(name) = suffix.strip_suffix("_INTERVAL") {
                            // RUSH_STAT_<name>_INTERVAL
                            if let Ok(secs) = value.parse::<u64>() {
                                stat_intervals.insert(name.to_lowercase(), secs);
                            }
                        } else if let Some(name) = suffix.strip_suffix("_TIMEOUT") {
                            // RUSH_STAT_<name>_TIMEOUT
                            if let Ok(secs) = value.parse::<u64>() {
                                stat_timeouts.insert(name.to_lowercase(), secs);
                            }
                        } else {
                            // RUSH_STAT_<name>="command"
                            let name = suffix.to_lowercase();
                            stat_commands.insert(name, value);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Build custom stats from parsed commands
        for (name, command) in stat_commands {
            let interval = stat_intervals
                .get(&name)
                .map(|&s| Duration::from_secs(s))
                .unwrap_or(Duration::from_secs(30));

            let timeout = stat_timeouts
                .get(&name)
                .map(|&s| Duration::from_secs(s))
                .unwrap_or(Duration::from_secs(2));

            config.custom_stats.push(CustomStatConfig {
                name: name.clone(),
                command,
                interval,
                timeout,
            });
        }

        // Sort custom stats by name for deterministic ordering
        config.custom_stats.sort_by(|a, b| a.name.cmp(&b.name));

        config
    }

    /// Get custom stat config by name
    pub fn get_custom_stat(&self, name: &str) -> Option<&CustomStatConfig> {
        self.custom_stats.iter().find(|s| s.name == name)
    }

    /// Check if a stat is configured for banner display
    pub fn is_banner_stat(&self, name: &str) -> bool {
        self.banner.stats.iter().any(|s| s == name)
    }
}

/// Parse a shell variable assignment (KEY=value or KEY="value")
fn parse_assignment(line: &str) -> Option<(String, String)> {
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim().to_string();
    let value = line[eq_pos + 1..].trim().to_string();

    // Validate key is a valid identifier
    if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    Some((key, value))
}

/// Remove surrounding quotes from a value
fn unquote(s: &str) -> String {
    let s = s.trim();

    // Handle double quotes
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return s[1..s.len() - 1].to_string();
    }

    // Handle single quotes
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        return s[1..s.len() - 1].to_string();
    }

    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let config = DaemonConfig::parse("");
        assert_eq!(config.banner.style, BannerStyle::Block);
        assert!(config.custom_stats.is_empty());
    }

    #[test]
    fn test_parse_banner_config() {
        let content = r#"
RUSH_BANNER_STYLE="minimal"
RUSH_BANNER_COLOR="green"
RUSH_BANNER_SHOW="first"
RUSH_BANNER_STATS="host uptime memory"
"#;
        let config = DaemonConfig::parse(content);

        assert_eq!(config.banner.style, BannerStyle::Minimal);
        assert_eq!(config.banner.color, "green");
        assert_eq!(config.banner.show, BannerShow::First);
        assert_eq!(config.banner.stats, vec!["host", "uptime", "memory"]);
    }

    #[test]
    fn test_parse_custom_stats() {
        let content = r#"
RUSH_STAT_weather="curl -s wttr.in"
RUSH_STAT_weather_INTERVAL=300
RUSH_STAT_weather_TIMEOUT=5
RUSH_STAT_todos="wc -l < ~/todo.txt"
"#;
        let config = DaemonConfig::parse(content);

        assert_eq!(config.custom_stats.len(), 2);

        let weather = config.get_custom_stat("weather").unwrap();
        assert_eq!(weather.command, "curl -s wttr.in");
        assert_eq!(weather.interval, Duration::from_secs(300));
        assert_eq!(weather.timeout, Duration::from_secs(5));

        let todos = config.get_custom_stat("todos").unwrap();
        assert_eq!(todos.command, "wc -l < ~/todo.txt");
        assert_eq!(todos.interval, Duration::from_secs(30)); // default
        assert_eq!(todos.timeout, Duration::from_secs(2)); // default
    }

    #[test]
    fn test_parse_with_export() {
        let content = r#"
export RUSH_BANNER_STYLE="line"
export RUSH_STAT_test="echo hello"
"#;
        let config = DaemonConfig::parse(content);

        assert_eq!(config.banner.style, BannerStyle::Line);
        assert_eq!(config.custom_stats.len(), 1);
    }

    #[test]
    fn test_parse_comments() {
        let content = r#"
# This is a comment
RUSH_BANNER_STYLE="block"
# Another comment
RUSH_STAT_test="echo hello"
"#;
        let config = DaemonConfig::parse(content);

        assert_eq!(config.banner.style, BannerStyle::Block);
        assert_eq!(config.custom_stats.len(), 1);
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("'hello'"), "hello");
        assert_eq!(unquote("hello"), "hello");
        assert_eq!(unquote("  \"hello\"  "), "hello");
    }
}
