//! OpenAI provider — chat completions API with function/tool calling
//!
//! Uses the standard `/v1/chat/completions` endpoint. Requires `OPENAI_API_KEY`
//! (or `api_key` in `~/.rush/ai.toml`).

use crate::ai::client::{LlmError, LlmProvider, Message, Response, Role, Tool};
use crate::ai::config::LlmConfig;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

pub struct OpenAiProvider {
    base_url: String,
    model: String,
    api_key: String,
}

impl OpenAiProvider {
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
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }
}

fn to_openai_message(msg: &Message) -> Value {
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

fn to_openai_tool(tool: &Tool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

fn parse_response(body: Value) -> Result<Response, LlmError> {
    let choice = body
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| LlmError::Parse("OpenAI response has no choices".to_string()))?;

    let message = choice
        .get("message")
        .ok_or_else(|| LlmError::Parse("Choice missing 'message'".to_string()))?;

    // Check for tool calls
    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        if let Some(call) = tool_calls.first() {
            let id = call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("call-0")
                .to_string();

            let function = call
                .get("function")
                .ok_or_else(|| LlmError::Parse("Tool call missing 'function'".to_string()))?;

            let name = function
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LlmError::Parse("Function missing 'name'".to_string()))?
                .to_string();

            // OpenAI returns arguments as a JSON string
            let arguments_str = function
                .get("arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let arguments: Value = serde_json::from_str(arguments_str)
                .unwrap_or_else(|_| json!({ "raw": arguments_str }));

            return Ok(Response::ToolCall {
                id,
                name,
                arguments,
            });
        }
    }

    let content = message
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LlmError::Parse("Message missing 'content'".to_string()))?
        .to_string();

    Ok(Response::Text(content))
}

impl LlmProvider for OpenAiProvider {
    fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> Result<Response, LlmError> {
        let messages_json: Vec<Value> = messages.iter().map(to_openai_message).collect();

        let mut body = json!({
            "model": self.model,
            "messages": messages_json,
        });

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = tools.iter().map(to_openai_tool).collect();
                body["tool_choice"] = json!("auto");
            }
        }

        let response = ureq::post(&self.endpoint())
            .header("Authorization", &self.auth_header())
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| {
                // ureq 3.x surfaces HTTP error responses as Err when
                // http_status_as_error is true (the default)
                LlmError::Http(e.to_string())
            })?;

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
        "openai"
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
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello from OpenAI"
                },
                "finish_reason": "stop"
            }]
        });
        match parse_response(body).unwrap() {
            Response::Text(t) => assert_eq!(t, "Hello from OpenAI"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_parse_tool_call_response() {
        let body = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "run_shell",
                            "arguments": "{\"command\": \"ls\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        match parse_response(body).unwrap() {
            Response::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "run_shell");
                assert_eq!(arguments["command"], "ls");
            }
            _ => panic!("expected ToolCall"),
        }
    }
}
