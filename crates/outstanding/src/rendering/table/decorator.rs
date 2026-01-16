//! Table decorator for adding borders, headers, and formatting.
//!
//! This module provides a `Table` type that wraps a `TabularSpec` and adds
//! decorative elements like borders, headers, and separators.
//!
//! # Example
//!
//! ```rust
//! use outstanding::table::{Table, TabularSpec, Col, BorderStyle};
//!
//! let spec = TabularSpec::builder()
//!     .column(Col::fixed(20))
//!     .column(Col::fixed(10))
//!     .column(Col::fixed(8))
//!     .separator("  ")
//!     .build();
//!
//! let table = Table::new(spec, 80)
//!     .border(BorderStyle::Light)
//!     .header(vec!["Name", "Status", "Count"]);
//!
//! // Render header
//! println!("{}", table.header_row());
//! println!("{}", table.separator_row());
//!
//! // Render data rows
//! println!("{}", table.row(&["Alice", "Active", "42"]));
//! println!("{}", table.row(&["Bob", "Pending", "17"]));
//!
//! // Or render everything at once
//! let data = vec![
//!     vec!["Alice", "Active", "42"],
//!     vec!["Bob", "Pending", "17"],
//! ];
//! println!("{}", table.render(&data));
//! ```

use super::formatter::TableFormatter;
use super::types::{FlatDataSpec, TabularSpec};
use super::util::display_width;

/// Border style for table decoration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderStyle {
    /// No borders.
    #[default]
    None,
    /// ASCII borders: +, -, |
    Ascii,
    /// Light Unicode box-drawing characters: ┌, ─, ┐, │, └, ┘, ├, ┼, ┤, ┬, ┴
    Light,
    /// Heavy Unicode box-drawing characters: ┏, ━, ┓, ┃, ┗, ┛, ┣, ╋, ┫, ┳, ┻
    Heavy,
    /// Double-line Unicode box-drawing: ╔, ═, ╗, ║, ╚, ╝, ╠, ╬, ╣, ╦, ╩
    Double,
    /// Rounded corners with light lines: ╭, ─, ╮, │, ╰, ╯, ├, ┼, ┤, ┬, ┴
    Rounded,
}

impl BorderStyle {
    /// Get the box-drawing characters for this border style.
    ///
    /// Returns a tuple of (horizontal, vertical, top_left, top_right, bottom_left,
    /// bottom_right, left_t, cross, right_t, top_t, bottom_t).
    fn chars(&self) -> BorderChars {
        match self {
            BorderStyle::None => BorderChars::empty(),
            BorderStyle::Ascii => BorderChars {
                horizontal: '-',
                vertical: '|',
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                left_t: '+',
                cross: '+',
                right_t: '+',
                top_t: '+',
                bottom_t: '+',
            },
            BorderStyle::Light => BorderChars {
                horizontal: '─',
                vertical: '│',
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                left_t: '├',
                cross: '┼',
                right_t: '┤',
                top_t: '┬',
                bottom_t: '┴',
            },
            BorderStyle::Heavy => BorderChars {
                horizontal: '━',
                vertical: '┃',
                top_left: '┏',
                top_right: '┓',
                bottom_left: '┗',
                bottom_right: '┛',
                left_t: '┣',
                cross: '╋',
                right_t: '┫',
                top_t: '┳',
                bottom_t: '┻',
            },
            BorderStyle::Double => BorderChars {
                horizontal: '═',
                vertical: '║',
                top_left: '╔',
                top_right: '╗',
                bottom_left: '╚',
                bottom_right: '╝',
                left_t: '╠',
                cross: '╬',
                right_t: '╣',
                top_t: '╦',
                bottom_t: '╩',
            },
            BorderStyle::Rounded => BorderChars {
                horizontal: '─',
                vertical: '│',
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                left_t: '├',
                cross: '┼',
                right_t: '┤',
                top_t: '┬',
                bottom_t: '┴',
            },
        }
    }
}

/// Box-drawing characters for a border style.
#[derive(Clone, Copy, Debug)]
struct BorderChars {
    horizontal: char,
    vertical: char,
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    left_t: char,
    cross: char,
    right_t: char,
    top_t: char,
    bottom_t: char,
}

impl BorderChars {
    fn empty() -> Self {
        BorderChars {
            horizontal: ' ',
            vertical: ' ',
            top_left: ' ',
            top_right: ' ',
            bottom_left: ' ',
            bottom_right: ' ',
            left_t: ' ',
            cross: ' ',
            right_t: ' ',
            top_t: ' ',
            bottom_t: ' ',
        }
    }
}

/// A decorated table with borders, headers, and separators.
#[derive(Clone, Debug)]
pub struct Table {
    /// The underlying formatter.
    formatter: TableFormatter,
    /// Column headers.
    headers: Option<Vec<String>>,
    /// Border style.
    border: BorderStyle,
    /// Header style name (for styling header cells).
    header_style: Option<String>,
}

impl Table {
    /// Create a new table with the given spec and total width.
    pub fn new(spec: TabularSpec, total_width: usize) -> Self {
        let formatter = TableFormatter::new(&spec, total_width);
        Table {
            formatter,
            headers: None,
            border: BorderStyle::None,
            header_style: None,
        }
    }

    /// Create a table from a raw FlatDataSpec.
    pub fn from_spec(spec: &FlatDataSpec, total_width: usize) -> Self {
        let formatter = TableFormatter::new(spec, total_width);
        Table {
            formatter,
            headers: None,
            border: BorderStyle::None,
            header_style: None,
        }
    }

    /// Set the border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set the column headers.
    pub fn header<S: Into<String>, I: IntoIterator<Item = S>>(mut self, headers: I) -> Self {
        self.headers = Some(headers.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Set the header style name.
    pub fn header_style(mut self, style: impl Into<String>) -> Self {
        self.header_style = Some(style.into());
        self
    }

    /// Get the border style.
    pub fn get_border(&self) -> BorderStyle {
        self.border
    }

    /// Get the number of columns.
    pub fn num_columns(&self) -> usize {
        self.formatter.num_columns()
    }

    /// Format a data row.
    pub fn row<S: AsRef<str>>(&self, values: &[S]) -> String {
        let content = self.formatter.format_row(values);
        self.wrap_row(&content)
    }

    /// Format the header row.
    pub fn header_row(&self) -> String {
        match &self.headers {
            Some(headers) => {
                // Format the headers first (handles truncation/padding)
                let content = self.formatter.format_row(headers);

                // Apply style after formatting to avoid style tags being truncated
                let styled_content = if let Some(style) = &self.header_style {
                    format!("[{}]{}[/{}]", style, content, style)
                } else {
                    content
                };

                self.wrap_row(&styled_content)
            }
            None => String::new(),
        }
    }

    /// Generate a horizontal separator row.
    pub fn separator_row(&self) -> String {
        self.horizontal_line(LineType::Middle)
    }

    /// Generate the top border row.
    pub fn top_border(&self) -> String {
        self.horizontal_line(LineType::Top)
    }

    /// Generate the bottom border row.
    pub fn bottom_border(&self) -> String {
        self.horizontal_line(LineType::Bottom)
    }

    /// Wrap a row content with vertical borders.
    fn wrap_row(&self, content: &str) -> String {
        if self.border == BorderStyle::None {
            return content.to_string();
        }

        let chars = self.border.chars();
        format!("{}{}{}", chars.vertical, content, chars.vertical)
    }

    /// Generate a horizontal line (top, middle, or bottom).
    fn horizontal_line(&self, line_type: LineType) -> String {
        if self.border == BorderStyle::None {
            return String::new();
        }

        let chars = self.border.chars();
        let widths = self.formatter.widths();

        // Calculate total content width
        let content_width: usize = widths.iter().sum();
        let sep_width = display_width(&self.formatter_separator());
        let num_seps = widths.len().saturating_sub(1);
        let total_content = content_width + (num_seps * sep_width);

        let (left, _joint, right) = match line_type {
            LineType::Top => (chars.top_left, chars.top_t, chars.top_right),
            LineType::Middle => (chars.left_t, chars.cross, chars.right_t),
            LineType::Bottom => (chars.bottom_left, chars.bottom_t, chars.bottom_right),
        };

        let mut line = String::new();
        line.push(left);

        for (i, &width) in widths.iter().enumerate() {
            if i > 0 {
                // Add joint for separator
                for _ in 0..sep_width {
                    line.push(chars.horizontal);
                }
                // The joint replaces the middle horizontal char
                // Actually, for proper box drawing, we need joint at column boundaries
            }
            for _ in 0..width {
                line.push(chars.horizontal);
            }
        }

        // Add separators between columns
        // For simplicity, we'll just draw a continuous line
        // A more sophisticated version would place joints at column boundaries
        line = format!(
            "{}{}{}",
            left,
            std::iter::repeat_n(chars.horizontal, total_content)
                .collect::<String>(),
            right
        );

        line
    }

    /// Get the separator string from formatter.
    fn formatter_separator(&self) -> String {
        // Access separator through the Object trait
        use minijinja::value::{Object, Value};
        use std::sync::Arc;
        let arc_formatter = Arc::new(self.formatter.clone());
        arc_formatter
            .get_value(&Value::from("separator"))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    /// Render the complete table with all rows.
    ///
    /// Includes top border, header (if set), separator, data rows, and bottom border.
    pub fn render<S: AsRef<str>>(&self, rows: &[Vec<S>]) -> String {
        let mut output = Vec::new();

        // Top border
        let top = self.top_border();
        if !top.is_empty() {
            output.push(top);
        }

        // Header
        let header = self.header_row();
        if !header.is_empty() {
            output.push(header);

            // Separator after header
            let sep = self.separator_row();
            if !sep.is_empty() {
                output.push(sep);
            }
        }

        // Data rows
        for row in rows {
            output.push(self.row(row));
        }

        // Bottom border
        let bottom = self.bottom_border();
        if !bottom.is_empty() {
            output.push(bottom);
        }

        output.join("\n")
    }
}

/// Type of horizontal line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineType {
    Top,
    Middle,
    Bottom,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Col;

    fn simple_spec() -> TabularSpec {
        TabularSpec::builder()
            .column(Col::fixed(10))
            .column(Col::fixed(8))
            .separator("  ")
            .build()
    }

    #[test]
    fn table_no_border() {
        let table = Table::new(simple_spec(), 80);
        let row = table.row(&["Hello", "World"]);
        // No border, just formatted content
        assert!(!row.contains('│'));
        assert!(row.contains("Hello"));
    }

    #[test]
    fn table_with_ascii_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Ascii);
        let row = table.row(&["Hello", "World"]);
        // Should have vertical bars
        assert!(row.starts_with('|'));
        assert!(row.ends_with('|'));
    }

    #[test]
    fn table_with_light_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Light);
        let row = table.row(&["Hello", "World"]);
        // Should have light box characters
        assert!(row.starts_with('│'));
        assert!(row.ends_with('│'));
    }

    #[test]
    fn table_with_heavy_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Heavy);
        let row = table.row(&["Hello", "World"]);
        assert!(row.starts_with('┃'));
        assert!(row.ends_with('┃'));
    }

    #[test]
    fn table_with_double_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Double);
        let row = table.row(&["Hello", "World"]);
        assert!(row.starts_with('║'));
        assert!(row.ends_with('║'));
    }

    #[test]
    fn table_with_rounded_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Rounded);
        let row = table.row(&["Hello", "World"]);
        assert!(row.starts_with('│'));
        assert!(row.ends_with('│'));
    }

    #[test]
    fn table_header_row() {
        let table = Table::new(simple_spec(), 80)
            .border(BorderStyle::Light)
            .header(vec!["Name", "Status"]);

        let header = table.header_row();
        assert!(header.contains("Name"));
        assert!(header.contains("Status"));
        assert!(header.starts_with('│'));
    }

    #[test]
    fn table_header_with_style() {
        let table = Table::new(simple_spec(), 80)
            .header(vec!["Name", "Status"])
            .header_style("header");

        let header = table.header_row();
        assert!(header.contains("[header]"));
        assert!(header.contains("[/header]"));
    }

    #[test]
    fn table_no_header() {
        let table = Table::new(simple_spec(), 80);
        let header = table.header_row();
        assert!(header.is_empty());
    }

    #[test]
    fn table_separator_row() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Light);
        let sep = table.separator_row();
        assert!(sep.contains('─'));
        assert!(sep.starts_with('├'));
        assert!(sep.ends_with('┤'));
    }

    #[test]
    fn table_top_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Light);
        let top = table.top_border();
        assert!(top.contains('─'));
        assert!(top.starts_with('┌'));
        assert!(top.ends_with('┐'));
    }

    #[test]
    fn table_bottom_border() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Light);
        let bottom = table.bottom_border();
        assert!(bottom.contains('─'));
        assert!(bottom.starts_with('└'));
        assert!(bottom.ends_with('┘'));
    }

    #[test]
    fn table_render_full() {
        let table = Table::new(simple_spec(), 80)
            .border(BorderStyle::Light)
            .header(vec!["Name", "Value"]);

        let data = vec![vec!["Alice", "100"], vec!["Bob", "200"]];

        let output = table.render(&data);
        let lines: Vec<&str> = output.lines().collect();

        // Should have: top border, header, separator, 2 data rows, bottom border
        assert!(lines.len() >= 5);

        // Top border
        assert!(lines[0].starts_with('┌'));
        // Header
        assert!(lines[1].contains("Name"));
        // Separator
        assert!(lines[2].starts_with('├'));
        // Data rows
        assert!(lines[3].contains("Alice"));
        assert!(lines[4].contains("Bob"));
        // Bottom border
        assert!(lines[5].starts_with('└'));
    }

    #[test]
    fn table_render_no_border() {
        let table = Table::new(simple_spec(), 80).header(vec!["Name", "Value"]);

        let data = vec![vec!["Alice", "100"]];

        let output = table.render(&data);
        let lines: Vec<&str> = output.lines().collect();

        // No borders, just header and data
        assert!(lines.len() >= 2);
        assert!(lines[0].contains("Name"));
        assert!(lines[1].contains("Alice"));
    }

    #[test]
    fn border_style_default() {
        assert_eq!(BorderStyle::default(), BorderStyle::None);
    }

    #[test]
    fn table_accessors() {
        let table = Table::new(simple_spec(), 80).border(BorderStyle::Ascii);

        assert_eq!(table.get_border(), BorderStyle::Ascii);
        assert_eq!(table.num_columns(), 2);
    }
}
