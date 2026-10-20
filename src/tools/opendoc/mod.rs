pub mod batch;
pub mod converters;
pub mod engine;
pub mod handlers;
pub mod ir;
pub mod ocr;
pub mod security;
pub mod server;
pub mod validators;

use anyhow::Result;
use crate::tools::opendoc::server::OpendocServer;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn get_server() -> &'static OpendocServer {
    static SERVER: std::sync::OnceLock<OpendocServer> = std::sync::OnceLock::new();
    SERVER.get_or_init(OpendocServer::new)
}

fn clean_and_resolve_path(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = trimmed.trim_matches('"').trim_matches('\'');
    let clean = unquoted.strip_prefix("file://").unwrap_or(unquoted);
    crate::config::loader::resolve_path(clean)
        .to_string_lossy()
        .to_string()
}

fn coerce_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        Value::Number(n) => match n.as_i64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

use crate::tools::arguments::{
    coerce_f32, coerce_f64, coerce_u32, coerce_usize, fill_from_aliases, get_alias,
    parse_json_string_field,
};

fn resolve_path_field(map: &mut serde_json::Map<String, Value>, key: &str) {
    let resolved = match map.get(key) {
        Some(Value::String(s)) => Some(clean_and_resolve_path(s)),
        _ => None,
    };
    match resolved {
        Some(r) => {
            map.insert(key.to_string(), Value::String(r));
        }
        None => {}
    }
}

fn fill_and_resolve_path(map: &mut serde_json::Map<String, Value>, key: &str, aliases: &[&str]) {
    fill_from_aliases(map, key, aliases);
    resolve_path_field(map, key);
}

fn normalize_opendoc_args(tool_name: &str, arguments: &Value) -> Value {
    match arguments {
        Value::String(s) => {
            let resolved = clean_and_resolve_path(s);
            let mut map = serde_json::Map::new();
            match tool_name {
                "opendoc_extract_archive_digest" => {
                    map.insert("archive_path".to_string(), Value::String(resolved));
                }
                "opendoc_convert" => {
                    map.insert("source".to_string(), Value::String(resolved.clone()));
                    map.insert("target_format".to_string(), Value::String("txt".to_string()));
                    map.insert("output".to_string(), Value::String(format!("{}.txt", resolved)));
                }
                "opendoc_create_docx" | "opendoc_create_pptx" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                }
                "opendoc_create_xlsx" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                    map.insert(
                        "sheets".to_string(),
                        json!([{"name": "Sheet1", "headers": [], "data": []}]),
                    );
                }
                "opendoc_create_pdf" | "opendoc_create_formatted_pdf" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                    map.insert("text".to_string(), Value::String("Document".to_string()));
                }
                "opendoc_create_html" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                    map.insert("body".to_string(), Value::String(String::new()));
                }
                "opendoc_batch_convert" => {
                    map.insert("input_dir".to_string(), Value::String(resolved.clone()));
                    map.insert("pattern".to_string(), Value::String("*.*".to_string()));
                    map.insert("target_format".to_string(), Value::String("pdf".to_string()));
                    map.insert("output_dir".to_string(), Value::String(resolved));
                }
                "opendoc_merge_pdfs" => {
                    map.insert("sources".to_string(), json!([resolved.clone()]));
                    map.insert(
                        "output_path".to_string(),
                        Value::String(format!("{}_merged.pdf", resolved)),
                    );
                }
                "opendoc_extract_images" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                    let out_dir = std::env::temp_dir()
                        .join("opendoc_extracted_images")
                        .to_string_lossy()
                        .to_string();
                    map.insert("output_dir".to_string(), Value::String(out_dir));
                }
                "opendoc_split_pdf" => {
                    map.insert("file_path".to_string(), Value::String(resolved.clone()));
                    map.insert(
                        "output_path".to_string(),
                        Value::String(format!("{}_split.pdf", resolved)),
                    );
                    map.insert("start_page".to_string(), json!(1));
                    map.insert("end_page".to_string(), json!(1));
                }
                "opendoc_search_document" => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                    map.insert("query".to_string(), Value::String(String::new()));
                }
                _ => {
                    map.insert("file_path".to_string(), Value::String(resolved));
                }
            }
            Value::Object(map)
        }
        Value::Object(orig_map) => {
            let mut map = orig_map.clone();

            // 1. file_path
            fill_and_resolve_path(
                &mut map,
                "file_path",
                &[
                    "path",
                    "filePath",
                    "file",
                    "document",
                    "doc",
                    "input_path",
                    "inputPath",
                    "input_file",
                    "inputFile",
                    "target",
                    "uri",
                    "url",
                ],
            );

            // 2. Convert tool: source, target_format, output
            match tool_name {
                "opendoc_convert" => {
                    fill_and_resolve_path(
                        &mut map,
                        "source",
                        &[
                            "file_path",
                            "filePath",
                            "input",
                            "input_path",
                            "inputPath",
                            "path",
                            "file",
                            "document",
                            "src",
                        ],
                    );
                    fill_from_aliases(
                        &mut map,
                        "target_format",
                        &[
                            "targetFormat",
                            "format",
                            "to_format",
                            "toFormat",
                            "to",
                            "type",
                        ],
                    );
                    fill_from_aliases(
                        &mut map,
                        "output",
                        &[
                            "output_path",
                            "outputPath",
                            "dest",
                            "destination",
                            "out",
                            "target",
                        ],
                    );
                    match (
                        map.contains_key("output"),
                        map.get("source").and_then(|v| v.as_str()).map(String::from),
                        map.get("target_format")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    ) {
                        (false, Some(src), Some(fmt)) => {
                            map.insert(
                                "output".to_string(),
                                Value::String(format!("{}.{}", src, fmt)),
                            );
                        }
                        _ => {}
                    }
                    resolve_path_field(&mut map, "output");
                }
                _ => {}
            }

            // 3. output_path & 4. input_dir
            fill_and_resolve_path(
                &mut map,
                "output_path",
                &[
                    "outputPath",
                    "output",
                    "out",
                    "dest",
                    "destination",
                    "target_path",
                    "targetPath",
                    "file_path",
                    "filePath",
                    "path",
                ],
            );
            fill_and_resolve_path(
                &mut map,
                "input_dir",
                &["inputDir", "dir", "directory", "folder", "path"],
            );

            // 5. output_dir
            fill_from_aliases(
                &mut map,
                "output_dir",
                &[
                    "outputDir",
                    "out_dir",
                    "outDir",
                    "dest_dir",
                    "destDir",
                    "destination",
                    "dir",
                ],
            );
            match (
                map.contains_key("output_dir"),
                matches!(
                    tool_name,
                    "opendoc_extract_images" | "opendoc_render_document_pages"
                ),
            ) {
                (false, true) => {
                    let temp = std::env::temp_dir()
                        .join("opendoc_extracted_output")
                        .to_string_lossy()
                        .to_string();
                    map.insert("output_dir".to_string(), Value::String(temp));
                }
                _ => {}
            }
            resolve_path_field(&mut map, "output_dir");

            // 6. archive_path, 7. file_path_a & file_path_b, 8. template_path & image_path
            fill_and_resolve_path(
                &mut map,
                "archive_path",
                &[
                    "archivePath",
                    "path",
                    "file_path",
                    "filePath",
                    "file",
                    "archive",
                    "zip_path",
                    "zipPath",
                ],
            );
            fill_and_resolve_path(
                &mut map,
                "file_path_a",
                &[
                    "filePathA",
                    "path_a",
                    "pathA",
                    "file_a",
                    "fileA",
                    "doc_a",
                    "source",
                ],
            );
            fill_and_resolve_path(
                &mut map,
                "file_path_b",
                &[
                    "filePathB",
                    "path_b",
                    "pathB",
                    "file_b",
                    "fileB",
                    "doc_b",
                    "target",
                ],
            );
            fill_and_resolve_path(
                &mut map,
                "template_path",
                &["templatePath", "template", "file_path", "path", "source"],
            );
            fill_and_resolve_path(
                &mut map,
                "image_path",
                &["imagePath", "image", "img", "src", "source", "path"],
            );

            // 9. sources (opendoc_merge_pdfs)
            fill_from_aliases(
                &mut map,
                "sources",
                &["input_paths", "inputPaths", "files", "paths", "inputs"],
            );
            match map.get("sources").cloned() {
                Some(Value::Array(arr)) => {
                    let cleaned: Vec<Value> = arr
                        .into_iter()
                        .map(|item| match item {
                            Value::String(s) => Value::String(clean_and_resolve_path(&s)),
                            other => other,
                        })
                        .collect();
                    map.insert("sources".to_string(), Value::Array(cleaned));
                }
                Some(Value::String(s)) => {
                    let cleaned: Vec<Value> = s
                        .split([',', '\n'])
                        .filter(|p| !p.trim().is_empty())
                        .map(|p| Value::String(clean_and_resolve_path(p)))
                        .collect();
                    map.insert("sources".to_string(), Value::Array(cleaned));
                }
                _ => {}
            }

            // 10. query, 11. text, 12. body
            fill_from_aliases(
                &mut map,
                "query",
                &["q", "search", "pattern", "text", "term", "filter"],
            );
            fill_from_aliases(
                &mut map,
                "text",
                &[
                    "content",
                    "body",
                    "markdown",
                    "text_content",
                    "paragraph",
                    "line",
                ],
            );
            fill_from_aliases(
                &mut map,
                "body",
                &["content", "text", "html", "markdown", "body_markdown"],
            );

            // 13..18. Tool-specific fields
            match tool_name {
                "opendoc_pptx_add_slide" => {
                    fill_from_aliases(
                        &mut map,
                        "body",
                        &[
                            "bullets",
                            "bullet_points",
                            "bulletPoints",
                            "points",
                            "content",
                            "text",
                        ],
                    );
                    match map.get("body").cloned() {
                        Some(Value::String(s)) => {
                            let lines: Vec<Value> = s
                                .lines()
                                .map(|l| {
                                    l.trim()
                                        .trim_start_matches('-')
                                        .trim_start_matches('*')
                                        .trim()
                                })
                                .filter(|l| !l.is_empty())
                                .map(|l| Value::String(l.to_string()))
                                .collect();
                            map.insert("body".to_string(), Value::Array(lines));
                        }
                        _ => {}
                    }
                }
                "opendoc_create_xlsx" => match map.contains_key("sheets") {
                    false => {
                        let sheet_name =
                            get_alias(&map, &["sheet", "sheet_name", "sheetName", "name"])
                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| "Sheet1".to_string());
                        let headers = get_alias(&map, &["headers", "columns", "header"])
                            .unwrap_or_else(|| json!([]));
                        let data = get_alias(&map, &["data", "rows", "values", "records"])
                            .unwrap_or_else(|| json!([]));
                        map.insert(
                            "sheets".to_string(),
                            json!([{"name": sheet_name, "headers": headers, "data": data}]),
                        );
                    }
                    true => parse_json_string_field(&mut map, "sheets"),
                },
                "opendoc_docx_add_table" => {
                    fill_from_aliases(
                        &mut map,
                        "data",
                        &["rows", "table_data", "tableData", "values", "records"],
                    );
                }
                "opendoc_fill_pdf_form" => {
                    fill_from_aliases(
                        &mut map,
                        "values",
                        &[
                            "fields",
                            "field_values",
                            "fieldValues",
                            "data",
                            "form_data",
                            "formData",
                        ],
                    );
                    parse_json_string_field(&mut map, "values");
                }
                "opendoc_fill_template" => {
                    fill_from_aliases(
                        &mut map,
                        "variables",
                        &["vars", "data", "values", "context"],
                    );
                    parse_json_string_field(&mut map, "variables");
                }
                "opendoc_split_pdf" => {
                    fill_from_aliases(
                        &mut map,
                        "start_page",
                        &[
                            "startPage",
                            "start",
                            "from_page",
                            "fromPage",
                            "page_start",
                            "first_page",
                        ],
                    );
                    let start = map.get("start_page").and_then(coerce_u32).unwrap_or(1);
                    map.insert("start_page".to_string(), json!(start));

                    fill_from_aliases(
                        &mut map,
                        "end_page",
                        &[
                            "endPage",
                            "end",
                            "to_page",
                            "toPage",
                            "page_end",
                            "last_page",
                        ],
                    );
                    let end = map.get("end_page").and_then(coerce_u32).unwrap_or(start);
                    map.insert("end_page".to_string(), json!(end));
                }
                "opendoc_batch_convert" => match map.contains_key("pattern") {
                    false => {
                        let val = get_alias(&map, &["filter", "glob"])
                            .unwrap_or_else(|| Value::String("*.*".to_string()));
                        map.insert("pattern".to_string(), val);
                    }
                    true => {}
                },
                _ => {}
            }

            // 19. Numeric fields coercions
            for num_key in &[
                "page",
                "page_number",
                "rows",
                "cols",
                "dpi",
                "width_emu",
                "height_emu",
            ] {
                match map.get(*num_key).and_then(coerce_u32) {
                    Some(coerced) => {
                        map.insert(num_key.to_string(), json!(coerced));
                    }
                    None => {}
                }
            }
            for usize_key in &["chunk_size", "overlap", "concurrency"] {
                match map.get(*usize_key).and_then(coerce_usize) {
                    Some(coerced) => {
                        map.insert(usize_key.to_string(), json!(coerced));
                    }
                    None => {}
                }
            }
            for f32_key in &["font_size"] {
                match map.get(*f32_key).and_then(coerce_f32) {
                    Some(coerced) => {
                        map.insert(f32_key.to_string(), json!(coerced));
                    }
                    None => {}
                }
            }
            for f64_key in &[
                "margin_top",
                "margin_bottom",
                "margin_left",
                "margin_right",
                "line_spacing",
            ] {
                match map.get(*f64_key).and_then(coerce_f64) {
                    Some(coerced) => {
                        map.insert(f64_key.to_string(), json!(coerced));
                    }
                    None => {}
                }
            }

            // 20. Boolean fields coercions
            for bool_key in &[
                "use_regex",
                "is_regex",
                "recursive",
                "flatten",
                "page_numbers",
                "bold",
                "italic",
                "underline",
                "keep_with_next",
                "keep_together",
                "page_break_before",
            ] {
                match map.get(*bool_key).and_then(coerce_bool) {
                    Some(coerced) => {
                        map.insert(bool_key.to_string(), Value::Bool(coerced));
                    }
                    None => {}
                }
            }

            Value::Object(map)
        }
        other => other.clone(),
    }
}

pub fn sanitize_opendoc_schema(mut val: Value) -> Value {
    match val {
        Value::Object(ref mut map) => {
            map.remove("$schema");
            map.remove("title");

            // Ensure top-level type is "object"
            match map.contains_key("type") {
                false => {
                    map.insert("type".to_string(), Value::String("object".to_string()));
                }
                true => {}
            }

            // Sanitize properties
            match map.get_mut("properties") {
                Some(Value::Object(props)) => {
                    for (k, v) in props.iter_mut() {
                        match v {
                            Value::Object(prop_map) => {
                                prop_map.remove("title");

                                let missing_type = !prop_map.contains_key("type")
                                    && !prop_map.contains_key("$ref")
                                    && !prop_map.contains_key("anyOf")
                                    && !prop_map.contains_key("oneOf");
                                match (missing_type, k.as_str()) {
                                    (true, "sheets" | "cell_updates") => {
                                        prop_map.insert(
                                            "type".to_string(),
                                            Value::String("array".to_string()),
                                        );
                                        prop_map.insert(
                                            "items".to_string(),
                                            json!({
                                                "type": "object",
                                                "additionalProperties": true
                                            }),
                                        );
                                    }
                                    (true, "add_sheets") => {
                                        prop_map.insert(
                                            "type".to_string(),
                                            Value::String("array".to_string()),
                                        );
                                        prop_map.insert(
                                            "items".to_string(),
                                            json!({
                                                "type": "string"
                                            }),
                                        );
                                    }
                                    (true, _) => {
                                        prop_map.insert(
                                            "type".to_string(),
                                            Value::String("object".to_string()),
                                        );
                                        prop_map.insert(
                                            "additionalProperties".to_string(),
                                            Value::Bool(true),
                                        );
                                    }
                                    (false, _) => {}
                                }

                                // If type is "object" or contains "object", ensure it either has properties or additionalProperties
                                let is_object_type = match prop_map.get("type") {
                                    Some(Value::String(s)) => s == "object",
                                    Some(Value::Array(arr)) => {
                                        arr.iter().any(|v| v.as_str() == Some("object"))
                                    }
                                    _ => false,
                                };
                                match is_object_type
                                    && !prop_map.contains_key("properties")
                                    && !prop_map.contains_key("additionalProperties")
                                {
                                    true => {
                                        prop_map.insert(
                                            "additionalProperties".to_string(),
                                            Value::Bool(true),
                                        );
                                    }
                                    false => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    val
}

macro_rules! define_opendoc_tool {
    ($struct_name:ident, $tool_name:expr, $description:expr, $params_struct:ident, $body:expr) => {
        pub struct $struct_name;

        #[async_trait::async_trait]
        impl crate::tools::Tool for $struct_name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn description(&self) -> &str {
                $description
            }

            fn parameters(&self) -> Value {
                let schema = schemars::schema_for!($params_struct);
                let val = serde_json::to_value(schema).unwrap_or_else(|_| json!({"type": "object"}));
                sanitize_opendoc_schema(val)
            }

            async fn call(&self, arguments: &Value) -> Result<Value> {
                let normalized = normalize_opendoc_args($tool_name, arguments);
                let req: $params_struct = serde_json::from_value(normalized)?;
                let caller = $body;
                let res_str = caller(req);
                let res_val: Value = serde_json::from_str(&res_str).unwrap_or_else(|_| json!({
                    "success": false,
                    "error": "Failed to parse JSON response from backend",
                    "raw": res_str
                }));
                Ok(res_val)
            }
        }
    };
}

// ────────────────────────────────────────────────────────────────
//  1. Open & Read & Search & Replace Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct OpenDocumentParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Level of detail: 'summary', 'metadata_only', or 'full'")]
    pub detail_level: Option<String>,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocOpenDocumentTool,
    "opendoc_open_document",
    "Open any supported document (DOCX, PPTX, PDF, XLSX, HTML, MD, CSV, TXT) and return structured JSON layout or content.",
    OpenDocumentParams,
    |p: OpenDocumentParams| { get_server().open_document(p.file_path, p.detail_level, p.password) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ReadDocumentTextParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocReadDocumentTextTool,
    "opendoc_read_document_text",
    "Read the full text content of any document (plain text extraction).",
    ReadDocumentTextParams,
    |p: ReadDocumentTextParams| { get_server().read_document_text(p.file_path, p.password) }
);

#[derive(Deserialize, JsonSchema)]
pub struct SearchDocumentParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Search query (plain text or regex pattern)")]
    pub query: String,
    #[schemars(description = "If true, treat query as a regex pattern")]
    pub use_regex: Option<bool>,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocSearchDocumentTool,
    "opendoc_search_document",
    "Search for text or regex pattern matches inside a document.",
    SearchDocumentParams,
    |p: SearchDocumentParams| {
        get_server().search_document(p.file_path, p.query, p.use_regex, p.password)
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct ReplaceTextParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Text or pattern to find")]
    pub find: String,
    #[schemars(description = "Replacement text")]
    pub replace: String,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocReplaceTextTool,
    "opendoc_replace_text",
    "Search and replace text in a document (works on DOCX, XLSX, MD, CSV, HTML, TXT).",
    ReplaceTextParams,
    |p: ReplaceTextParams| {
        get_server().replace_text(p.file_path, p.find, p.replace, p.password)
    }
);

// ────────────────────────────────────────────────────────────────
//  2. Diff & Templating & Chunking Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct DiffDocumentsParams {
    #[schemars(description = "First document path")]
    pub file_a: String,
    #[schemars(description = "Second document path")]
    pub file_b: String,
}
define_opendoc_tool!(
    OpendocDiffDocumentsTool,
    "opendoc_diff_documents",
    "Compare two documents and return a structured JSON diff of paragraphs, tables, and sections.",
    DiffDocumentsParams,
    |p: DiffDocumentsParams| { get_server().diff_documents(p.file_a, p.file_b) }
);

#[derive(Deserialize, JsonSchema)]
pub struct DiffDocumentsVisualParams {
    #[schemars(description = "First document path")]
    pub file_a: String,
    #[schemars(description = "Second document path")]
    pub file_b: String,
    #[schemars(description = "Output format: html or markdown (default: html)")]
    pub format: Option<String>,
}
define_opendoc_tool!(
    OpendocDiffDocumentsVisualTool,
    "opendoc_diff_documents_visual",
    "Compare two documents and render a visual HTML/Markdown diff with additions and deletions highlighted.",
    DiffDocumentsVisualParams,
    |p: DiffDocumentsVisualParams| {
        get_server().diff_documents_visual(p.file_a, p.file_b, p.format)
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct ChunkForEmbeddingParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Strategy: fixed, heading, recursive, page (default: fixed)")]
    pub strategy: Option<String>,
    #[schemars(description = "Maximum tokens per chunk (default 512)")]
    pub max_tokens: Option<usize>,
    #[schemars(description = "Token overlap between consecutive chunks (default: 50)")]
    pub overlap: Option<usize>,
    #[schemars(
        description = "Whether to compute dense vector embeddings for chunks using local AllMiniLML6V2 model (default false)"
    )]
    pub generate_embeddings: Option<bool>,
}
define_opendoc_tool!(
    OpendocChunkForEmbeddingTool,
    "opendoc_chunk_for_embedding",
    "Chunk document content using smart strategies (fixed token count, heading, page) for RAG input pipelines.",
    ChunkForEmbeddingParams,
    |p: ChunkForEmbeddingParams| {
        get_server().chunk_for_embedding(
            p.file_path,
            p.strategy,
            p.max_tokens,
            p.overlap,
            p.generate_embeddings,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct FillTemplateParams {
    #[schemars(description = "File path to the template document")]
    pub file_path: String,
    #[schemars(
        description = "JSON object of key-value pairs to fill, e.g. {\"name\":\"Ada\",\"items\":[\"A\",\"B\"]}"
    )]
    pub variables: serde_json::Value,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocFillTemplateTool,
    "opendoc_fill_template",
    "Fill template placeholders in a document using a JSON context (supports nested structures, conditionals, loops).",
    FillTemplateParams,
    |p: FillTemplateParams| { get_server().fill_template(p.file_path, p.variables, p.password) }
);

// ────────────────────────────────────────────────────────────────
//  3. Document Validation & Metadata Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct ValidateDocumentParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocValidateDocumentTool,
    "opendoc_validate_document",
    "Validate document integrity and return structural statistics (character count, paragraphs, sections, tables, images).",
    ValidateDocumentParams,
    |p: ValidateDocumentParams| { get_server().validate_document(p.file_path, p.password) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ValidatePdfAComplianceParams {
    #[schemars(description = "File path to the PDF document")]
    pub file_path: String,
}
define_opendoc_tool!(
    OpendocValidatePdfAComplianceTool,
    "opendoc_validate_pdf_a_compliance",
    "Validate a PDF file for PDF/A standard compliance (long-term preservation).",
    ValidatePdfAComplianceParams,
    |p: ValidatePdfAComplianceParams| { get_server().validate_pdf_a_compliance(p.file_path) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ExtractStructuredMetadataParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "The target domain template: 'legal', 'financial', 'timeline', or 'general' (optional, defaults to 'general')")]
    pub template_type: Option<String>,
}
define_opendoc_tool!(
    OpendocExtractStructuredMetadataTool,
    "opendoc_extract_structured_metadata",
    "Extract structured domain metadata (legal, financial, timeline, general) using rule-based parsing.",
    ExtractStructuredMetadataParams,
    |p: ExtractStructuredMetadataParams| {
        get_server().extract_structured_metadata(
            p.file_path,
            p.template_type.unwrap_or_else(|| "general".to_string()),
        )
    }
);

// ────────────────────────────────────────────────────────────────
//  4. Conversion & Split & Image Extraction Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct ConvertParams {
    #[schemars(description = "Source file path")]
    pub source: String,
    #[schemars(description = "Target format (pdf, md, html, csv, txt, json)")]
    pub target_format: String,
    #[schemars(description = "Output file path")]
    pub output: String,
    #[schemars(description = "Optional password for encrypted source documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocConvertTool,
    "opendoc_convert",
    "Convert a document from one format to another (e.g. DOCX to PDF, HTML to XLSX, PDF to Markdown).",
    ConvertParams,
    |p: ConvertParams| { get_server().convert(p.source, p.target_format, p.output, p.password) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ExtractImagesParams {
    #[schemars(description = "File path to the DOCX or PPTX document")]
    pub file_path: String,
    #[schemars(description = "Directory where the extracted images should be saved")]
    pub output_dir: String,
}
define_opendoc_tool!(
    OpendocExtractImagesTool,
    "opendoc_extract_images",
    "Extract embedded images from office files (DOCX, PPTX).",
    ExtractImagesParams,
    |p: ExtractImagesParams| { get_server().extract_images(p.file_path, p.output_dir) }
);

#[derive(Deserialize, JsonSchema)]
pub struct SplitPdfParams {
    #[schemars(description = "File path to the PDF document to split")]
    pub file_path: String,
    #[schemars(description = "File path to save the split PDF")]
    pub output_path: String,
    #[schemars(description = "Start page number (1-based, inclusive)")]
    pub start_page: u32,
    #[schemars(description = "End page number (1-based, inclusive)")]
    pub end_page: u32,
    #[schemars(description = "Optional password for encrypted documents")]
    pub password: Option<String>,
}
define_opendoc_tool!(
    OpendocSplitPdfTool,
    "opendoc_split_pdf",
    "Split a PDF document into a subset of pages specified by a start and end page (1-based, inclusive).",
    SplitPdfParams,
    |p: SplitPdfParams| {
        get_server().split_pdf(
            p.file_path,
            p.output_path,
            p.start_page,
            p.end_page,
            p.password,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct CreateHtmlParams {
    #[schemars(description = "File path to save the HTML")]
    pub file_path: String,
    #[schemars(description = "Body text content (paragraphs separated by newlines)")]
    pub body: String,
    #[schemars(description = "Optional HTML title")]
    pub title: Option<String>,
}
define_opendoc_tool!(
    OpendocCreateHtmlTool,
    "opendoc_create_html",
    "Create a basic HTML document with paragraphs.",
    CreateHtmlParams,
    |p: CreateHtmlParams| { get_server().create_html(p.file_path, p.body, p.title) }
);

#[derive(Deserialize, JsonSchema)]
pub struct BatchConvertParams {
    #[schemars(description = "Input directory path")]
    pub input_dir: String,
    #[schemars(description = "File pattern (e.g., *.docx, *.pdf)")]
    pub pattern: String,
    #[schemars(description = "Target format")]
    pub target_format: String,
    #[schemars(description = "Output directory")]
    pub output_dir: String,
    #[schemars(description = "If true, walk directories recursively")]
    pub recursive: Option<bool>,
    #[schemars(description = "Optional password for encrypted source documents")]
    pub password: Option<String>,
    #[schemars(description = "Optional concurrency thread limit")]
    pub concurrency: Option<usize>,
}
define_opendoc_tool!(
    OpendocBatchConvertTool,
    "opendoc_batch_convert",
    "Batch convert all document files in a directory using parallel processing.",
    BatchConvertParams,
    |p: BatchConvertParams| {
        get_server().batch_convert(
            p.input_dir,
            p.pattern,
            p.target_format,
            p.output_dir,
            p.recursive,
            p.password,
            p.concurrency,
        )
    }
);

// ────────────────────────────────────────────────────────────────
//  5. Office Creation & Formatting Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct CreateDocxParams {
    #[schemars(description = "File path to create")]
    pub file_path: String,
    #[schemars(description = "Document main header/title")]
    pub title: Option<String>,
}
define_opendoc_tool!(
    OpendocCreateDocxTool,
    "opendoc_create_docx",
    "Create a new empty DOCX document with an optional title.",
    CreateDocxParams,
    |p: CreateDocxParams| { get_server().create_docx(p.file_path, p.title) }
);

#[derive(Deserialize, JsonSchema)]
pub struct DocxAddParagraphParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(
        description = "Text content. Never insert manual bullet character lists; use standard paragraph lines."
    )]
    pub text: String,
    #[schemars(description = "Optional bold. Use for titles and table headers.")]
    pub bold: Option<bool>,
    #[schemars(description = "Optional italic. Use for captions, quotes, or sub-details.")]
    pub italic: Option<bool>,
    #[schemars(description = "Optional underline.")]
    pub underline: Option<bool>,
    #[schemars(description = "Optional font size in points.")]
    pub font_size: Option<f32>,
    #[schemars(description = "Optional font family name.")]
    pub font_family: Option<String>,
    #[schemars(description = "Optional font color (Hex RGB).")]
    pub color: Option<String>,
    #[schemars(description = "Optional font highlight color (Hex RGB).")]
    pub highlight: Option<String>,
    #[schemars(description = "Optional alignment: left, center, right, justify.")]
    pub alignment: Option<String>,
    #[schemars(description = "Optional shading fill color (Hex RGB).")]
    pub shading: Option<String>,
    #[schemars(description = "Optional line spacing.")]
    pub line_spacing: Option<f64>,
    #[schemars(description = "Optional keep with next paragraph.")]
    pub keep_with_next: Option<bool>,
    #[schemars(description = "Optional keep lines together.")]
    pub keep_together: Option<bool>,
    #[schemars(description = "Optional page break before paragraph.")]
    pub page_break_before: Option<bool>,
}
define_opendoc_tool!(
    OpendocDocxAddParagraphTool,
    "opendoc_docx_add_paragraph",
    "Append or insert a styled paragraph to a DOCX document.",
    DocxAddParagraphParams,
    |p: DocxAddParagraphParams| {
        get_server().docx_add_paragraph(
            p.file_path,
            p.text,
            p.bold,
            p.italic,
            p.underline,
            p.font_size,
            p.font_family,
            p.color,
            p.highlight,
            p.alignment,
            p.shading,
            p.line_spacing,
            p.keep_with_next,
            p.keep_together,
            p.page_break_before,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct DocxAddTableParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(
        description = "Headers (JSON array of strings). E.g. ['Item', 'Quantity', 'Cost']."
    )]
    pub headers: Vec<String>,
    #[schemars(
        description = "Data rows (JSON array of arrays of strings). Must match header length."
    )]
    #[serde(alias = "rows")]
    pub data: Vec<Vec<String>>,
    #[schemars(description = "Optional table width percentage (0 to 100).")]
    pub width_pct: Option<f64>,
    #[schemars(description = "Optional table alignment: left, center, right, justify.")]
    pub alignment: Option<String>,
    #[schemars(description = "Optional table border style.")]
    pub border_style: Option<String>,
    #[schemars(description = "Optional border size in eighths of a pt.")]
    pub border_size: Option<u32>,
    #[schemars(description = "Optional border color (Hex RGB).")]
    pub border_color: Option<String>,
    #[schemars(description = "Optional shading header color (Hex RGB).")]
    pub shading_header: Option<String>,
    #[schemars(description = "Optional shading data cell color (Hex RGB).")]
    pub shading_data: Option<String>,
    #[schemars(description = "Optional prevent page breaks inside rows.")]
    pub cant_split: Option<bool>,
}
define_opendoc_tool!(
    OpendocDocxAddTableTool,
    "opendoc_docx_add_table",
    "Append or insert a table structure in a DOCX document.",
    DocxAddTableParams,
    |p: DocxAddTableParams| {
        get_server().docx_add_table(
            p.file_path,
            p.headers,
            p.data,
            p.width_pct,
            p.alignment,
            p.border_style,
            p.border_size,
            p.border_color,
            p.shading_header,
            p.shading_data,
            p.cant_split,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct DocxAddImageParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "File path to the image to insert")]
    pub image_path: String,
    #[schemars(description = "Optional image width in inches (default: 2.0)")]
    pub width_inches: Option<f64>,
    #[schemars(description = "Optional image height in inches (default: 1.5)")]
    pub height_inches: Option<f64>,
}
define_opendoc_tool!(
    OpendocDocxAddImageTool,
    "opendoc_docx_add_image",
    "Append or insert an image file structure in a DOCX document.",
    DocxAddImageParams,
    |p: DocxAddImageParams| {
        get_server().docx_add_image(p.file_path, p.image_path, p.width_inches, p.height_inches)
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct CreatePptxParams {
    #[schemars(description = "File path to create")]
    pub file_path: String,
    #[schemars(description = "Presentation main title")]
    pub title: Option<String>,
}
define_opendoc_tool!(
    OpendocCreatePptxTool,
    "opendoc_create_pptx",
    "Create a new empty PPTX presentation file.",
    CreatePptxParams,
    |p: CreatePptxParams| { get_server().create_pptx(p.file_path, p.title) }
);

#[derive(Deserialize, JsonSchema)]
pub struct PptxAddSlideParams {
    #[schemars(description = "File path to the presentation")]
    pub file_path: String,
    #[schemars(description = "Slide title.")]
    pub title: String,
    #[schemars(description = "Optional bullet point content.")]
    pub body: Option<Vec<String>>,
    #[schemars(description = "Optional slide background color (Hex RGB).")]
    pub bg_color: Option<String>,
    #[schemars(description = "Optional font size for body points.")]
    pub font_size: Option<f32>,
    #[schemars(description = "Optional font color (Hex RGB).")]
    pub font_color: Option<String>,
    #[schemars(description = "Optional font family name.")]
    pub font_family: Option<String>,
    #[schemars(description = "Optional alignment: left, center, right, justify.")]
    pub alignment: Option<String>,
}
define_opendoc_tool!(
    OpendocPptxAddSlideTool,
    "opendoc_pptx_add_slide",
    "Append a new slide to a PPTX presentation with custom layout and styles.",
    PptxAddSlideParams,
    |p: PptxAddSlideParams| {
        get_server().pptx_add_slide(
            p.file_path,
            p.title,
            p.body,
            p.bg_color,
            p.font_size,
            p.font_color,
            p.font_family,
            p.alignment,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct CreateXlsxParams {
    #[schemars(description = "File path to save the XLSX")]
    pub file_path: String,
    #[schemars(
        description = "JSON array of sheets: [{\"name\": \"Sheet1\", \"headers\": [\"A\",\"B\"], \"data\": [[\"1\",\"2\"]]}]"
    )]
    pub sheets: serde_json::Value,
}
define_opendoc_tool!(
    OpendocCreateXlsxTool,
    "opendoc_create_xlsx",
    "Create a new Excel XLSX spreadsheet file.",
    CreateXlsxParams,
    |p: CreateXlsxParams| { get_server().create_xlsx(p.file_path, p.sheets) }
);

#[derive(Deserialize, JsonSchema)]
pub struct EditXlsxParams {
    #[schemars(description = "File path of the existing XLSX to edit")]
    pub file_path: String,
    #[schemars(description = "Optional JSON array of sheet names to add: [\"Summary\"]")]
    pub add_sheets: Option<serde_json::Value>,
    #[schemars(
        description = "Optional JSON array of cell updates: [{\"sheet_name\": \"Sheet1\", \"row\": 1, \"col\": 1, \"value\": \"31\"}]"
    )]
    pub cell_updates: Option<serde_json::Value>,
}
define_opendoc_tool!(
    OpendocEditXlsxTool,
    "opendoc_edit_xlsx",
    "Write or edit a cell coordinate value and formatting in an Excel sheet.",
    EditXlsxParams,
    |p: EditXlsxParams| { get_server().edit_xlsx(p.file_path, p.add_sheets, p.cell_updates) }
);

// ────────────────────────────────────────────────────────────────
//  6. PDF Creation & PDF Form Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct CreatePdfParams {
    #[schemars(description = "File path to save the PDF")]
    pub file_path: String,
    #[schemars(description = "Text content")]
    pub text: String,
    #[schemars(description = "Optional author name")]
    pub author: Option<String>,
}
define_opendoc_tool!(
    OpendocCreatePdfTool,
    "opendoc_create_pdf",
    "Create a basic plain text PDF document.",
    CreatePdfParams,
    |p: CreatePdfParams| { get_server().create_pdf(p.file_path, p.text, p.author) }
);

#[derive(Deserialize, JsonSchema)]
pub struct CreateFormattedPdfParams {
    #[schemars(description = "File path to save the PDF")]
    pub file_path: String,
    #[schemars(
        description = "Text content. Separate chapters/pages using Form Feed '\\x0c' or '\\f' characters."
    )]
    pub text: String,
    #[schemars(description = "Optional document title (rendered centered on page 1).")]
    pub title: Option<String>,
    #[schemars(description = "Optional author name (printed on page 1 below title).")]
    pub author: Option<String>,
    #[schemars(description = "Whether to show page numbers in footer (default: false).")]
    pub page_numbers: Option<bool>,
    #[schemars(description = "Font size in points (default: 12).")]
    pub font_size: Option<f64>,
    #[schemars(description = "Top margin in points (default: 72 = 1 inch).")]
    pub margin_top: Option<f64>,
    #[schemars(description = "Bottom margin in points (default: 72 = 1 inch).")]
    pub margin_bottom: Option<f64>,
    #[schemars(description = "Left margin in points (default: 72 = 1 inch).")]
    pub margin_left: Option<f64>,
    #[schemars(description = "Right margin in points (default: 72 = 1 inch).")]
    pub margin_right: Option<f64>,
}
define_opendoc_tool!(
    OpendocCreateFormattedPdfTool,
    "opendoc_create_formatted_pdf",
    "Create a highly structured multi-page PDF document with automatic word-wrap, headings, tables, margins, and page numbers.",
    CreateFormattedPdfParams,
    |p: CreateFormattedPdfParams| {
        get_server().create_formatted_pdf(
            p.file_path,
            p.text,
            p.title,
            p.author,
            p.page_numbers,
            p.font_size,
            p.margin_top,
            p.margin_bottom,
            p.margin_left,
            p.margin_right,
        )
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct MergePdfsParams {
    #[schemars(description = "List of PDF file paths to merge")]
    pub sources: Vec<String>,
    #[schemars(description = "Output PDF file path")]
    pub output_path: String,
}
define_opendoc_tool!(
    OpendocMergePdfsTool,
    "opendoc_merge_pdfs",
    "Merge multiple PDF documents together in sequence.",
    MergePdfsParams,
    |p: MergePdfsParams| { get_server().merge_pdfs(p.sources, p.output_path) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ExtractPdfTextParams {
    #[schemars(description = "File path to the PDF")]
    pub file_path: String,
    #[schemars(description = "Optional page number (0-based)")]
    pub page: Option<u32>,
}
define_opendoc_tool!(
    OpendocExtractPdfTextTool,
    "opendoc_extract_pdf_text",
    "Extract plain text content from a PDF document.",
    ExtractPdfTextParams,
    |p: ExtractPdfTextParams| { get_server().extract_pdf_text(p.file_path, p.page) }
);

#[derive(Deserialize, JsonSchema)]
pub struct ListPdfFieldsParams {
    #[schemars(description = "File path to the PDF")]
    pub file_path: String,
}
define_opendoc_tool!(
    OpendocListPdfFieldsTool,
    "opendoc_list_pdf_fields",
    "List all interactive form fields (AcroForm) and types in a PDF file.",
    ListPdfFieldsParams,
    |p: ListPdfFieldsParams| { get_server().list_pdf_fields(p.file_path) }
);

#[derive(Deserialize, JsonSchema)]
pub struct FillPdfFormParams {
    #[schemars(description = "File path to the PDF")]
    pub file_path: String,
    #[schemars(
        description = "JSON object mapping field names to values, e.g. {\"full_name\":\"Ada Lovelace\",\"accepted\":true}"
    )]
    pub values: serde_json::Value,
}
define_opendoc_tool!(
    OpendocFillPdfFormTool,
    "opendoc_fill_pdf_form",
    "Fill interactive form fields in a PDF using a JSON fields mapping context.",
    FillPdfFormParams,
    |p: FillPdfFormParams| { get_server().fill_pdf_form(p.file_path, p.values) }
);

// ────────────────────────────────────────────────────────────────
//  7. Advanced Tables & OCR & Vision & Archive Tools
// ────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct FindTablesParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
}
define_opendoc_tool!(
    OpendocFindTablesTool,
    "opendoc_find_tables",
    "Locate and extract all table structures from any document type.",
    FindTablesParams,
    |p: FindTablesParams| { get_server().find_tables(p.file_path) }
);

#[derive(Deserialize, JsonSchema)]
pub struct AnalyzeDocumentComplexityParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
}
define_opendoc_tool!(
    OpendocAnalyzeDocumentComplexityTool,
    "opendoc_analyze_document_complexity",
    "Analyze document properties to determine processing complexity (text density, scanned page heuristics, OCR recommendations).",
    AnalyzeDocumentComplexityParams,
    |p: AnalyzeDocumentComplexityParams| { get_server().analyze_document_complexity(p.file_path) }
);

#[derive(Deserialize, JsonSchema)]
pub struct OcrDocumentParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Language code (default: eng)")]
    pub language: Option<String>,
}
define_opendoc_tool!(
    OpendocOcrDocumentTool,
    "opendoc_ocr_document",
    "Run scanned image OCR text recognition on document pages (requires Tesseract CLI).",
    OcrDocumentParams,
    |p: OcrDocumentParams| { get_server().ocr_document(p.file_path, p.language) }
);

pub struct OpendocCheckOcrAvailableTool;
#[async_trait::async_trait]
impl crate::tools::Tool for OpendocCheckOcrAvailableTool {
    fn name(&self) -> &str {
        "opendoc_check_ocr_available"
    }
    fn description(&self) -> &str {
        "Check status and availability of OCR system dependencies (Tesseract)"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let res_str = get_server().check_ocr_available();
        let res_val: Value = serde_json::from_str(&res_str).unwrap_or_else(|_| {
            json!({
                "success": false,
                "raw": res_str
            })
        });
        Ok(res_val)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RenderDocumentPagesParams {
    #[schemars(description = "File path to the document")]
    pub file_path: String,
    #[schemars(description = "Target directory to save the rendered PNG files")]
    pub output_dir: String,
    #[schemars(description = "Optional resolution DPI (default: 150)")]
    pub dpi: Option<u32>,
    #[schemars(description = "Optional list of 1-based page numbers to render")]
    pub pages: Option<Vec<u32>>,
}
define_opendoc_tool!(
    OpendocRenderDocumentPagesTool,
    "opendoc_render_document_pages",
    "Render PDF/Office document pages directly into high-fidelity image paths for AI vision inspection.",
    RenderDocumentPagesParams,
    |p: RenderDocumentPagesParams| {
        get_server().render_document_pages(p.file_path, p.output_dir, p.dpi, p.pages)
    }
);

#[derive(Deserialize, JsonSchema)]
pub struct ExtractArchiveDigestParams {
    #[schemars(description = "Absolute path to the ZIP archive")]
    pub archive_path: String,
    #[schemars(
        description = "Optional destination directory. If not specified, a temporary directory will be created."
    )]
    pub output_dir: Option<String>,
}
define_opendoc_tool!(
    OpendocExtractArchiveDigestTool,
    "opendoc_extract_archive_digest",
    "Recursively unpack and extract text digests from all documents inside a ZIP archive.",
    ExtractArchiveDigestParams,
    |p: ExtractArchiveDigestParams| {
        get_server().extract_archive_digest(p.archive_path, p.output_dir)
    }
);

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

