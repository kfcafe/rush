//! AI / LLM integration for Rush
//!
//! This module provides a provider-agnostic LLM client used by the shell's
//! agent features (`?` prefix, `|?` pipe operator).
//!
//! # Quick start
//!
//! ```no_run
//! use rush::ai::client::{LlmClient, Message};
//!
//! let client = LlmClient::from_config().unwrap();
//! let messages = vec![Message::user("List files in the current directory")];
//! let response = client.chat(&messages, None).unwrap();
//! ```
//!
//! # Configuration
//!
//! Reads `~/.rushrc`. If missing, defaults to Ollama on localhost.
//! ```toml
//! provider = "ollama"
//! model = "qwen2.5-coder:7b"
//! ```

pub mod agent;
pub mod client;
pub mod config;
pub mod providers;
pub mod tools;
pub mod wizard;

// Re-export the most commonly used types at module level
pub use agent::{execute_agent, Agent, ToolCall};
pub use client::{LlmClient, LlmError, LlmProvider, Message, Response, Role, Tool};
pub use config::{LlmConfig, ProviderType};
pub use wizard::setup_wizard;
