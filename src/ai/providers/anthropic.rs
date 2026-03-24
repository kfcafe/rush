//! Anthropic provider — Claude models via the Messages API
//!
//! Uses `POST /v1/messages`. Requires `ANTHROPIC_API_KEY` (or `api_key` in
//! `~/.rush/ai.toml`). Tool use is via Anthropic's `tool_use` content blocks.

use crate::ai::client::{LlmError, LlmProvider, Message, Response, Role, Tool};
use crate::ai::config::LlmConfig;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    base_url: String,
    model: String,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(config: &LlmConfig, api_key: String) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        Self {
            base_url,
            model: config.model.clone(),
            api_key,
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }
}

/// Anthropic's API separates system messages from the conversation history.
/// Returns `(system_prompt, conversation_messages_json)`.
fn split_messages(messages: &[Message]) -> (Option<String>, Vec<Value>) {
    let mut system = None;
    let mut conversation = Vec::new();

    for msg in messages {
        match msg.role {
            Role::System => {
                // Anthropic only supports a single top-level system prompt
                system = Some(msg.content.clone());
            }
            Role::User => {
                conversation.push(json!({ "role": "user", "content": msg.content }));
            }
            Role::Assistant => {
                conversation.push(json!({ "role": "assistant", "content": msg.content }));
            }
            Role::Tool => {
                // Tool results go back as user messages with a tool_result content block
                let id = msg.tool_call_id.as_deref().unwrap_or("tool-0");
                conversation.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": msg.content,
                    }]
                }));
            }
        }
    }

    (system, conversation)
}

fn to_anthropic_tool(tool: &Tool) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

fn parse_response(body: Value) -> Result<Response, LlmError> {
    let content = body
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LlmError::Parse("Anthropic response missing 'content' array".to_string()))?;

    // Check for tool_use blocks first
    for block in content {
        if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
            let id = block
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("tool-0")
                .to_string();

            let name = block
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LlmError::Parse("tool_use block missing 'name'".to_string()))?
                .to_string();

            let arguments = block.get("input").cloned().unwrap_or(json!({}));

            return Ok(Response::ToolCall {
                id,
                name,
                arguments,
            });
        }
    }

    // Gather text blocks
    let text: String = content
        .iter()
        .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("text"))
        .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() {
        return Err(LlmError::Parse(
            "Anthropic response has no text content".to_string(),
        ));
    }

    Ok(Response::Text(text))
}

impl LlmProvider for AnthropicProvider {
    fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> Result<Response, LlmError> {
        let (system, conversation) = split_messages(messages);

        let mut body = json!({
            "model": self.model,
            "messages": conversation,
            "max_tokens": 4096,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = tools.iter().map(to_anthropic_tool).collect();
            }
        }

        let response = ureq::post(&self.endpoint())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let status = response.status();
        let response_body: Value = response
            .into_body()
            .read_json()
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        if !status.is_success() {
            let message = response_body
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(LlmError::Api {
                status: status.as_u16(),
                message,
            });
        }

        parse_response(response_body)
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn supports_tools(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_response() {
        let body = json!({
            "content": [{
                "type": "text",
                "text": "Hello from Claude"
            }],
            "stop_reason": "end_turn"
        });
        match parse_response(body).unwrap() {
            Response::Text(t) => assert_eq!(t, "Hello from Claude"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_parse_tool_use_response() {
        let body = json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_01",
                "name": "read_file",
                "input": { "path": "/etc/hosts" }
            }],
            "stop_reason": "tool_use"
        });
        match parse_response(body).unwrap() {
            Response::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "read_file");
                assert_eq!(arguments["path"], "/etc/hosts");
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn test_split_messages_with_system() {
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];
        let (system, conversation) = split_messages(&messages);
        assert_eq!(system, Some("You are a helpful assistant".to_string()));
        assert_eq!(conversation.len(), 2);
        assert_eq!(conversation[0]["role"], "user");
        assert_eq!(conversation[1]["role"], "assistant");
    }

    #[test]
    fn test_split_messages_tool_result() {
        let messages = vec![
            Message::user("Run ls"),
            Message::tool_result("toolu_01", "file1.txt\nfile2.txt"),
        ];
        let (_, conversation) = split_messages(&messages);
        assert_eq!(conversation.len(), 2);
        // Tool result becomes a user message with tool_result content block
        assert_eq!(conversation[1]["role"], "user");
        assert_eq!(conversation[1]["content"][0]["type"], "tool_result");
        assert_eq!(conversation[1]["content"][0]["tool_use_id"], "toolu_01");
    }
}
