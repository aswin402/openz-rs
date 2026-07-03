//! XLSX handler — reads Excel workbooks into IR using calamine.
//!
//! Each worksheet becomes a `Section` + `Table` in the IR.
//! The first row of each sheet is treated as the table header.

use crate::ir::{Document, Section, Table};
use calamine::{open_workbook, Reader, Xlsx, Data};
use std::fs::File;
use std::io::BufReader;

/// Load an XLSX file into the Internal Representation
pub fn to_ir(file_path: &str) -> Result<Document, String> {
    let mut workbook: Xlsx<BufReader<File>> = open_workbook(file_path)
        .map_err(|e| format!("Failed to open XLSX: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut doc = Document::new("xlsx");
    doc.path = Some(file_path.to_string());

    for sheet_name in &sheet_names {
        let range = workbook.worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{sheet_name}': {e}"))?;

        let rows: Vec<Vec<String>> = range.rows()
            .map(|row| {
                row.iter()
                    .map(cell_to_string)
                    .collect()
            })
            .collect();

        if rows.is_empty() {
            continue;
        }

        // First row as headers
        let headers = rows[0].clone();
        let data_rows = rows[1..].to_vec();

        let table = Table {
            headers,
            rows: data_rows,
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

    // Build a text representation for the full document
    let mut text_parts: Vec<String> = Vec::new();
    for (i, sheet_name) in sheet_names.iter().enumerate() {
        text_parts.push(format!("# Sheet: {sheet_name}"));

        if let Some(table) = doc.tables.get(i) {
            // Header row
            if !table.headers.is_empty() {
                let rendered: Vec<&str> = table.headers.iter().map(|s| s.as_str()).collect();
                text_parts.push(format!("  {} |", rendered.join(" | ")));
                text_parts.push(format!("  {} |", vec!["---"; table.headers.len()].join(" | ")));
            }
            // Data rows
            for row in &table.rows {
                let rendered: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
                text_parts.push(format!("  {} |", rendered.join(" | ")));
            }
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
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(_) => cell.to_string(),
        Data::DateTimeIso(_) => cell.to_string(),
        Data::DurationIso(_) => cell.to_string(),
        Data::Error(e) => format!("#ERROR:{e}"),
        _ => String::new(),
    }
}

/// Create an XLSX workbook from headers and row data
pub fn create_xlsx(
    file_path: &str,
    sheets: &[XlsxSheet],
) -> String {
    use rust_xlsxwriter::*;
    
    let mut workbook = Workbook::new();
    
    for sheet_input in sheets {
        let sheet = workbook.add_worksheet();
        if let Some(name) = &sheet_input.name {
            sheet.set_name(name).unwrap();
        }
        
        // Write headers
        for (col, header) in sheet_input.headers.iter().enumerate() {
            sheet.write_string(0, col as u16, header).unwrap();
        }
        
        // Write data rows
        for (row_idx, row) in sheet_input.data.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                // Try number first, fall back to string
                if let Ok(num) = cell.parse::<f64>() {
                    sheet.write_number(row_idx as u32 + 1, col_idx as u16, num).unwrap();
                } else {
                    sheet.write_string(row_idx as u32 + 1, col_idx as u16, cell).unwrap();
                }
            }
        }
    }
    
    match workbook.save(file_path) {
        Ok(_) => serde_json::json!({
            "success": true,
            "path": file_path,
            "format": "xlsx",
            "sheets": sheets.len(),
        }).to_string(),
        Err(e) => serde_json::json!({"error": format!("Failed to save XLSX: {e}")}).to_string(),
    }
}

/// Export an IR Document to XLSX
pub fn from_ir(doc: &crate::ir::Document, file_path: &str) -> Result<(), String> {
    use rust_xlsxwriter::*;
    
    let mut workbook = Workbook::new();
    
    if !doc.tables.is_empty() {
        for (t_idx, table) in doc.tables.iter().enumerate() {
            let sheet_name = table.caption.clone().unwrap_or_else(|| format!("Sheet{}", t_idx + 1));
            let sheet = workbook.add_worksheet();
            sheet.set_name(&sheet_name).map_err(|e| e.to_string())?;
            
            // Headers
            for (col, header) in table.headers.iter().enumerate() {
                sheet.write_string(0, col as u16, header).map_err(|e| e.to_string())?;
            }
            
            // Data rows
            for (row_idx, row) in table.rows.iter().enumerate() {
                for (col_idx, cell) in row.iter().enumerate() {
                    if let Ok(num) = cell.parse::<f64>() {
                        sheet.write_number(row_idx as u32 + 1, col_idx as u16, num).map_err(|e| e.to_string())?;
                    } else {
                        sheet.write_string(row_idx as u32 + 1, col_idx as u16, cell).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    } else if !doc.sections.is_empty() {
        for section in doc.sections.iter() {
            let sheet = workbook.add_worksheet();
            sheet.set_name(&section.title).map_err(|e| e.to_string())?;
            sheet.write_string(0, 0, &section.title).map_err(|e| e.to_string())?;
        }
    } else {
        let sheet = workbook.add_worksheet();
        sheet.set_name("Sheet1").map_err(|e| e.to_string())?;
        if let Some(ref text) = doc.text {
            for (i, line) in text.lines().enumerate() {
                sheet.write_string(i as u32, 0, line).map_err(|e| e.to_string())?;
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
    pub headers: Vec<String>,
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
    let mut workbook: Xlsx<BufReader<File>> = open_workbook(&request.file_path)
        .map_err(|e| format!("Failed to open XLSX for editing: {e}"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets_data = Vec::new();

    // 2. Read each existing sheet's grid of cells
    for sheet_name in &sheet_names {
        let range = workbook.worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{sheet_name}': {e}"))?;

        let (row_count, col_count) = range.get_size();
        let mut grid = vec![vec![String::new(); col_count]; row_count];

        for r in 0..row_count {
            for c in 0..col_count {
                if let Some(val) = range.get_value((r as u32, c as u32)) {
                    grid[r][c] = cell_to_string(val);
                }
            }
        }
        sheets_data.push((sheet_name.clone(), grid));
    }

    // 3. Close the workbook reader to release file handle
    drop(workbook);

    // 4. Apply any added sheets
    if let Some(ref new_sheets) = request.add_sheets {
        for name in new_sheets {
            if !sheets_data.iter().any(|(s, _)| s == name) {
                sheets_data.push((name.clone(), vec![vec![String::new(); 1]; 1]));
            }
        }
    }

    // 5. Apply cell updates
    if let Some(ref updates) = request.cell_updates {
        for update in updates {
            // Find sheet or create it if not exists
            let sheet_idx = match sheets_data.iter().position(|(s, _)| s == &update.sheet_name) {
                Some(idx) => idx,
                None => {
                    sheets_data.push((update.sheet_name.clone(), vec![vec![String::new(); 1]; 1]));
                    sheets_data.len() - 1
                }
            };

            let grid = &mut sheets_data[sheet_idx].1;
            
            // Resize grid if row or col is out of bounds
            let target_rows = (update.row + 1) as usize;
            let target_cols = (update.col + 1) as usize;

            if target_rows > grid.len() {
                let current_cols = if grid.is_empty() { 1 } else { grid[0].len() };
                grid.resize(target_rows, vec![String::new(); current_cols]);
            }
            
            let current_cols = grid[0].len();
            if target_cols > current_cols {
                for r in grid.iter_mut() {
                    r.resize(target_cols, String::new());
                }
            }

            grid[update.row as usize][update.col as usize] = update.value.clone();
        }
    }

    // 6. Save/Write everything back using rust_xlsxwriter
    let mut out_workbook = rust_xlsxwriter::Workbook::new();
    for (sheet_name, grid) in sheets_data {
        let sheet = out_workbook.add_worksheet();
        let _ = sheet.set_name(&sheet_name);

        for (r, row_vec) in grid.iter().enumerate() {
            for (c, cell_val) in row_vec.iter().enumerate() {
                if cell_val.is_empty() {
                    continue;
                }
                
                // Write formulas, numbers, booleans, or strings
                if cell_val.starts_with('=') {
                    let _ = sheet.write_formula(r as u32, c as u16, cell_val.as_str());
                } else if let Ok(num) = cell_val.parse::<f64>() {
                    let _ = sheet.write_number(r as u32, c as u16, num);
                } else if let Ok(b) = cell_val.parse::<bool>() {
                    let _ = sheet.write_boolean(r as u32, c as u16, b);
                } else {
                    let _ = sheet.write_string(r as u32, c as u16, cell_val);
                }
            }
        }
    }

    out_workbook.save(&request.file_path)
        .map_err(|e| format!("Failed to save edited XLSX: {e}"))?;

    Ok(serde_json::json!({
        "success": true,
        "path": request.file_path,
        "format": "xlsx",
    }).to_string())
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
