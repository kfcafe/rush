use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub mod render;

/// Core value types for structured data in rush shell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Value {
    // Primitives
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,

    // Collections
    List(Vec<Value>),
    Record(HashMap<String, Value>),
    Table(Table),

    // Shell-specific types
    Path(PathBuf),
    Duration(#[serde(with = "duration_serde")] Duration),
    Filesize(u64),
    #[serde(with = "chrono::serde::ts_seconds")]
    Date(chrono::DateTime<chrono::Utc>),

    // Error value for propagating structured errors
    Error(String),
}

/// Specialized table type for efficient tabular data
/// Optimized for ls, ps, jobs, find output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<HashMap<String, Value>>,
}

impl Table {
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    pub fn with_capacity(columns: Vec<String>, capacity: usize) -> Self {
        Self {
            columns,
            rows: Vec::with_capacity(capacity),
        }
    }

    pub fn push_row(&mut self, row: HashMap<String, Value>) {
        self.rows.push(row);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Value {
    /// Convert value to plain text representation
    /// Used when piping to external commands or displaying in terminal
    pub fn to_text(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            Value::List(items) => items
                .iter()
                .map(|v| v.to_text())
                .collect::<Vec<_>>()
                .join("\n"),
            Value::Record(map) => {
                // Render as JSON for records (most portable)
                serde_json::to_string_pretty(map).unwrap_or_default()
            }
            Value::Table(table) => {
                // Render as TSV for external commands (grep, awk, etc.)
                table.to_tsv()
            }
            Value::Path(p) => p.to_string_lossy().to_string(),
            Value::Duration(d) => format!("{:.3}s", d.as_secs_f64()),
            Value::Filesize(bytes) => format_filesize(*bytes),
            Value::Date(dt) => dt.to_rfc3339(),
            Value::Error(e) => e.clone(),
        }
    }

    /// Convert value to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
    }

    /// Convert value to pretty-printed JSON string
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self)
            .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
    }

    /// Try to parse JSON string into a Value
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Table {
    /// Render table as tab-separated values (TSV)
    /// Most portable format for Unix tools
    pub fn to_tsv(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&self.columns.join("\t"));
        output.push('\n');

        // Rows
        for row in &self.rows {
            let values: Vec<String> = self
                .columns
                .iter()
                .map(|col| row.get(col).map(|v| v.to_text()).unwrap_or_default())
                .collect();
            output.push_str(&values.join("\t"));
            output.push('\n');
        }

        output
    }
}

/// Format bytes as human-readable filesize
fn format_filesize(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

// Serde helper for Duration (not natively supported)
mod duration_serde {
    use serde::{Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(duration.as_secs_f64())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::Deserialize;
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}
