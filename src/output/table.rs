// Aligned table renderer for JSON arrays of objects.

use nu_ansi_term::{Color, Style};
use serde_json::Value;

const MIN_COL_WIDTH: usize = 3;
const MAX_COL_WIDTH: usize = 60;
const COL_GAP: usize = 2; // spaces between columns

/// Render a JSON value as an aligned table.
///
/// Returns `Some(table_string)` when the input is a non-empty JSON array whose
/// elements are objects.  Returns `None` for any other shape (pass-through to
/// the caller so it can fall back to raw JSON).
pub fn render_json_table(value: &Value) -> Option<String> {
    let rows = value.as_array()?;
    if rows.is_empty() {
        return Some(String::new());
    }

    // Collect headers — union of all keys, in insertion order of the first row.
    let mut headers: Vec<String> = Vec::new();
    for row in rows {
        if let Some(obj) = row.as_object() {
            for key in obj.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    if headers.is_empty() {
        return None; // array of scalars — not suitable for table display
    }

    let term_width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(120);

    // Compute natural column widths (header or widest value, capped).
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len().max(MIN_COL_WIDTH)).collect();

    for row in rows {
        if let Some(obj) = row.as_object() {
            for (i, header) in headers.iter().enumerate() {
                let cell = obj.get(header).map(value_to_cell).unwrap_or_default();
                col_widths[i] = col_widths[i].max(cell.len()).min(MAX_COL_WIDTH);
            }
        }
    }

    // Shrink columns if total width exceeds terminal width.
    shrink_columns(&mut col_widths, term_width);

    let mut out = String::new();

    // Header row (bold + underline).
    let header_style = Style::new().bold();
    let header_line: String = headers
        .iter()
        .zip(&col_widths)
        .enumerate()
        .map(|(i, (h, &w))| {
            let cell = truncate(h, w);
            let padded = format!("{:<width$}", cell, width = w);
            let styled = header_style.fg(Color::Cyan).paint(padded).to_string();
            if i + 1 < headers.len() {
                format!("{}{}", styled, " ".repeat(COL_GAP))
            } else {
                styled
            }
        })
        .collect();
    out.push_str(&header_line);
    out.push('\n');

    // Separator (dim dashes).
    let sep_style = Style::new().dimmed();
    let sep_line: String = col_widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let dashes = sep_style.paint("─".repeat(w)).to_string();
            if i + 1 < col_widths.len() {
                format!("{}{}", dashes, " ".repeat(COL_GAP))
            } else {
                dashes
            }
        })
        .collect();
    out.push_str(&sep_line);
    out.push('\n');

    // Data rows.
    for (row_idx, row) in rows.iter().enumerate() {
        // Alternate row shading: every even data row gets a slight dim.
        let row_style = if row_idx % 2 == 1 {
            Style::new().dimmed()
        } else {
            Style::new()
        };

        let obj = row.as_object();
        let row_line: String = headers
            .iter()
            .zip(&col_widths)
            .enumerate()
            .map(|(i, (header, &w))| {
                let raw = obj
                    .and_then(|o| o.get(header))
                    .map(value_to_cell)
                    .unwrap_or_else(|| "-".to_string());
                let cell = truncate(&raw, w);
                let padded = format!("{:<width$}", cell, width = w);
                let styled = row_style.paint(padded).to_string();
                if i + 1 < headers.len() {
                    format!("{}{}", styled, " ".repeat(COL_GAP))
                } else {
                    styled
                }
            })
            .collect();
        out.push_str(&row_line);
        out.push('\n');
    }

    Some(out)
}

/// Convert a JSON scalar value to a display string.
fn value_to_cell(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "-".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(_) | Value::Object(_) => {
            // Compact JSON for nested structures.
            serde_json::to_string(v).unwrap_or_else(|_| "…".to_string())
        }
    }
}

/// Truncate a string to `max` characters, appending `…` if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        "…".to_string()
    } else {
        // "…" is 3 bytes in UTF-8, so we need to leave room for it
        format!("{}…", &s[..max - 3])
    }
}

/// Proportionally shrink column widths so the total fits within `term_width`.
fn shrink_columns(widths: &mut [usize], term_width: usize) {
    let gaps = if widths.len() > 1 {
        (widths.len() - 1) * COL_GAP
    } else {
        0
    };
    let total: usize = widths.iter().sum::<usize>() + gaps;
    if total <= term_width {
        return;
    }

    let available = term_width.saturating_sub(gaps);
    let n = widths.len();

    // Iteratively shrink the widest columns.
    for _ in 0..n {
        let current_total: usize = widths.iter().sum();
        if current_total <= available {
            break;
        }
        let max_w = *widths.iter().max().unwrap_or(&0);
        let excess = current_total - available;
        for w in widths.iter_mut() {
            if *w == max_w {
                *w = (*w).saturating_sub(excess / n + 1).max(MIN_COL_WIDTH);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_simple_array() {
        let data = json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob",   "age": 25}
        ]);
        let out = render_json_table(&data).expect("should render");
        assert!(out.contains("name"), "header 'name' present");
        assert!(out.contains("age"), "header 'age' present");
        assert!(out.contains("Alice"));
        assert!(out.contains("Bob"));
        assert!(out.contains("30"));
        assert!(out.contains("25"));
    }

    #[test]
    fn test_render_empty_array() {
        let data = json!([]);
        let out = render_json_table(&data).expect("empty array yields empty string");
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_non_array_returns_none() {
        let data = json!({"key": "value"});
        assert!(render_json_table(&data).is_none());
    }

    #[test]
    fn test_render_scalar_array_returns_none() {
        // Array of scalars has no column headers — should be None.
        let data = json!(["a", "b", "c"]);
        assert!(render_json_table(&data).is_none());
    }

    #[test]
    fn test_render_missing_fields_shown_as_dash() {
        let data = json!([
            {"name": "Alice", "role": "admin"},
            {"name": "Bob"}
        ]);
        let out = render_json_table(&data).expect("should render");
        assert!(out.contains('-'), "missing field rendered as dash");
    }

    #[test]
    fn test_render_null_shown_as_dash() {
        let data = json!([{"name": "Alice", "role": null}]);
        let out = render_json_table(&data).expect("should render");
        assert!(out.contains('-'), "null rendered as dash");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("hello world", 7);
        assert_eq!(result.len(), 7);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_value_to_cell_types() {
        assert_eq!(value_to_cell(&json!("hello")), "hello");
        assert_eq!(value_to_cell(&json!(42)), "42");
        assert_eq!(value_to_cell(&json!(true)), "true");
        assert_eq!(value_to_cell(&json!(null)), "-");
    }

    #[test]
    fn test_render_json_table_row_count() {
        let data = json!([
            {"x": 1},
            {"x": 2},
            {"x": 3}
        ]);
        let out = render_json_table(&data).expect("should render");
        // header + separator + 3 data rows = 5 lines
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 5);
    }
}
