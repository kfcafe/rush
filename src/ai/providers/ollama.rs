//! Ollama provider — local LLM via the Ollama HTTP API
//!
//! Communicates with a local Ollama instance at `http://localhost:11434`
//! (or a custom URL from config). No API key required.
//!
//! Tool calling is supported for models that understand it (qwen2.5-coder,
//! llama3.1+, etc.). The Ollama `/api/chat` endpoint uses the same tool
//! calling format as OpenAI's chat completions API.

use crate::ai::client::{LlmError, LlmProvider, Message, Response, Role, Tool};
use crate::ai::config::LlmConfig;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";

pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(config: &LlmConfig) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        Self {
            base_url,
            model: config.model.clone(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }
}

/// Convert our internal `Message` type to the Ollama JSON format
fn to_ollama_message(msg: &Message) -> Value {
    let role = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };
    let mut obj = json!({ "role": role, "content": msg.content });
    if let Some(id) = &msg.tool_call_id {
        obj["tool_call_id"] = json!(id);
    }
    obj
}

/// Convert our `Tool` definition to the Ollama/OpenAI-style JSON schema
fn to_ollama_tool(tool: &Tool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

/// Extract a `Response` from the Ollama API response JSON
fn parse_response(body: Value) -> Result<Response, LlmError> {
    let message = body
        .get("message")
        .ok_or_else(|| LlmError::Parse("Ollama response missing 'message' field".to_string()))?;

    // Check for tool calls first
    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        if let Some(call) = tool_calls.first() {
            let function = call
                .get("function")
                .ok_or_else(|| LlmError::Parse("Tool call missing 'function'".to_string()))?;

            let name = function
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LlmError::Parse("Tool call function missing 'name'".to_string()))?
                .to_string();

            let arguments = function.get("arguments").cloned().unwrap_or(json!({}));

            // Ollama may return arguments as a JSON string — parse it if so
            let arguments = if let Some(s) = arguments.as_str() {
                serde_json::from_str(s).unwrap_or_else(|_| json!({ "raw": s }))
            } else {
                arguments
            };

            let id = call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("tool-0")
                .to_string();

            return Ok(Response::ToolCall {
                id,
                name,
                arguments,
            });
        }
    }

    // Plain text response
    let content = message
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LlmError::Parse("Ollama response missing 'content'".to_string()))?
        .to_string();

    Ok(Response::Text(content))
}

impl LlmProvider for OllamaProvider {
    fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> Result<Response, LlmError> {
        let messages_json: Vec<Value> = messages.iter().map(to_ollama_message).collect();

        let mut body = json!({
            "model": self.model,
            "messages": messages_json,
            "stream": false,
        });

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = tools.iter().map(to_ollama_tool).collect();
            }
        }

        let response = ureq::post(&self.endpoint())
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
                .get("error")
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
        "ollama"
    }

    fn supports_tools(&self) -> bool {
        // Tool support depends on the model, but Ollama 0.3+ passes tool call
        // definitions through to compatible models. We advertise support and
        // let incompatible models simply ignore the tools field.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::config::LlmConfig;

    #[test]
    fn test_endpoint_construction() {
        let mut config = LlmConfig::default();
        config.base_url = Some("http://localhost:11434/".to_string());
        let provider = OllamaProvider::new(&config);
        assert_eq!(provider.endpoint(), "http://localhost:11434/api/chat");
    }

    #[test]
    fn test_parse_text_response() {
        let body = json!({
            "message": {
                "role": "assistant",
                "content": "Hello, world!"
            },
            "done": true
        });
        let response = parse_response(body).unwrap();
        match response {
            Response::Text(text) => assert_eq!(text, "Hello, world!"),
            _ => panic!("expected Text response"),
        }
    }

    #[test]
    fn test_parse_tool_call_response() {
        let body = json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call-1",
                    "function": {
                        "name": "run_command",
                        "arguments": { "cmd": "ls -la" }
                    }
                }]
            },
            "done": true
        });
        let response = parse_response(body).unwrap();
        match response {
            Response::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call-1");
                assert_eq!(name, "run_command");
                assert_eq!(arguments["cmd"], "ls -la");
            }
            _ => panic!("expected ToolCall response"),
        }
    }

    #[test]
    fn test_parse_tool_call_with_string_arguments() {
        // Ollama sometimes returns arguments as a JSON string
        let body = json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call-2",
                    "function": {
                        "name": "run_command",
                        "arguments": "{\"cmd\": \"pwd\"}"
                    }
                }]
            },
            "done": true
        });
        let response = parse_response(body).unwrap();
        match response {
            Response::ToolCall { arguments, .. } => {
                assert_eq!(arguments["cmd"], "pwd");
            }
            _ => panic!("expected ToolCall response"),
        }
    }
}
