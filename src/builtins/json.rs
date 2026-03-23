use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// Parse JSON from a string
fn parse_json(input: &str) -> Result<Value> {
    serde_json::from_str(input).context("Failed to parse JSON")
}

/// Parse JSON from stdin, file, or argument
fn get_json_input(args: &[String], stdin_data: Option<&[u8]>) -> Result<(Value, usize)> {
    // Check if we have stdin data
    if let Some(data) = stdin_data {
        let json_str = String::from_utf8_lossy(data);
        let value = parse_json(&json_str)?;
        return Ok((value, 0));
    }

    // Check if last argument is a file path or JSON string
    if args.is_empty() {
        // Try to read from stdin
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        let value = parse_json(&buffer)?;
        return Ok((value, 0));
    }

    let last_arg = &args[args.len() - 1];

    // Check if it's a file
    if Path::new(last_arg).exists() {
        let content = fs::read_to_string(last_arg)
            .with_context(|| format!("Failed to read file: {}", last_arg))?;
        let value = parse_json(&content)?;
        return Ok((value, 1)); // 1 argument consumed (the file path)
    }

    // Try to parse as JSON string
    if let Ok(value) = parse_json(last_arg) {
        return Ok((value, 1)); // 1 argument consumed (the JSON string)
    }

    // No valid JSON input found
    Err(anyhow!(
        "No valid JSON input provided. Use stdin, a file path, or a JSON string."
    ))
}

/// Navigate a JSON path (simple implementation)
/// Supports: .field, .field.nested, .[0], .[].field
fn navigate_path(value: &Value, path: &str) -> Result<Value> {
    if path.is_empty() || path == "." {
        return Ok(value.clone());
    }

    let path = path.trim_start_matches('.');
    let parts: Vec<&str> = path.split('.').collect();

    let mut current = value.clone();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        // Handle array access: [0] or []
        if part.starts_with('[') && part.ends_with(']') {
            let index_str = &part[1..part.len() - 1];

            if index_str.is_empty() {
                // .[] - iterate over array
                if let Value::Array(arr) = &current {
                    return Ok(Value::Array(arr.clone()));
                } else {
                    return Err(anyhow!("Cannot iterate non-array with []"));
                }
            } else {
                // .[0] - array index
                let index: usize = index_str
                    .parse()
                    .with_context(|| format!("Invalid array index: {}", index_str))?;

                if let Value::Array(arr) = &current {
                    current = arr
                        .get(index)
                        .ok_or_else(|| anyhow!("Array index {} out of bounds", index))?
                        .clone();
                } else {
                    return Err(anyhow!("Cannot index non-array value"));
                }
            }
        } else if part.contains('[') {
            // Handle field[0] notation
            let bracket_pos = part.find('[').unwrap();
            let field = &part[..bracket_pos];
            let rest = &part[bracket_pos..];

            // Navigate to field first
            if let Value::Object(map) = &current {
                current = map
                    .get(field)
                    .ok_or_else(|| anyhow!("Field '{}' not found", field))?
                    .clone();
            } else {
                return Err(anyhow!("Cannot access field '{}' on non-object", field));
            }

            // Then handle array access
            if rest.starts_with('[') && rest.ends_with(']') {
                let index_str = &rest[1..rest.len() - 1];
                let index: usize = index_str
                    .parse()
                    .with_context(|| format!("Invalid array index: {}", index_str))?;

                if let Value::Array(arr) = &current {
                    current = arr
                        .get(index)
                        .ok_or_else(|| anyhow!("Array index {} out of bounds", index))?
                        .clone();
                } else {
                    return Err(anyhow!("Cannot index non-array value"));
                }
            }
        } else {
            // Regular field access
            if let Value::Object(map) = &current {
                current = map
                    .get(part)
                    .ok_or_else(|| anyhow!("Field '{}' not found", part))?
                    .clone();
            } else {
                return Err(anyhow!("Cannot access field '{}' on non-object", part));
            }
        }
    }

    Ok(current)
}

/// Set a value at a JSON path
fn set_at_path(value: &mut Value, path: &str, new_value: Value) -> Result<()> {
    if path.is_empty() || path == "." {
        *value = new_value;
        return Ok(());
    }

    let path = path.trim_start_matches('.');
    let parts: Vec<&str> = path.split('.').collect();

    if parts.is_empty() {
        *value = new_value;
        return Ok(());
    }

    let mut current = value;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if part.is_empty() {
            continue;
        }

        // Handle array access
        if part.starts_with('[') && part.ends_with(']') {
            let index_str = &part[1..part.len() - 1];
            let index: usize = index_str
                .parse()
                .with_context(|| format!("Invalid array index: {}", index_str))?;

            if let Value::Array(arr) = current {
                if is_last {
                    if index >= arr.len() {
                        return Err(anyhow!("Array index {} out of bounds", index));
                    }
                    arr[index] = new_value;
                    return Ok(());
                } else {
                    current = arr
                        .get_mut(index)
                        .ok_or_else(|| anyhow!("Array index {} out of bounds", index))?;
                }
            } else {
                return Err(anyhow!("Cannot index non-array value"));
            }
        } else {
            // Regular field access
            if let Value::Object(map) = current {
                if is_last {
                    map.insert(part.to_string(), new_value);
                    return Ok(());
                } else {
                    current = map
                        .get_mut(*part)
                        .ok_or_else(|| anyhow!("Field '{}' not found", part))?;
                }
            } else {
                return Err(anyhow!("Cannot access field '{}' on non-object", part));
            }
        }
    }

    Ok(())
}

/// Format value for output
fn format_value(value: &Value, raw: bool) -> String {
    match value {
        Value::String(s) if raw => s.clone(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Null if raw => String::new(),
        _ => value.to_string(),
    }
}

/// json_get builtin: Extract values from JSON
/// Usage: json_get [OPTIONS] PATH [PATH...] [FILE|JSON]
///   -r, --raw     Output raw strings without quotes
///   -c, --compact Compact JSON output
///   -p, --pretty  Pretty-print JSON output
pub fn builtin_json_get(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_json_get_impl(args, None)
}

pub fn builtin_json_get_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    builtin_json_get_impl(args, Some(stdin_data))
}

fn builtin_json_get_impl(args: &[String], stdin_data: Option<&[u8]>) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "Usage: json_get [OPTIONS] PATH [PATH...] [FILE|JSON]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let mut raw = false;
    let mut compact = false;
    let mut pretty = false;
    let mut paths = Vec::new();
    let mut i = 0;

    // Parse arguments
    while i < args.len() {
        let arg = &args[i];
        if arg == "-r" || arg == "--raw" {
            raw = true;
        } else if arg == "-c" || arg == "--compact" {
            compact = true;
        } else if arg == "-p" || arg == "--pretty" {
            pretty = true;
        } else if !arg.starts_with('-') {
            paths.push(arg.clone());
        } else {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("json_get: unknown option: {}\n", arg),
                exit_code: 1,
                error: None,
            });
        }
        i += 1;
    }

    if paths.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "json_get: no path specified\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    // Get JSON input
    let (json_value, consumed) = match get_json_input(&paths, stdin_data) {
        Ok(result) => result,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("json_get: {}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    // Remove consumed arguments (file path or JSON string)
    let query_paths = if consumed > 0 {
        &paths[..paths.len() - consumed]
    } else {
        &paths[..]
    };

    if query_paths.is_empty() {
        // No path specified, output the whole JSON
        let output = if pretty {
            serde_json::to_string_pretty(&json_value).unwrap()
        } else if compact {
            json_value.to_string()
        } else {
            serde_json::to_string_pretty(&json_value).unwrap()
        };
        return Ok(ExecutionResult::success(output + "\n"));
    }

    let mut output = String::new();
    let mut exit_code = 0;

    // Process each path
    for path in query_paths {
        match navigate_path(&json_value, path) {
            Ok(result) => {
                // Handle array iteration
                if let Value::Array(arr) = &result {
                    for item in arr {
                        if pretty {
                            output.push_str(&serde_json::to_string_pretty(item).unwrap());
                        } else {
                            output.push_str(&format_value(item, raw));
                        }
                        output.push('\n');
                    }
                } else {
                    if pretty {
                        output.push_str(&serde_json::to_string_pretty(&result).unwrap());
                    } else {
                        output.push_str(&format_value(&result, raw));
                    }
                    output.push('\n');
                }
            }
            Err(e) => {
                eprintln!("json_get: {}: {}", path, e);
                exit_code = 1;
            }
        }
    }

    Ok(ExecutionResult {
        output: Output::Text(output),
        stderr: String::new(),
        exit_code,
        error: None,
    })
}

/// json_set builtin: Modify JSON values
/// Usage: json_set PATH VALUE [FILE|JSON]
///   -c, --compact Compact JSON output
///   -p, --pretty  Pretty-print JSON output (default)
pub fn builtin_json_set(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_json_set_impl(args, None)
}

pub fn builtin_json_set_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    builtin_json_set_impl(args, Some(stdin_data))
}

fn builtin_json_set_impl(args: &[String], stdin_data: Option<&[u8]>) -> Result<ExecutionResult> {
    if args.len() < 2 {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "Usage: json_set [OPTIONS] PATH VALUE [FILE|JSON]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let mut compact = false;
    let mut non_option_args = Vec::new();
    let mut i = 0;

    // Parse arguments
    while i < args.len() {
        let arg = &args[i];
        if arg == "-c" || arg == "--compact" {
            compact = true;
        } else if arg == "-p" || arg == "--pretty" {
            compact = false;
        } else if !arg.starts_with('-') {
            non_option_args.push(arg.clone());
        } else {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("json_set: unknown option: {}\n", arg),
                exit_code: 1,
                error: None,
            });
        }
        i += 1;
    }

    if non_option_args.len() < 2 {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "json_set: requires PATH and VALUE\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let path = &non_option_args[0];
    let value_str = &non_option_args[1];

    // Parse the new value
    let new_value = match parse_json(value_str) {
        Ok(v) => v,
        Err(_) => {
            // If not valid JSON, treat as a string
            Value::String(value_str.clone())
        }
    };

    // Get JSON input (from remaining args or stdin)
    let input_args = if non_option_args.len() > 2 {
        &non_option_args[2..]
    } else {
        &[]
    };

    let mut json_value = match get_json_input(input_args, stdin_data) {
        Ok((val, _)) => val,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("json_set: {}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    // Set the value
    if let Err(e) = set_at_path(&mut json_value, path, new_value) {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("json_set: {}\n", e),
            exit_code: 1,
            error: None,
        });
    }

    // Output the modified JSON
    let output = if compact {
        json_value.to_string()
    } else {
        serde_json::to_string_pretty(&json_value).unwrap()
    };

    Ok(ExecutionResult::success(output + "\n"))
}

/// json_query builtin: Advanced JSON querying
/// Usage: json_query QUERY [FILE|JSON]
///   Currently supports simple filters and selections
pub fn builtin_json_query(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    builtin_json_query_impl(args, None)
}

pub fn builtin_json_query_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    builtin_json_query_impl(args, Some(stdin_data))
}

fn builtin_json_query_impl(args: &[String], stdin_data: Option<&[u8]>) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "Usage: json_query QUERY [FILE|JSON]\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    let query = &args[0];
    let input_args = if args.len() > 1 { &args[1..] } else { &[] };

    // Get JSON input
    let (json_value, _) = match get_json_input(input_args, stdin_data) {
        Ok(result) => result,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: format!("json_query: {}\n", e),
                exit_code: 1,
                error: None,
            });
        }
    };

    // For now, implement a simple query parser
    // Format: ".path | select(.field == value)"
    // or just ".path"

    let result = if query.contains('|') {
        // Complex query with pipe and filter
        execute_complex_query(&json_value, query)?
    } else {
        // Simple path navigation
        navigate_path(&json_value, query)?
    };

    let output = serde_json::to_string_pretty(&result).unwrap();
    Ok(ExecutionResult::success(output + "\n"))
}

/// Execute a complex query with pipes and filters
fn execute_complex_query(value: &Value, query: &str) -> Result<Value> {
    let parts: Vec<&str> = query.split('|').map(|s| s.trim()).collect();

    let mut current = value.clone();

    for part in parts {
        if part.starts_with("select(") && part.ends_with(')') {
            // Extract the filter expression
            let filter = &part[7..part.len() - 1];
            current = apply_filter(&current, filter)?;
        } else if part == "length" {
            // Get length of array or object
            current = match &current {
                Value::Array(arr) => json!(arr.len()),
                Value::Object(map) => json!(map.len()),
                Value::String(s) => json!(s.len()),
                _ => {
                    return Err(anyhow!(
                        "length can only be used on arrays, objects, or strings"
                    ))
                }
            };
        } else if part == "keys" {
            // Get keys of object
            current = match &current {
                Value::Object(map) => {
                    let keys: Vec<String> = map.keys().cloned().collect();
                    json!(keys)
                }
                _ => return Err(anyhow!("keys can only be used on objects")),
            };
        } else if part == "values" {
            // Get values of object
            current = match &current {
                Value::Object(map) => {
                    let values: Vec<Value> = map.values().cloned().collect();
                    json!(values)
                }
                _ => return Err(anyhow!("values can only be used on objects")),
            };
        } else {
            // Path navigation
            current = navigate_path(&current, part)?;
        }
    }

    Ok(current)
}

/// Apply a filter to an array
fn apply_filter(value: &Value, filter: &str) -> Result<Value> {
    if let Value::Array(arr) = value {
        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| matches_filter(item, filter))
            .cloned()
            .collect();
        Ok(Value::Array(filtered))
    } else {
        // Apply filter to single value
        if matches_filter(value, filter) {
            Ok(value.clone())
        } else {
            Ok(Value::Null)
        }
    }
}

/// Check if a value matches a filter expression
/// Supports: .field == "value", .field == number, .field == true/false
fn matches_filter(value: &Value, filter: &str) -> bool {
    // Simple parsing: .field == value
    if let Some(eq_pos) = filter.find("==") {
        let left = filter[..eq_pos].trim();
        let right = filter[eq_pos + 2..].trim();

        // Navigate to the field
        let field_value = match navigate_path(value, left) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // Compare values
        let right_value = if right.starts_with('"') && right.ends_with('"') {
            Value::String(right[1..right.len() - 1].to_string())
        } else if right == "true" {
            Value::Bool(true)
        } else if right == "false" {
            Value::Bool(false)
        } else if right == "null" {
            Value::Null
        } else if let Ok(num) = right.parse::<i64>() {
            json!(num)
        } else if let Ok(num) = right.parse::<f64>() {
            json!(num)
        } else {
            return false;
        };

        field_value == right_value
    } else if let Some(ne_pos) = filter.find("!=") {
        let left = filter[..ne_pos].trim();
        let right = filter[ne_pos + 2..].trim();

        let field_value = match navigate_path(value, left) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let right_value = if right.starts_with('"') && right.ends_with('"') {
            Value::String(right[1..right.len() - 1].to_string())
        } else if right == "true" {
            Value::Bool(true)
        } else if right == "false" {
            Value::Bool(false)
        } else if right == "null" {
            Value::Null
        } else if let Ok(num) = right.parse::<i64>() {
            json!(num)
        } else if let Ok(num) = right.parse::<f64>() {
            json!(num)
        } else {
            return false;
        };

        field_value != right_value
    } else {
        // Just a path - check if it exists and is truthy
        matches!(
            navigate_path(value, filter),
            Ok(Value::Bool(true))
                | Ok(Value::String(_))
                | Ok(Value::Number(_))
                | Ok(Value::Array(_))
                | Ok(Value::Object(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        let json = r#"{"name": "test", "value": 42}"#;
        let result = parse_json(json).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_navigate_simple_field() {
        let json = json!({"name": "Alice", "age": 30});
        let result = navigate_path(&json, ".name").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_navigate_nested_field() {
        let json = json!({"user": {"name": "Alice", "age": 30}});
        let result = navigate_path(&json, ".user.name").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_navigate_array_index() {
        let json = json!({"items": [1, 2, 3]});
        let result = navigate_path(&json, ".items.[0]").unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_navigate_array_iteration() {
        let json = json!([{"name": "Alice"}, {"name": "Bob"}]);
        let result = navigate_path(&json, ".[]").unwrap();
        assert!(result.is_array());
    }

    #[test]
    fn test_set_at_path_simple() {
        let mut json = json!({"name": "Alice"});
        set_at_path(&mut json, ".name", json!("Bob")).unwrap();
        assert_eq!(json["name"], "Bob");
    }

    #[test]
    fn test_set_at_path_nested() {
        let mut json = json!({"user": {"name": "Alice"}});
        set_at_path(&mut json, ".user.name", json!("Bob")).unwrap();
        assert_eq!(json["user"]["name"], "Bob");
    }

    #[test]
    fn test_set_at_path_array() {
        let mut json = json!({"items": [1, 2, 3]});
        set_at_path(&mut json, ".items.[1]", json!(99)).unwrap();
        assert_eq!(json["items"][1], 99);
    }

    #[test]
    fn test_filter_match_string() {
        let value = json!({"status": "active"});
        assert!(matches_filter(&value, r#".status == "active""#));
        assert!(!matches_filter(&value, r#".status == "inactive""#));
    }

    #[test]
    fn test_filter_match_number() {
        let value = json!({"count": 42});
        assert!(matches_filter(&value, ".count == 42"));
        assert!(!matches_filter(&value, ".count == 43"));
    }

    #[test]
    fn test_filter_match_bool() {
        let value = json!({"enabled": true});
        assert!(matches_filter(&value, ".enabled == true"));
        assert!(!matches_filter(&value, ".enabled == false"));
    }

    #[test]
    fn test_apply_filter_array() {
        let json = json!([
            {"name": "Alice", "active": true},
            {"name": "Bob", "active": false},
            {"name": "Charlie", "active": true}
        ]);
        let result = apply_filter(&json, ".active == true").unwrap();
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_json_get_simple() {
        let mut runtime = Runtime::new();
        let json = r#"{"name": "Alice", "age": 30}"#;
        let args = vec![".name".to_string(), json.to_string()];
        let result = builtin_json_get(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout().trim(), r#""Alice""#);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_json_get_raw() {
        let mut runtime = Runtime::new();
        let json = r#"{"name": "Alice"}"#;
        let args = vec!["-r".to_string(), ".name".to_string(), json.to_string()];
        let result = builtin_json_get(&args, &mut runtime).unwrap();
        assert_eq!(result.stdout().trim(), "Alice");
    }

    #[test]
    fn test_json_set_simple() {
        let mut runtime = Runtime::new();
        let json = r#"{"name": "Alice"}"#;
        let args = vec![".name".to_string(), "\"Bob\"".to_string(), json.to_string()];
        let result = builtin_json_set(&args, &mut runtime).unwrap();
        assert!(result.stdout().contains(r#""Bob""#));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_json_query_simple() {
        let mut runtime = Runtime::new();
        let json = r#"{"user": {"name": "Alice"}}"#;
        let args = vec![".user.name".to_string(), json.to_string()];
        let result = builtin_json_query(&args, &mut runtime).unwrap();
        assert!(result.stdout().contains("Alice"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_json_query_with_filter() {
        let mut runtime = Runtime::new();
        let json = r#"[{"name": "Alice", "active": true}, {"name": "Bob", "active": false}]"#;
        let args = vec![
            ".[] | select(.active == true)".to_string(),
            json.to_string(),
        ];
        let result = builtin_json_query(&args, &mut runtime).unwrap();
        assert!(result.stdout().contains("Alice"));
        assert!(!result.stdout().contains("Bob"));
    }
}
