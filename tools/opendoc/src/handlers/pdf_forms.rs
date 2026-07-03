//! PDF form field (AcroForm) listing and filling.
//!
//! Uses lopdf to read/write AcroForm dictionaries directly.
//! Supports AcroForm fields (TextField, CheckBox, RadioButton, ComboBox, ListBox, Signature).

use lopdf::{Document, Object, ObjectId};
use std::collections::HashMap;

/// A single PDF form field
#[derive(Debug, Clone, serde::Serialize)]
pub struct FormField {
    pub name: String,           // Fully qualified field name
    pub partial_name: String,   // /T of this field
    pub field_type: String,     // Tx, Btn, Ch, Sig
    pub field_type_name: String, // Text, Button, Choice, Signature
    pub value: Option<String>,
    pub default_value: Option<String>,
    pub page: Option<u32>,
    pub is_readonly: bool,
    pub is_required: bool,
    pub options: Vec<String>,   // For Choice fields
}

/// List all form fields in a PDF
pub fn list_form_fields(file_path: &str) -> Result<Vec<FormField>, String> {
    let doc = Document::load(file_path)
        .map_err(|e| format!("Failed to load PDF: {e}"))?;
    get_acroform_fields(&doc)
}

/// Fill form fields in a PDF with given values
pub fn fill_form_fields(file_path: &str, values: &[(String, String)]) -> Result<usize, String> {
    let mut doc = Document::load(file_path)
        .map_err(|e| format!("Failed to load PDF: {e}"))?;

    let mut filled_count = 0;

    // Get AcroForm dictionary
    let acroform_id = get_acroform_id(&doc)?;
    let acroform = doc.get_dictionary(acroform_id)
        .map_err(|e| format!("Failed to get AcroForm dict: {e}"))?;

    let fields_array = acroform.get(b"Fields")
        .map_err(|_| "No Fields array in AcroForm".to_string())?
        .as_array()
        .map_err(|e| format!("Fields is not an array: {e}"))?;

    // Build field path -> object id map
    let field_map = build_field_map(&doc, fields_array, "")?;

    for (field_name, value) in values {
        if let Some(&obj_id) = field_map.get(field_name) {
            set_field_value(&mut doc, obj_id, value)?;
            filled_count += 1;
        }
    }

    doc.save(file_path)
        .map_err(|e| format!("Failed to save PDF: {e}"))?;

    Ok(filled_count)
}

/// Get the AcroForm dictionary ObjectId from the catalog
fn get_acroform_id(doc: &Document) -> Result<ObjectId, String> {
    let catalog = doc.catalog()
        .map_err(|e| format!("Failed to get catalog: {e}"))?;

    let acroform_obj = catalog.get(b"AcroForm")
        .map_err(|_| "No AcroForm dictionary found".to_string())?;

    match acroform_obj {
        Object::Reference(id) => Ok(*id),
        _ => Err("AcroForm is not a reference".to_string()),
    }
}

/// Build a map of fully qualified field name -> ObjectId
fn build_field_map(
    doc: &Document,
    fields_array: &[Object],
    parent_name: &str,
) -> Result<HashMap<String, ObjectId>, String> {
    let mut map = HashMap::new();

    for field_ref in fields_array {
        let obj_id = field_ref.as_reference()
            .map_err(|_| "Expected reference in Fields array".to_string())?;

        let field_dict = doc.get_dictionary(obj_id)
            .map_err(|e| format!("Field dict not found: {e}"))?;

        let partial_name = field_dict.get(b"T").ok()
            .and_then(|o| o.as_string().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        // Build full name
        let full_name = if parent_name.is_empty() {
            partial_name.clone()
        } else {
            format!("{}.{}", parent_name, partial_name)
        };

        // Check for child fields
        if let Ok(kids) = field_dict.get(b"Kids").and_then(|o| o.as_array()) {
            let child_map = build_field_map(doc, kids, &full_name)?;
            map.extend(child_map);
        }

        // Only add leaf fields (or fields with no kids)
        if field_dict.get(b"Kids").is_err() {
            map.insert(full_name, obj_id);
        } else if field_dict.get(b"Kids").is_ok() {
            // Terminal field with kids? add it anyway
            map.insert(full_name, obj_id);
        }
    }

    Ok(map)
}

/// Get all form fields as structured data
fn get_acroform_fields(doc: &Document) -> Result<Vec<FormField>, String> {
    let acroform_id = get_acroform_id(doc)?;
    let acroform = doc.get_dictionary(acroform_id)
        .map_err(|e| format!("Failed to get AcroForm dict: {e}"))?;

    let fields_array = acroform.get(b"Fields")
        .map_err(|_| "No Fields array in AcroForm".to_string())?
        .as_array()
        .map_err(|e| format!("Fields is not an array: {e}"))?;

    let mut fields = Vec::new();
    collect_fields(doc, fields_array, "", &mut fields)?;
    Ok(fields)
}

/// Recursively collect all form fields
fn collect_fields(
    doc: &Document,
    fields_array: &[Object],
    parent_name: &str,
    result: &mut Vec<FormField>,
) -> Result<(), String> {
    for field_ref in fields_array {
        let obj_id = field_ref.as_reference()
            .map_err(|_| "Expected reference".to_string())?;

        let field_dict = doc.get_dictionary(obj_id)
            .map_err(|e| format!("Field not found: {e}"))?;

        let partial_name = field_dict.get(b"T").ok()
            .and_then(|o| o.as_string().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let full_name = if parent_name.is_empty() {
            partial_name.clone()
        } else {
            format!("{}.{}", parent_name, partial_name)
        };

        // Get field type
        let field_type = field_dict.get(b"FT").ok()
            .and_then(|o| o.as_name().ok())
            .and_then(|n| std::str::from_utf8(n).ok())
            .unwrap_or("")
            .to_string();

        let field_type_name = match field_type.as_str() {
            "Tx" => "Text",
            "Btn" => "Button",
            "Ch" => "Choice",
            "Sig" => "Signature",
            _ => "Unknown",
        }.to_string();

        // Get value
        let value = field_dict.get(b"V").ok().and_then(|o| match o {
            Object::String(s, _) => String::from_utf8(s.clone()).ok(),
            Object::Name(n) => std::str::from_utf8(n).ok().map(|s| s.to_string()),
            _ => None,
        });

        // Get default value
        let default_value = field_dict.get(b"DV").ok().and_then(|o| match o {
            Object::String(s, _) => String::from_utf8(s.clone()).ok(),
            Object::Name(n) => std::str::from_utf8(n).ok().map(|s| s.to_string()),
            _ => None,
        });

        // Check flags: /Ff
        let flags = field_dict.get(b"Ff").ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let is_readonly = (flags & 1) != 0;
        let is_required = (flags & 2) != 0;

        // Get options for Choice fields
        let options = if field_type == "Ch" {
            field_dict.get(b"Opt").ok()
                .and_then(|o| o.as_array().ok())
                .map(|arr| {
                    arr.iter().filter_map(|o| match o {
                        Object::String(s, _) => String::from_utf8(s.clone()).ok(),
                        Object::Name(n) => std::str::from_utf8(n).ok().map(|s| s.to_string()),
                        _ => None,
                    }).collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Determine page
        let page = None; // Could add page lookup if needed

        result.push(FormField {
            name: full_name.clone(),
            partial_name,
            field_type,
            field_type_name,
            value,
            default_value,
            page,
            is_readonly,
            is_required,
            options,
        });

        // Recurse into children
        if let Ok(kids) = field_dict.get(b"Kids").and_then(|o| o.as_array()) {
            collect_fields(doc, kids, &full_name, result)?;
        }
    }

    Ok(())
}

/// Set a field's value in the PDF
fn set_field_value(doc: &mut Document, field_id: ObjectId, value: &str) -> Result<(), String> {
    // Read field type first (owned copy to avoid borrow conflict)
    let field_type = {
        let field_dict = doc.get_dictionary(field_id)
            .map_err(|e| format!("Field not found: {e}"))?;
        field_dict.get(b"FT").ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| n.to_vec())
            .unwrap_or_default()
    };

    let pdf_string = Object::String(value.as_bytes().to_vec(), lopdf::StringFormat::Literal);

    // Set /V (value)
    let obj = doc.get_object_mut(field_id)
        .map_err(|e| format!("Cannot modify field: {e}"))?;

    if let Object::Dictionary(ref mut dict) = obj {
        dict.set("V", pdf_string.clone());

        // For text fields, also set /AP if needed
        if field_type == b"Tx" {
            // Mark as filled
            dict.set("AS", pdf_string);
        }

        // For button fields
        if field_type == b"Btn" {
            let name = Object::Name(value.as_bytes().to_vec());
            dict.set("V", name.clone());
            dict.set("AS", name);
        }
    }

    Ok(())
}
