//! Provider-agnostic LLM client
//!
//! The `LlmClient` wraps any `LlmProvider` implementation and exposes a
//! uniform `chat` interface. The shell's agent logic only talks to this type —
//! swapping providers is a configuration change, not a code change.

use crate::ai::config::{LlmConfig, ProviderType};
use crate::ai::providers::{
    anthropic::AnthropicProvider, ollama::OllamaProvider, openai::OpenAiProvider,
};
use serde_json::Value;

/// Role of a message in a conversation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    /// Tool result being returned to the model
    Tool,
}

/// A single message in the conversation history
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Tool call ID this message is a result for (only used when role == Tool)
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// A tool (function) the model may call
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the tool's parameters
    pub parameters: Value,
}

impl Tool {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// A response from the model
#[derive(Debug, Clone)]
pub enum Response {
    /// The model produced a text response
    Text(String),

    /// The model wants to call a tool
    ToolCall {
        /// Unique ID for this tool invocation (used to correlate the result)
        id: String,
        /// Name of the tool to call
        name: String,
        /// Arguments as a JSON object
        arguments: Value,
    },
}

/// Errors from the LLM client
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error(
        "Provider '{0}' requires an API key (set {1} env var or add api_key to ~/.rush/ai.toml)"
    )]
    MissingApiKey(String, String),
}

/// Trait implemented by each provider backend (Ollama, OpenAI, Anthropic)
pub trait LlmProvider: Send {
    /// Send a chat request and return one response.
    ///
    /// If `tools` is non-empty and the model decides to call one, the response
    /// is `Response::ToolCall`. Otherwise it is `Response::Text`.
    fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> Result<Response, LlmError>;

    /// Human-readable provider name (e.g. "ollama", "openai")
    fn name(&self) -> &str;

    /// Whether this provider supports tool / function calling
    fn supports_tools(&self) -> bool;
}

/// The main entry point for LLM interaction.
///
/// Created once per session (or per request) and delegates to the configured
/// provider. Use `LlmClient::from_config` to build one from `~/.rush/ai.toml`.
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    pub config: LlmConfig,
}

impl LlmClient {
    /// Create a client from the given config.
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        let provider: Box<dyn LlmProvider> = match config.provider {
            ProviderType::Ollama => Box::new(OllamaProvider::new(&config)),
            ProviderType::OpenAi => {
                let key = config.resolved_api_key().ok_or_else(|| {
                    LlmError::MissingApiKey("openai".to_string(), "OPENAI_API_KEY".to_string())
                })?;
                Box::new(OpenAiProvider::new(&config, key))
            }
            ProviderType::Anthropic => {
                let key = config.resolved_api_key().ok_or_else(|| {
                    LlmError::MissingApiKey(
                        "anthropic".to_string(),
                        "ANTHROPIC_API_KEY".to_string(),
                    )
                })?;
                Box::new(AnthropicProvider::new(&config, key))
            }
        };

        Ok(Self { provider, config })
    }

    /// Load config from `~/.rush/ai.toml` and create a client.
    pub fn from_config() -> Result<Self, LlmError> {
        let config = LlmConfig::load().map_err(LlmError::Config)?;
        Self::new(config)
    }

    /// Send messages to the model and get a response.
    ///
    /// Delegates to the underlying provider's `chat` method.
    pub fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> Result<Response, LlmError> {
        self.provider.chat(messages, tools)
    }

    /// The name of the active provider (e.g. "ollama")
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Whether the active provider supports tool calling
    pub fn supports_tools(&self) -> bool {
        self.provider.supports_tools()
    }
}
