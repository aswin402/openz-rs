use serde::{Deserialize, Serialize};

/// A paragraph of text with optional formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub text: String,
    pub is_heading: bool,
    pub heading_level: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font_size: Option<f32>,
    pub font_name: Option<String>,
    pub color: Option<String>,
    pub alignment: Alignment,
    pub list_type: ListType,
}

impl Paragraph {
    /// Create a new paragraph with the given text content.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_heading: false,
            heading_level: 0,
            bold: false,
            italic: false,
            underline: false,
            font_size: None,
            font_name: None,
            color: None,
            alignment: Alignment::Left,
            list_type: ListType::None,
        }
    }

    /// Create a heading paragraph at the specified level (1-6).
    pub fn heading(text: impl Into<String>, level: u32) -> Self {
        Self {
            text: text.into(),
            is_heading: true,
            heading_level: level,
            ..Self::default()
        }
    }
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            text: String::new(),
            is_heading: false,
            heading_level: 0,
            bold: false,
            italic: false,
            underline: false,
            font_size: None,
            font_name: None,
            color: None,
            alignment: Alignment::Left,
            list_type: ListType::None,
        }
    }
}

/// A table with headers and rows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub caption: Option<String>,
}

impl Table {
    /// Create a table with the given headers and data rows.
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self {
            headers,
            rows,
            caption: None,
        }
    }

    /// Number of data rows in the table (excluding header).
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns (header count).
    pub fn col_count(&self) -> usize {
        self.headers.len()
    }

    /// Get the cell value at (row, col), where row is 0-indexed into data rows.
    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        self.rows.get(row)?.get(col).map(|s| s.as_str())
    }
}

/// An embedded image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub data_base64: Option<String>,
    pub path: Option<String>,
}

/// Text alignment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// List type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ListType {
    #[default]
    None,
    Bullet,
    Ordered,
}
