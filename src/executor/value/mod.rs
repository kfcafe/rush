use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub mod render;

/// Core value types for structured data in rush shell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Render table as CSV
    pub fn to_csv(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&self.columns.join(","));
        output.push('\n');

        // Rows
        for row in &self.rows {
            let values: Vec<String> = self
                .columns
                .iter()
                .map(|col| {
                    let val = row.get(col).map(|v| v.to_text()).unwrap_or_default();
                    // Escape commas and quotes
                    if val.contains(',') || val.contains('"') {
                        format!("\"{}\"", val.replace('"', "\"\""))
                    } else {
                        val
                    }
                })
                .collect();
            output.push_str(&values.join(","));
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
    use serde::{Deserialize, Deserializer, Serializer};
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
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_to_text_primitives() {
        assert_eq!(Value::String("hello".into()).to_text(), "hello");
        assert_eq!(Value::Int(42).to_text(), "42");
        assert_eq!(Value::Float(3.5).to_text(), "3.5");
        assert_eq!(Value::Bool(true).to_text(), "true");
        assert_eq!(Value::Null.to_text(), "");
    }

    #[test]
    fn test_value_to_text_list() {
        let list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(list.to_text(), "1\n2\n3");
    }

    #[test]
    fn test_table_to_tsv() {
        let mut table = Table::new(vec!["name".into(), "age".into()]);

        let mut row1 = HashMap::new();
        row1.insert("name".into(), Value::String("Alice".into()));
        row1.insert("age".into(), Value::Int(30));
        table.push_row(row1);

        let mut row2 = HashMap::new();
        row2.insert("name".into(), Value::String("Bob".into()));
        row2.insert("age".into(), Value::Int(25));
        table.push_row(row2);

        let tsv = table.to_tsv();
        assert!(tsv.contains("name\tage"));
        assert!(tsv.contains("Alice\t30"));
        assert!(tsv.contains("Bob\t25"));
    }

    #[test]
    fn test_json_roundtrip() {
        let original = Value::Record(HashMap::from([
            ("key".into(), Value::String("value".into())),
            ("num".into(), Value::Int(42)),
        ]));

        let json = original.to_json();
        let parsed = Value::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_filesize_formatting() {
        assert_eq!(format_filesize(0), "0 B");
        assert_eq!(format_filesize(500), "500 B");
        assert_eq!(format_filesize(1024), "1.00 KB");
        assert_eq!(format_filesize(1536), "1.50 KB");
        assert_eq!(format_filesize(1048576), "1.00 MB");
        assert_eq!(format_filesize(1073741824), "1.00 GB");
    }

    #[test]
    fn test_table_capacity() {
        let table = Table::with_capacity(vec!["a".into(), "b".into()], 100);
        assert_eq!(table.rows.capacity(), 100);
        assert!(table.is_empty());
    }
}
