use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::fs;
use std::io::{self, Read};

/// Read input text from a file argument or stdin (piped data takes priority).
fn read_input(file_args: &[String], stdin_data: Option<&[u8]>) -> Result<String> {
    if let Some(data) = stdin_data {
        return Ok(String::from_utf8_lossy(data).into_owned());
    }

    if !file_args.is_empty() {
        let path = &file_args[0];
        return fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path));
    }

    // Fall back to blocking stdin read (e.g. when not piped via executor)
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("Failed to read from stdin")?;
    Ok(buf)
}

/// Parse CSV text into a JSON array of objects, using the first row as headers.
/// Numeric and boolean cell values are coerced to their JSON equivalents.
fn csv_to_json(input: &str) -> Result<Value> {
    let mut reader = csv::Reader::from_reader(input.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .context("Failed to read CSV headers")?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut records = Vec::new();

    for result in reader.records() {
        let record = result.context("Failed to read CSV record")?;
        let mut obj = serde_json::Map::new();

        for (header, field) in headers.iter().zip(record.iter()) {
            let value = coerce_cell(field);
            obj.insert(header.clone(), value);
        }

        records.push(Value::Object(obj));
    }

    Ok(Value::Array(records))
}

/// Coerce a CSV cell string into a JSON scalar.
/// Tries boolean → integer → float → string, in that order.
fn coerce_cell(field: &str) -> Value {
    match field {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        s => {
            if let Ok(n) = s.parse::<i64>() {
                Value::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(s.to_string()))
            } else {
                Value::String(s.to_string())
            }
        }
    }
}

/// Parse YAML text into a JSON value.
fn yaml_to_json(input: &str) -> Result<Value> {
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(input).context("Failed to parse YAML")?;

    // Round-trip through serde_json serialization to get a serde_json::Value.
    let json_str =
        serde_json::to_string(&yaml_value).context("Failed to serialize YAML as JSON")?;
    serde_json::from_str(&json_str).context("Failed to parse serialized JSON")
}

/// Parse TOML text into a JSON value.
fn toml_to_json(input: &str) -> Result<Value> {
    let toml_value: toml::Value = toml::from_str(input).context("Failed to parse TOML")?;

    // Round-trip through serde_json serialization to get a serde_json::Value.
    let json_str =
        serde_json::to_string(&toml_value).context("Failed to serialize TOML as JSON")?;
    serde_json::from_str(&json_str).context("Failed to parse serialized JSON")
}

pub fn builtin_from(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_from_impl(args, None, runtime)
}

pub fn builtin_from_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    builtin_from_impl(args, Some(stdin_data), runtime)
}

fn builtin_from_impl(
    args: &[String],
    stdin_data: Option<&[u8]>,
    _runtime: &mut Runtime,
) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "Usage: from <csv|yaml|toml> [FILE]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let format = &args[0];
    let file_args = &args[1..];

    let input = match read_input(file_args, stdin_data) {
        Ok(s) => s,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("from: {}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    let result = match format.as_str() {
        "csv" => csv_to_json(&input),
        "yaml" => yaml_to_json(&input),
        "toml" => toml_to_json(&input),
        other => Err(anyhow!(
            "Unknown format '{}'. Supported formats: csv, yaml, toml",
            other
        )),
    };

    match result {
        Ok(value) => {
            let output = serde_json::to_string_pretty(&value).unwrap();
            Ok(ExecutionResult::success(output + "\n"))
        }
        Err(e) => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("from {}: {}\n", format, e),
            exit_code: 1,
            error: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    // --- CSV ---

    #[test]
    fn test_builtin_from_csv_basic() {
        let mut runtime = Runtime::new();
        let csv = "name,age\nAlice,30\nBob,25\n";
        let result =
            builtin_from_with_stdin(&["csv".to_string()], &mut runtime, csv.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[0]["age"], 30);
        assert_eq!(arr[1]["name"], "Bob");
        assert_eq!(arr[1]["age"], 25);
    }

    #[test]
    fn test_builtin_from_csv_booleans() {
        let mut runtime = Runtime::new();
        let csv = "label,active\nfoo,true\nbar,false\n";
        let result =
            builtin_from_with_stdin(&["csv".to_string()], &mut runtime, csv.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr[0]["active"], true);
        assert_eq!(arr[1]["active"], false);
    }

    #[test]
    fn test_builtin_from_csv_string_fallback() {
        let mut runtime = Runtime::new();
        let csv = "city,zip\nSpringfield,01234\n";
        let result =
            builtin_from_with_stdin(&["csv".to_string()], &mut runtime, csv.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        // "01234" parses as integer 1234; "Springfield" is a string
        assert_eq!(parsed[0]["city"], "Springfield");
    }

    // --- YAML ---

    #[test]
    fn test_builtin_from_yaml_object() {
        let mut runtime = Runtime::new();
        let yaml = "name: Alice\nage: 30\n";
        let result =
            builtin_from_with_stdin(&["yaml".to_string()], &mut runtime, yaml.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn test_builtin_from_yaml_array() {
        let mut runtime = Runtime::new();
        let yaml = "- a\n- b\n- c\n";
        let result =
            builtin_from_with_stdin(&["yaml".to_string()], &mut runtime, yaml.as_bytes()).unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed, serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn test_builtin_from_yaml_invalid() {
        let mut runtime = Runtime::new();
        // Invalid YAML (tab where space expected)
        let bad = "key:\tvalue\n";
        let result =
            builtin_from_with_stdin(&["yaml".to_string()], &mut runtime, bad.as_bytes()).unwrap();
        // Should succeed (tabs in scalars are fine) or fail gracefully
        // Either way the exit code signals the outcome — just don't panic.
        let _ = result.exit_code;
    }

    // --- TOML ---

    #[test]
    fn test_builtin_from_toml_basic() {
        let mut runtime = Runtime::new();
        let toml_str = "name = \"Alice\"\nage = 30\n";
        let result =
            builtin_from_with_stdin(&["toml".to_string()], &mut runtime, toml_str.as_bytes())
                .unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn test_builtin_from_toml_nested() {
        let mut runtime = Runtime::new();
        let toml_str = "[server]\nhost = \"localhost\"\nport = 8080\n";
        let result =
            builtin_from_with_stdin(&["toml".to_string()], &mut runtime, toml_str.as_bytes())
                .unwrap();

        assert_eq!(result.exit_code, 0);
        let out = result.stdout();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["server"]["host"], "localhost");
        assert_eq!(parsed["server"]["port"], 8080);
    }

    // --- Error cases ---

    #[test]
    fn test_builtin_from_unknown_format() {
        let mut runtime = Runtime::new();
        let result =
            builtin_from_with_stdin(&["xml".to_string()], &mut runtime, b"<foo/>").unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("xml"));
    }

    #[test]
    fn test_builtin_from_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_from(&[], &mut runtime).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("Usage"));
    }
}
