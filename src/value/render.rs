use super::{Table, Value};
use nu_ansi_term::Color;
use std::collections::HashMap;

/// Table renderer with automatic column width adjustment and color support
pub struct TableRenderer {
    use_colors: bool,
    max_width: Option<usize>,
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

        Self {
            use_colors,
            max_width,
        }
    }

    pub fn with_colors(mut self, enabled: bool) -> Self {
        self.use_colors = enabled;
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

        // Separator
        let separator = self.render_separator(&widths);
        output.push_str(&separator);
        output.push('\n');

        // Rows
        for row in &table.rows {
            let row_str = self.render_row(&table.columns, row, &widths);
            output.push_str(&row_str);
            output.push('\n');
        }

        output
    }

    fn calculate_column_widths(&self, table: &Table) -> Vec<usize> {
        table
            .columns
            .iter()
            .map(|col| {
                let header_len = col.len();
                let max_value_len = table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(col))
                    .map(|v| v.to_text().len())
                    .max()
                    .unwrap_or(0);
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
                    Color::Cyan.bold().paint(&padded).to_string()
                } else {
                    padded
                }
            })
            .collect();

        styled.join(" │ ")
    }

    fn render_separator(&self, widths: &[usize]) -> String {
        widths
            .iter()
            .map(|&w| "─".repeat(w))
            .collect::<Vec<_>>()
            .join("─┼─")
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
                let truncated = if value.len() > width {
                    format!("{}…", &value[..width.saturating_sub(1)])
                } else {
                    value
                };
                format!("{:width$}", truncated, width = width)
            })
            .collect();

        cells.join(" │ ")
    }
}

/// Render a value for terminal display
pub fn render_value(value: &Value) -> String {
    match value {
        Value::Table(table) => TableRenderer::new().render(table),
        Value::List(items) => items
            .iter()
            .map(render_value)
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Record(map) => {
            let columns: Vec<String> = map.keys().cloned().collect();
            let mut table = Table::new(columns);
            table.push_row(map.clone());
            TableRenderer::new().render(&table)
        }
        _ => value.to_text(),
    }
}
