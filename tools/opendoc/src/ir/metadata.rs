use serde::{Deserialize, Serialize};

/// Document metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<u32>,
    pub word_count: Option<u32>,
    pub character_count: Option<u32>,
    pub encrypted: bool,
    pub pdf_version: Option<String>,
    pub form_fields: Option<usize>,
    pub needs_ocr: Option<bool>,
    pub document_complexity: Option<String>,
}
