//! Configuration for the LLM client
//!
//! Loaded from `~/.rush/ai.toml`. If the file does not exist, the defaults
//! point to a local Ollama instance so no setup is required for local use.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Which LLM provider to use
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Ollama,
    OpenAi,
    Anthropic,
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::Ollama => write!(f, "ollama"),
            ProviderType::OpenAi => write!(f, "openai"),
            ProviderType::Anthropic => write!(f, "anthropic"),
        }
    }
}

impl FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ollama" => Ok(ProviderType::Ollama),
            "openai" => Ok(ProviderType::OpenAi),
            "anthropic" => Ok(ProviderType::Anthropic),
            other => Err(format!(
                "unknown provider '{}'; valid options: ollama, openai, anthropic",
                other
            )),
        }
    }
}

/// Configuration for the LLM client
///
/// Stored in `~/.rush/ai.toml`:
/// ```toml
/// provider = "ollama"
/// model = "qwen2.5-coder:7b"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Which provider to use
    pub provider: ProviderType,

    /// Model name (provider-specific, e.g. "qwen2.5-coder:7b" for Ollama)
    pub model: String,

    /// API key — required for OpenAI and Anthropic, unused for Ollama.
    ///
    /// If `None`, the client falls back to the provider's standard env var
    /// (`OPENAI_API_KEY` or `ANTHROPIC_API_KEY`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Override the provider's default base URL (e.g. a custom Ollama port).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Auto-confirm all agent tool calls (no Y/n prompts).
    ///
    /// ```toml
    /// autorun = true
    /// ```
    #[serde(default)]
    pub autorun: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::Ollama,
            model: "qwen2.5-coder:7b".to_string(),
            api_key: None,
            base_url: None,
            autorun: false,
        }
    }
}

impl LlmConfig {
    /// Load config from `~/.rush/ai.toml`.
    ///
    /// Returns `Default` if the file does not exist.
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load() -> Result<Self, String> {
        let path =
            Self::config_path().ok_or_else(|| "Could not determine home directory".to_string())?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }

    /// Save config to `~/.rush/ai.toml`.
    pub fn save(&self) -> Result<(), String> {
        let path =
            Self::config_path().ok_or_else(|| "Could not determine home directory".to_string())?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
    }

    /// Path to the config file: `~/.rush/ai.toml`
    pub fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".rush").join("ai.toml"))
    }

    /// Resolve the API key: config field first, then env var fallback.
    pub fn resolved_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            return Some(key.clone());
        }
        match self.provider {
            ProviderType::OpenAi => std::env::var("OPENAI_API_KEY").ok(),
            ProviderType::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
            ProviderType::Ollama => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = LlmConfig::default();
        assert_eq!(cfg.provider, ProviderType::Ollama);
        assert_eq!(cfg.model, "qwen2.5-coder:7b");
        assert!(cfg.api_key.is_none());
        assert!(cfg.base_url.is_none());
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(ProviderType::Ollama.to_string(), "ollama");
        assert_eq!(ProviderType::OpenAi.to_string(), "openai");
        assert_eq!(ProviderType::Anthropic.to_string(), "anthropic");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(
            "ollama".parse::<ProviderType>().unwrap(),
            ProviderType::Ollama
        );
        assert_eq!(
            "openai".parse::<ProviderType>().unwrap(),
            ProviderType::OpenAi
        );
        assert_eq!(
            "anthropic".parse::<ProviderType>().unwrap(),
            ProviderType::Anthropic
        );
        assert!("unknown".parse::<ProviderType>().is_err());
    }

    #[test]
    fn test_toml_roundtrip() {
        let cfg = LlmConfig {
            provider: ProviderType::OpenAi,
            model: "gpt-4o".to_string(),
            api_key: Some("sk-test".to_string()),
            base_url: None,
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: LlmConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.provider, ProviderType::OpenAi);
        assert_eq!(parsed.model, "gpt-4o");
        assert_eq!(parsed.api_key, Some("sk-test".to_string()));
    }
}
