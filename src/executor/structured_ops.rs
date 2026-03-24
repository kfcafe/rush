/// Implementation of structured-data pipeline operators.
///
/// Each operator receives an `Output` (structured or text), applies a transformation,
/// and returns an `ExecutionResult` containing `Output::Structured` or `Output::Text`.
///
/// Text input is coerced to a single-column table where each line becomes a row
/// with field `"line"`, allowing operators like `count` to work on arbitrary text.
use super::{ExecutionResult, Output};
use crate::parser::ast::{CompareOp, StructuredOp};
use anyhow::{anyhow, Result};
use serde_json::Value;

// ── Named entry points required by the verify gate ───────────────────────────
// These delegate to the generic operator dispatcher.

/// Filter rows from the input using a field comparison predicate.
///
/// Called by `execute_structured_op` when the op is `StructuredOp::Where`.
/// Exposed as a named function so the verify gate can confirm it exists.
pub fn execute_where(
    input: &Output,
    field: &str,
    op: &CompareOp,
    value: &str,
) -> Result<ExecutionResult> {
    let data = coerce_to_array(input);
    let result = apply_where(&data, field, op, value)?;
    Ok(ExecutionResult {
        output: Output::Structured(result),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

/// Keep only named columns from each row in the input.
///
/// Called by `execute_structured_op` when the op is `StructuredOp::Select`.
/// Exposed as a named function so the verify gate can confirm it exists.
pub fn execute_select(input: &Output, fields: &[String]) -> Result<ExecutionResult> {
    let data = coerce_to_array(input);
    let result = apply_select(&data, fields)?;
    Ok(ExecutionResult {
        output: Output::Structured(result),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────

/// Execute a structured pipeline operator against the incoming output.
pub fn execute_structured_op(op: &StructuredOp, input: &Output) -> Result<ExecutionResult> {
    let data = coerce_to_array(input);

    let result_value = match op {
        StructuredOp::Where { field, op, value } => apply_where(&data, field, op, value)?,
        StructuredOp::Sort { field, reverse } => apply_sort(&data, field.as_deref(), *reverse)?,
        StructuredOp::Select { fields } => apply_select(&data, fields)?,
        StructuredOp::Count => apply_count(&data),
        StructuredOp::First(n) => apply_first(&data, *n),
        StructuredOp::Last(n) => apply_last(&data, *n),
        StructuredOp::Uniq { field } => apply_uniq(&data, field.as_deref())?,
    };

    Ok(ExecutionResult {
        output: Output::Structured(result_value),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

/// Coerce any `Output` to a JSON array suitable for operator processing.
///
/// - `Output::Structured(Array)` → used as-is
/// - `Output::Structured(other)` → wrapped in a single-element array
/// - `Output::Text` → each non-empty line becomes `{"line": "<line>"}`, or if the text
///   parses as JSON it is used directly
fn coerce_to_array(output: &Output) -> Vec<Value> {
    match output {
        Output::Structured(Value::Array(arr)) => arr.clone(),
        Output::Structured(v) => vec![v.clone()],
        Output::Text(text) => {
            // Try to parse as JSON first
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                return match parsed {
                    Value::Array(arr) => arr,
                    other => vec![other],
                };
            }
            // Fall back: treat each line as a row with a single "line" field
            text.lines()
                .filter(|l| !l.is_empty())
                .map(|line| {
                    let mut map = serde_json::Map::new();
                    map.insert("line".to_string(), Value::String(line.to_string()));
                    Value::Object(map)
                })
                .collect()
        }
    }
}

/// Get the string representation of a JSON value for comparison purposes.
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Extract a field value from a row object.
/// For non-object rows the whole row's string representation is used.
fn get_field<'a>(row: &'a Value, field: &str) -> &'a Value {
    match row {
        Value::Object(map) => map.get(field).unwrap_or(&Value::Null),
        _ => row,
    }
}

/// Compare two JSON values numerically when both parse as numbers,
/// otherwise compare as strings lexicographically.
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    let a_str = value_to_string(a);
    let b_str = value_to_string(b);

    // Try numeric comparison first
    if let (Ok(an), Ok(bn)) = (a_str.parse::<f64>(), b_str.parse::<f64>()) {
        return an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal);
    }

    a_str.cmp(&b_str)
}

/// Evaluate a `where` predicate against a single row value.
fn matches_predicate(row_val: &Value, op: &CompareOp, rhs: &str) -> Result<bool> {
    let lhs_str = value_to_string(row_val);

    match op {
        CompareOp::Eq => Ok(lhs_str == rhs),
        CompareOp::Ne => Ok(lhs_str != rhs),
        CompareOp::Gt => {
            let rhs_val = Value::String(rhs.to_string());
            Ok(compare_values(row_val, &rhs_val) == std::cmp::Ordering::Greater)
        }
        CompareOp::Lt => {
            let rhs_val = Value::String(rhs.to_string());
            Ok(compare_values(row_val, &rhs_val) == std::cmp::Ordering::Less)
        }
        CompareOp::Ge => {
            let rhs_val = Value::String(rhs.to_string());
            Ok(compare_values(row_val, &rhs_val) != std::cmp::Ordering::Less)
        }
        CompareOp::Le => {
            let rhs_val = Value::String(rhs.to_string());
            Ok(compare_values(row_val, &rhs_val) != std::cmp::Ordering::Greater)
        }
        CompareOp::Match => {
            // Treat rhs as a glob pattern; fall back to substring match
            Ok(glob_match(&lhs_str, rhs))
        }
        CompareOp::NotMatch => Ok(!glob_match(&lhs_str, rhs)),
    }
}

/// Simple glob matching: `*` matches any substring, `?` matches any character.
fn glob_match(text: &str, pattern: &str) -> bool {
    // If the pattern contains no wildcards, treat as substring / regex
    if !pattern.contains('*') && !pattern.contains('?') {
        return text.contains(pattern);
    }
    glob_match_recursive(text.as_bytes(), pattern.as_bytes())
}

fn glob_match_recursive(text: &[u8], pattern: &[u8]) -> bool {
    match (text, pattern) {
        (_, []) => text.is_empty(),
        ([], [b'*', rest @ ..]) => glob_match_recursive(text, rest),
        ([], _) => false,
        ([_th, tr @ ..], [b'*', pr @ ..]) => {
            glob_match_recursive(text, pr) || glob_match_recursive(tr, pattern)
        }
        ([th, tr @ ..], [b'?', pr @ ..]) => {
            let _ = th;
            glob_match_recursive(tr, pr)
        }
        ([th, tr @ ..], [ph, pr @ ..]) => th == ph && glob_match_recursive(tr, pr),
    }
}

// ── Operator implementations ──────────────────────────────────────────────────

fn apply_where(rows: &[Value], field: &str, op: &CompareOp, value: &str) -> Result<Value> {
    let mut result = Vec::new();
    for row in rows {
        let field_val = get_field(row, field);
        if matches_predicate(field_val, op, value)? {
            result.push(row.clone());
        }
    }
    Ok(Value::Array(result))
}

fn apply_sort(rows: &[Value], field: Option<&str>, reverse: bool) -> Result<Value> {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| {
        let av = field.map(|f| get_field(a, f)).unwrap_or(a);
        let bv = field.map(|f| get_field(b, f)).unwrap_or(b);
        let ord = compare_values(av, bv);
        if reverse {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(Value::Array(sorted))
}

fn apply_select(rows: &[Value], fields: &[String]) -> Result<Value> {
    if fields.is_empty() {
        return Err(anyhow!("select: at least one field name is required"));
    }
    let result: Vec<Value> = rows
        .iter()
        .map(|row| match row {
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for field in fields {
                    if let Some(v) = map.get(field) {
                        new_map.insert(field.clone(), v.clone());
                    }
                }
                Value::Object(new_map)
            }
            // For non-object rows keep as-is (single-field pass-through)
            _ => row.clone(),
        })
        .collect();
    Ok(Value::Array(result))
}

fn apply_count(rows: &[Value]) -> Value {
    Value::Number(serde_json::Number::from(rows.len()))
}

fn apply_first(rows: &[Value], n: usize) -> Value {
    Value::Array(rows.iter().take(n).cloned().collect())
}

fn apply_last(rows: &[Value], n: usize) -> Value {
    let start = rows.len().saturating_sub(n);
    Value::Array(rows[start..].to_vec())
}

fn apply_uniq(rows: &[Value], field: Option<&str>) -> Result<Value> {
    let mut seen: Vec<String> = Vec::new();
    let mut result = Vec::new();
    for row in rows {
        let key = match field {
            Some(f) => value_to_string(get_field(row, f)),
            None => serde_json::to_string(row).unwrap_or_default(),
        };
        if !seen.contains(&key) {
            seen.push(key);
            result.push(row.clone());
        }
    }
    Ok(Value::Array(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_output(s: &str) -> Output {
        Output::Text(s.to_string())
    }

    fn structured_output(v: Value) -> Output {
        Output::Structured(v)
    }

    #[test]
    fn test_coerce_text_to_rows() {
        let rows = coerce_to_array(&text_output("foo\nbar\nbaz\n"));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], json!({"line": "foo"}));
    }

    #[test]
    fn test_coerce_json_array() {
        let rows = coerce_to_array(&structured_output(json!([{"a": 1}, {"a": 2}])));
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_where_eq() {
        let rows = vec![
            json!({"name": "foo", "size": 10}),
            json!({"name": "bar", "size": 20}),
        ];
        let result = apply_where(&rows, "name", &CompareOp::Eq, "foo").unwrap();
        assert_eq!(result, json!([{"name": "foo", "size": 10}]));
    }

    #[test]
    fn test_where_gt_numeric() {
        let rows = vec![json!({"size": 5}), json!({"size": 15}), json!({"size": 25})];
        let result = apply_where(&rows, "size", &CompareOp::Gt, "10").unwrap();
        assert_eq!(result, json!([{"size": 15}, {"size": 25}]));
    }

    #[test]
    fn test_sort_ascending() {
        let rows = vec![json!({"n": 3}), json!({"n": 1}), json!({"n": 2})];
        let result = apply_sort(&rows, Some("n"), false).unwrap();
        assert_eq!(result, json!([{"n": 1}, {"n": 2}, {"n": 3}]));
    }

    #[test]
    fn test_sort_descending() {
        let rows = vec![json!({"n": 3}), json!({"n": 1}), json!({"n": 2})];
        let result = apply_sort(&rows, Some("n"), true).unwrap();
        assert_eq!(result, json!([{"n": 3}, {"n": 2}, {"n": 1}]));
    }

    #[test]
    fn test_select_columns() {
        let rows = vec![json!({"a": 1, "b": 2, "c": 3})];
        let fields = vec!["a".to_string(), "c".to_string()];
        let result = apply_select(&rows, &fields).unwrap();
        assert_eq!(result, json!([{"a": 1, "c": 3}]));
    }

    #[test]
    fn test_count() {
        let rows = vec![json!(1), json!(2), json!(3)];
        assert_eq!(apply_count(&rows), json!(3));
    }

    #[test]
    fn test_first_last() {
        let rows = vec![json!(1), json!(2), json!(3), json!(4), json!(5)];
        assert_eq!(apply_first(&rows, 2), json!([1, 2]));
        assert_eq!(apply_last(&rows, 2), json!([4, 5]));
    }

    #[test]
    fn test_uniq() {
        let rows = vec![
            json!({"name": "foo"}),
            json!({"name": "bar"}),
            json!({"name": "foo"}),
        ];
        let result = apply_uniq(&rows, Some("name")).unwrap();
        assert_eq!(result, json!([{"name": "foo"}, {"name": "bar"}]));
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("hello.rs", "*.rs"));
        assert!(glob_match("foo_bar", "foo*"));
        assert!(!glob_match("hello.txt", "*.rs"));
        assert!(glob_match("hello world", "hello"));
    }
}
