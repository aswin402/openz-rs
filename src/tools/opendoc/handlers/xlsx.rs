//! XLSX handler — reads Excel workbooks into IR using calamine.
//!
//! Each worksheet becomes a `Section` + `Table` in the IR.
//! The first row of each sheet is treated as the table header.

use crate::tools::opendoc::ir::{Document, Section, Table};
use calamine::{Data, Reader};

/// Load an XLSX file into the Internal Representation
pub fn to_ir(file_path: &str) -> Result<Document, String> {
    let mut workbook = calamine::open_workbook_auto(file_path)
        .map_err(|e| format!("Failed to open spreadsheet: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut doc = Document::new("xlsx");
    doc.path = Some(file_path.to_string());

    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{sheet_name}': {e}"))?;

        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(cell_to_string).collect())
            .collect();

        match rows.split_first() {
            None => {}
            Some((first_row, rest_rows)) => {
                let table = Table {
                    headers: first_row.clone(),
                    rows: rest_rows.to_vec(),
                    caption: Some(sheet_name.clone()),
                };

                let section = Section {
                    title: sheet_name.clone(),
                    level: 0,
                    index: doc.tables.len(),
                    content: vec![],
                };

                doc.tables.push(table);
                doc.sections.push(section);
            }
        }
    }

    // Build a text representation for the full document
    let mut text_parts: Vec<String> = Vec::new();
    for (i, sheet_name) in sheet_names.iter().enumerate() {
        text_parts.push(format!("# Sheet: {sheet_name}"));

        match doc.tables.get(i) {
            Some(table) => {
                // Header row
                match table.headers.is_empty() {
                    false => {
                        let rendered: Vec<&str> =
                            table.headers.iter().map(|s| s.as_str()).collect();
                        text_parts.push(format!("  {} |", rendered.join(" | ")));
                        text_parts.push(format!(
                            "  {} |",
                            vec!["---"; table.headers.len()].join(" | ")
                        ));
                    }
                    true => {}
                }
                // Data rows
                for row in &table.rows {
                    let rendered: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
                    text_parts.push(format!("  {} |", rendered.join(" | ")));
                }
            }
            None => {}
        }
        text_parts.push(String::new());
    }
    doc.text = Some(text_parts.join("\n"));

    // Add page_count = sheet count as metadata
    doc.metadata.page_count = Some(sheet_names.len() as u32);

    Ok(doc)
}

/// Convert a calamine Data cell to a String
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.clone(),
        Data::Float(f) => match f.fract() == 0.0 {
            true => format!("{}", *f as i64),
            false => f.to_string(),
        },
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(_) => cell.to_string(),
        Data::DateTimeIso(_) => cell.to_string(),
        Data::DurationIso(_) => cell.to_string(),
        Data::Error(e) => format!("#ERROR:{e}"),
        _ => String::new(),
    }
}

fn write_cell_typed(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    cell: &str,
    support_formula_and_bool: bool,
) -> Result<(), rust_xlsxwriter::XlsxError> {
    match (support_formula_and_bool, cell.starts_with('=')) {
        (true, true) => sheet.write_formula(row, col, cell).map(|_| ()),
        _ => match cell.parse::<f64>() {
            Ok(num) => sheet.write_number(row, col, num).map(|_| ()),
            Err(_) => match (support_formula_and_bool, cell.parse::<bool>()) {
                (true, Ok(b)) => sheet.write_boolean(row, col, b).map(|_| ()),
                _ => sheet.write_string(row, col, cell).map(|_| ()),
            },
        },
    }
}

/// Create an XLSX workbook from headers and row data
pub fn create_xlsx(file_path: &str, sheets: &[XlsxSheet]) -> String {
    use rust_xlsxwriter::*;

    let mut workbook = Workbook::new();

    for sheet_input in sheets {
        let sheet = workbook.add_worksheet();
        match &sheet_input.name {
            Some(name) => {
                sheet.set_name(name).unwrap();
            }
            None => {}
        }

        // Write headers
        for (col, header) in sheet_input.headers.iter().enumerate() {
            sheet.write_string(0, col as u16, header).unwrap();
        }

        // Write data rows
        for (row_idx, row) in sheet_input.data.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                write_cell_typed(sheet, row_idx as u32 + 1, col_idx as u16, cell, false).unwrap();
            }
        }
    }

    match workbook.save(file_path) {
        Ok(_) => serde_json::json!({
            "success": true,
            "path": file_path,
            "format": "xlsx",
            "sheets": sheets.len(),
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": format!("Failed to save XLSX: {e}")}).to_string(),
    }
}

/// Export an IR Document to XLSX
pub fn from_ir(doc: &crate::tools::opendoc::ir::Document, file_path: &str) -> Result<(), String> {
    use rust_xlsxwriter::*;

    let mut workbook = Workbook::new();

    match (!doc.tables.is_empty(), !doc.sections.is_empty()) {
        (true, _) => {
            for (t_idx, table) in doc.tables.iter().enumerate() {
                let sheet_name = table
                    .caption
                    .clone()
                    .unwrap_or_else(|| format!("Sheet{}", t_idx + 1));
                let sheet = workbook.add_worksheet();
                sheet.set_name(&sheet_name).map_err(|e| e.to_string())?;

                // Headers
                for (col, header) in table.headers.iter().enumerate() {
                    sheet
                        .write_string(0, col as u16, header)
                        .map_err(|e| e.to_string())?;
                }

                // Data rows
                for (row_idx, row) in table.rows.iter().enumerate() {
                    for (col_idx, cell) in row.iter().enumerate() {
                        write_cell_typed(sheet, row_idx as u32 + 1, col_idx as u16, cell, false)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        (false, true) => {
            for section in doc.sections.iter() {
                let sheet = workbook.add_worksheet();
                sheet.set_name(&section.title).map_err(|e| e.to_string())?;
                sheet
                    .write_string(0, 0, &section.title)
                    .map_err(|e| e.to_string())?;
            }
        }
        (false, false) => {
            let sheet = workbook.add_worksheet();
            sheet.set_name("Sheet1").map_err(|e| e.to_string())?;
            match doc.text {
                Some(ref text) => {
                    for (i, line) in text.lines().enumerate() {
                        sheet
                            .write_string(i as u32, 0, line)
                            .map_err(|e| e.to_string())?;
                    }
                }
                None => {}
            }
        }
    }

    workbook.save(file_path).map_err(|e| e.to_string())?;
    Ok(())
}

/// A single worksheet specification for XLSX creation.
/// Contains optional sheet name, column headers, and data rows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct XlsxSheet {
    pub name: Option<String>,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default, alias = "rows")]
    pub data: Vec<Vec<String>>,
}

/// A cell update operation for modifying spreadsheets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct XlsxCellOperation {
    pub sheet_name: String,
    pub row: u32,
    pub col: u16,
    pub value: String,
}

/// Request payload for editing spreadsheets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct XlsxEditRequest {
    pub file_path: String,
    pub cell_updates: Option<Vec<XlsxCellOperation>>,
    pub add_sheets: Option<Vec<String>>,
}

/// Edit an existing XLSX file by applying sheet additions and cell updates
pub fn edit_xlsx(request: &XlsxEditRequest) -> Result<String, String> {
    // 1. Open the existing workbook to read all sheets
    let mut workbook = calamine::open_workbook_auto(&request.file_path)
        .map_err(|e| format!("Failed to open spreadsheet for editing: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets_data = Vec::new();

    // 2. Read each existing sheet's grid of cells
    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{sheet_name}': {e}"))?;

        let (row_count, col_count) = range.get_size();
        let mut grid = vec![vec![String::new(); col_count]; row_count];

        for (r, row) in grid.iter_mut().enumerate().take(row_count) {
            for (c, cell) in row.iter_mut().enumerate().take(col_count) {
                match range.get_value((r as u32, c as u32)) {
                    Some(val) => {
                        *cell = cell_to_string(val);
                    }
                    None => {}
                }
            }
        }
        sheets_data.push((sheet_name.clone(), grid));
    }

    // 3. Close the workbook reader to release file handle
    drop(workbook);

    // 4. Apply any added sheets
    match request.add_sheets {
        Some(ref new_sheets) => {
            for name in new_sheets {
                match sheets_data.iter().any(|(s, _)| s == name) {
                    false => {
                        sheets_data.push((name.clone(), vec![vec![String::new(); 1]; 1]));
                    }
                    true => {}
                }
            }
        }
        None => {}
    }

    // 5. Apply cell updates
    match request.cell_updates {
        Some(ref updates) => {
            for update in updates {
                // Find sheet or create it if not exists
                let sheet_idx = match sheets_data
                    .iter()
                    .position(|(s, _)| s == &update.sheet_name)
                {
                    Some(idx) => idx,
                    None => {
                        sheets_data.push((
                            update.sheet_name.clone(),
                            vec![vec![String::new(); 1]; 1],
                        ));
                        sheets_data.len() - 1
                    }
                };

                let grid = &mut sheets_data[sheet_idx].1;

                // Resize grid if row or col is out of bounds
                let target_rows = (update.row + 1) as usize;
                let target_cols = (update.col + 1) as usize;

                match target_rows > grid.len() {
                    true => {
                        let current_cols = grid.first().map(|r| r.len()).unwrap_or(1);
                        grid.resize(target_rows, vec![String::new(); current_cols]);
                    }
                    false => {}
                }

                let current_cols = grid[0].len();
                match target_cols > current_cols {
                    true => {
                        for r in grid.iter_mut() {
                            r.resize(target_cols, String::new());
                        }
                    }
                    false => {}
                }

                grid[update.row as usize][update.col as usize] = update.value.clone();
            }
        }
        None => {}
    }

    // 6. Save/Write everything back using rust_xlsxwriter
    let mut out_workbook = rust_xlsxwriter::Workbook::new();
    for (sheet_name, grid) in sheets_data {
        let sheet = out_workbook.add_worksheet();
        let _ = sheet.set_name(&sheet_name);

        for (r, row_vec) in grid.iter().enumerate() {
            for (c, cell_val) in row_vec.iter().enumerate() {
                match cell_val.is_empty() {
                    true => {}
                    false => {
                        let _ = write_cell_typed(sheet, r as u32, c as u16, cell_val, true);
                    }
                }
            }
        }
    }

    out_workbook
        .save(&request.file_path)
        .map_err(|e| format!("Failed to save edited XLSX: {e}"))?;

    Ok(serde_json::json!({
        "success": true,
        "path": request.file_path,
        "format": "xlsx",
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static XLSX_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_xlsx() -> (String, PathBuf) {
        let id = XLSX_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opendoc_xlsx_test_{id}.xlsx"));
        let path_str = path.to_str().unwrap().to_string();

        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.write_string(0, 0, "Name").unwrap();
        sheet.write_string(0, 1, "Age").unwrap();
        sheet.write_string(1, 0, "Alice").unwrap();
        sheet.write_number(1, 1, 30.0).unwrap();
        sheet.write_string(2, 0, "Bob").unwrap();
        sheet.write_number(2, 1, 25.0).unwrap();
        workbook.save(&path).unwrap();

        (path_str, path)
    }

    #[test]
    fn test_xlsx_to_ir_basic() {
        let (path_str, path) = unique_xlsx();

        let doc = to_ir(&path_str).unwrap();
        assert_eq!(doc.format, "xlsx");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.tables.len(), 1);

        let table = &doc.tables[0];
        assert_eq!(table.headers, vec!["Name", "Age"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][0], "Alice");
        assert_eq!(table.rows[1][1], "25");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_xlsx_text_representation() {
        let (path_str, path) = unique_xlsx();

        let doc = to_ir(&path_str).unwrap();
        let text = doc.text.unwrap_or_default();
        assert!(text.contains("Name"));
        assert!(text.contains("Alice"));
        assert!(text.contains("30"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_xlsx_file_not_found() {
        let result = to_ir("/nonexistent/path/file.xlsx");
        assert!(result.is_err());
    }

    #[test]
    fn test_edit_xlsx() {
        let (path_str, path) = unique_xlsx();

        let request = XlsxEditRequest {
            file_path: path_str.clone(),
            cell_updates: Some(vec![
                XlsxCellOperation {
                    sheet_name: "Sheet1".to_string(),
                    row: 1,
                    col: 1,
                    value: "31".to_string(),
                },
                XlsxCellOperation {
                    sheet_name: "Summary".to_string(),
                    row: 0,
                    col: 0,
                    value: "=Sheet1!B2".to_string(),
                },
            ]),
            add_sheets: Some(vec!["Summary".to_string()]),
        };

        let res = edit_xlsx(&request);
        assert!(res.is_ok());

        // Read it back
        let doc = to_ir(&path_str).unwrap();
        assert_eq!(doc.tables.len(), 2);

        // First sheet
        let table1 = &doc.tables[0];
        assert_eq!(table1.rows[0][1], "31");

        // Second sheet
        let table2 = &doc.tables[1];
        assert_eq!(table2.caption.as_deref(), Some("Summary"));

        let _ = std::fs::remove_file(&path);
    }
}
