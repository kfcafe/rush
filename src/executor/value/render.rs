use super::{Table, Value};
use nu_ansi_term::Color;
use std::collections::HashMap;

/// Parse a hex color (#rrggbb) from an env var into a nu_ansi_term Color.
fn parse_env_color(var: &str) -> Option<Color> {
    let val = std::env::var(var).ok()?;
    let hex = val.strip_prefix('#').unwrap_or(&val);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Table renderer with automatic column width adjustment and color support
pub struct TableRenderer {
    use_colors: bool,
    max_width: Option<usize>,
    style: TableStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum TableStyle {
    Unicode,
    Ascii,
    Minimal,
}

impl Default for TableRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TableRenderer {
    pub fn new() -> Self {
        // Detect if we're in a TTY for color support
        let use_colors = atty::is(atty::Stream::Stdout)
            && std::env::var("NO_COLOR").is_err()
            && std::env::var("RUSH_NO_COLOR").is_err();

        // Get terminal width
        let max_width = terminal_size::terminal_size().map(|(w, _)| w.0 as usize);

        // Get table style from environment
        let style = match std::env::var("RUSH_TABLE_STYLE").as_deref() {
            Ok("ascii") => TableStyle::Ascii,
            Ok("minimal") => TableStyle::Minimal,
            _ => TableStyle::Unicode,
        };

        Self {
            use_colors,
            max_width,
            style,
        }
    }

    pub fn with_colors(mut self, enabled: bool) -> Self {
        self.use_colors = enabled;
        self
    }

    pub fn with_style(mut self, style: TableStyle) -> Self {
        self.style = style;
        self
    }

    /// Render a table with automatic column width adjustment
    pub fn render(&self, table: &Table) -> String {
        if table.is_empty() {
            return String::new();
        }

        // Calculate column widths
        let widths = self.calculate_column_widths(table);

        let mut output = String::new();

        // Header
        let header = self.render_header(&table.columns, &widths);
        output.push_str(&header);
        output.push('\n');

        // Separator (if not minimal style)
        if !matches!(self.style, TableStyle::Minimal) {
            let separator = self.render_separator(&widths);
            output.push_str(&separator);
            output.push('\n');
        }

        // Rows
        for row in &table.rows {
            let row_str = self.render_row(&table.columns, row, &widths);
            output.push_str(&row_str);
            output.push('\n');
        }

        output
    }

    fn calculate_column_widths(&self, table: &Table) -> Vec<usize> {
        let _available_width = self.max_width.unwrap_or(120);

        table
            .columns
            .iter()
            .map(|col| {
                // Get header length
                let header_len = col.len();

                // Get max value length in this column
                let max_value_len = table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(col))
                    .map(|v| v.to_text().len())
                    .max()
                    .unwrap_or(0);

                // Take the max of header and value lengths
                // Cap at 50 chars per column to prevent super wide tables
                header_len.max(max_value_len).min(50)
            })
            .collect()
    }

    fn render_header(&self, columns: &[String], widths: &[usize]) -> String {
        let styled: Vec<String> = columns
            .iter()
            .zip(widths.iter())
            .map(|(col, &width)| {
                let padded = format!("{:width$}", col, width = width);
                if self.use_colors {
                    let color = parse_env_color("RUSH_COLOR_HEADER").unwrap_or(Color::Cyan);
                    color.bold().paint(&padded).to_string()
                } else {
                    padded
                }
            })
            .collect();

        match self.style {
            TableStyle::Unicode => styled.join(" │ "),
            TableStyle::Ascii => styled.join(" | "),
            TableStyle::Minimal => styled.join("  "),
        }
    }

    fn render_separator(&self, widths: &[usize]) -> String {
        match self.style {
            TableStyle::Unicode => widths
                .iter()
                .map(|&w| "─".repeat(w))
                .collect::<Vec<_>>()
                .join("─┼─"),
            TableStyle::Ascii => widths
                .iter()
                .map(|&w| "-".repeat(w))
                .collect::<Vec<_>>()
                .join("-+-"),
            TableStyle::Minimal => String::new(),
        }
    }

    fn render_row(
        &self,
        columns: &[String],
        row: &HashMap<String, Value>,
        widths: &[usize],
    ) -> String {
        let cells: Vec<String> = columns
            .iter()
            .zip(widths.iter())
            .map(|(col, &width)| {
                let value = row.get(col).map(|v| v.to_text()).unwrap_or_default();

                // Truncate if too long
                let truncated = if value.len() > width {
                    format!("{}…", &value[..width.saturating_sub(1)])
                } else {
                    value
                };

                format!("{:width$}", truncated, width = width)
            })
            .collect();

        match self.style {
            TableStyle::Unicode => cells.join(" │ "),
            TableStyle::Ascii => cells.join(" | "),
            TableStyle::Minimal => cells.join("  "),
        }
    }
}

/// Render a value for terminal display
/// This is the main entry point for displaying structured data
pub fn render_value(value: &Value) -> String {
    match value {
        Value::Table(table) => TableRenderer::new().render(table),
        Value::List(items) => {
            // Render each item on its own line
            items
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Record(map) => {
            // Convert record to single-row table for nice display
            let columns: Vec<String> = map.keys().cloned().collect();
            let mut table = Table::new(columns);
            table.push_row(map.clone());
            TableRenderer::new().render(&table)
        }
        // All other types just convert to text
        _ => value.to_text(),
    }
}

/// Render a value with explicit color control
pub fn render_value_with_colors(value: &Value, use_colors: bool) -> String {
    match value {
        Value::Table(table) => TableRenderer::new().with_colors(use_colors).render(table),
        Value::List(items) => items
            .iter()
            .map(|v| render_value_with_colors(v, use_colors))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Record(map) => {
            let columns: Vec<String> = map.keys().cloned().collect();
            let mut table = Table::new(columns);
            table.push_row(map.clone());
            TableRenderer::new().with_colors(use_colors).render(&table)
        }
        _ => value.to_text(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_render_simple() {
        let mut table = Table::new(vec!["name".into(), "value".into()]);

        let mut row = HashMap::new();
        row.insert("name".into(), Value::String("test".into()));
        row.insert("value".into(), Value::Int(42));
        table.push_row(row);

        let renderer = TableRenderer::new().with_colors(false);
        let output = renderer.render(&table);

        assert!(output.contains("name"));
        assert!(output.contains("value"));
        assert!(output.contains("test"));
        assert!(output.contains("42"));
    }

    #[test]
    fn test_table_render_empty() {
        let table = Table::new(vec!["a".into(), "b".into()]);
        let renderer = TableRenderer::new();
        let output = renderer.render(&table);
        assert_eq!(output, "");
    }

    #[test]
    fn test_render_value_primitives() {
        assert_eq!(render_value(&Value::String("hello".into())), "hello");
        assert_eq!(render_value(&Value::Int(123)), "123");
        assert_eq!(render_value(&Value::Bool(true)), "true");
    }

    #[test]
    fn test_render_value_list() {
        let list = Value::List(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]);
        let output = render_value(&list);
        assert_eq!(output, "a\nb\nc");
    }

    #[test]
    fn test_table_style_ascii() {
        let mut table = Table::new(vec!["a".into(), "b".into()]);
        let mut row = HashMap::new();
        row.insert("a".into(), Value::Int(1));
        row.insert("b".into(), Value::Int(2));
        table.push_row(row);

        let renderer = TableRenderer::new()
            .with_colors(false)
            .with_style(TableStyle::Ascii);
        let output = renderer.render(&table);

        // Should use ASCII characters
        assert!(output.contains("|"));
        assert!(output.contains("-"));
    }

    #[test]
    fn test_column_truncation() {
        let mut table = Table::new(vec!["long".into()]);
        let mut row = HashMap::new();
        // Create a very long value
        let long_value = "a".repeat(100);
        row.insert("long".into(), Value::String(long_value));
        table.push_row(row);

        let renderer = TableRenderer::new().with_colors(false);
        let output = renderer.render(&table);

        // Should be truncated (max 50 chars per column)
        assert!(output.contains("…"));
    }
}
