//! Configuration for the LLM client.
//!
//! All settings are read from environment variables, which are typically set
//! in `~/.rushrc`:
//!
//! ```bash
//! RUSH_AI_PROVIDER=ollama
//! RUSH_AI_MODEL=qwen2.5-coder:7b
//! RUSH_AI_AUTORUN=true
//! ```

use std::fmt;
use std::str::FromStr;

/// Which LLM provider to use.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Configuration for the LLM client, read entirely from env vars.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: ProviderType,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
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
    /// Load config from environment variables.
    ///
    /// Returns `Ok` with defaults for any unset variable.
    /// Returns `Err` only if a variable has an invalid value.
    pub fn load() -> Result<Self, String> {
        let provider = match std::env::var("RUSH_AI_PROVIDER") {
            Ok(val) => val.parse::<ProviderType>()?,
            Err(_) => ProviderType::Ollama,
        };

        let model =
            std::env::var("RUSH_AI_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());

        let api_key = std::env::var("RUSH_AI_API_KEY")
            .ok()
            .or_else(|| match provider {
                ProviderType::OpenAi => std::env::var("OPENAI_API_KEY").ok(),
                ProviderType::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
                ProviderType::Ollama => None,
            });

        let base_url = std::env::var("RUSH_AI_BASE_URL").ok();

        let autorun = matches!(
            std::env::var("RUSH_AI_AUTORUN").as_deref(),
            Ok("1" | "true" | "yes")
        );

        Ok(Self {
            provider,
            model,
            api_key,
            base_url,
            autorun,
        })
    }

    /// Check if AI has been configured (provider env var is set).
    pub fn is_configured() -> bool {
        std::env::var("RUSH_AI_PROVIDER").is_ok()
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
        assert!(!cfg.autorun);
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
}
