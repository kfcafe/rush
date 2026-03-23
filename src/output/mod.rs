// Output formatting (text and JSON)

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct OutputFormatter {
    json_mode: bool,
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormatter {
    pub fn new() -> Self {
        Self { json_mode: false }
    }

    pub fn set_json_mode(&mut self, enabled: bool) {
        self.json_mode = enabled;
    }

    pub fn is_json_mode(&self) -> bool {
        self.json_mode
    }

    pub fn format_text(&self, data: &str) -> String {
        data.to_string()
    }

    pub fn format_json(&self, value: &Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }

    pub fn format_result(&self, stdout: &str, stderr: &str, exit_code: i32) -> String {
        if self.json_mode {
            let result = json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code,
                "success": exit_code == 0
            });
            self.format_json(&result)
        } else {
            stdout.to_string()
        }
    }
}

// Helper for converting command output to JSON
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
    pub data: Value,
    pub metadata: HashMap<String, Value>,
}

impl JsonOutput {
    pub fn new(data: Value) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Strip ANSI escape sequences from a string.
///
/// Removes all sequences of the form `\x1b[...m` and similar control codes,
/// leaving only the plain text content. Used in agent mode to ensure output
/// is clean for programmatic consumers.
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Skip ESC [ ... <final byte in range 0x40–0x7e>
            i += 2;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            i += 1; // skip the final byte
        } else if bytes[i] == 0x1b && i + 1 < bytes.len() {
            // Other ESC sequences (e.g. ESC c) — skip just the two bytes
            i += 2;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

impl std::fmt::Display for JsonOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string_pretty(self).unwrap_or_default()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_mode() {
        let formatter = OutputFormatter::new();
        assert!(!formatter.is_json_mode());
        let output = formatter.format_result("hello\n", "", 0);
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn test_json_mode() {
        let mut formatter = OutputFormatter::new();
        formatter.set_json_mode(true);
        assert!(formatter.is_json_mode());

        let output = formatter.format_result("hello\n", "", 0);
        assert!(output.contains("\"stdout\""));
        assert!(output.contains("\"success\": true"));
    }

    #[test]
    fn test_json_output() {
        let data = json!({"files": ["file1.txt", "file2.txt"]});
        let output = JsonOutput::new(data).with_metadata("count".to_string(), json!(2));

        let json_str = output.to_string();
        assert!(json_str.contains("files"));
        assert!(json_str.contains("metadata"));
    }
}
