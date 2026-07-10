use anyhow::Result;
use opendoc_mcp::server::OpendocServer;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn get_server() -> &'static OpendocServer {
    static SERVER: std::sync::OnceLock<OpendocServer> = std::sync::OnceLock::new();
    SERVER.get_or_init(OpendocServer::new)
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
                serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
            }

            async fn call(&self, arguments: &Value) -> Result<Value> {
                let req: $params_struct = serde_json::from_value(arguments.clone())?;
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
define_opendoc_tool!(OpendocOpenDocumentTool, "opendoc_open_document", "Open any supported document (DOCX, PPTX, PDF, XLSX, HTML, MD, CSV, TXT) and return structured JSON layout or content.", OpenDocumentParams, |p: OpenDocumentParams| {
    get_server().open_document(p.file_path, p.detail_level, p.password)
});

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
define_opendoc_tool!(OpendocDiffDocumentsVisualTool, "opendoc_diff_documents_visual", "Compare two documents and render a visual HTML/Markdown diff with additions and deletions highlighted.", DiffDocumentsVisualParams, |p: DiffDocumentsVisualParams| {
    get_server().diff_documents_visual(p.file_a, p.file_b, p.format)
});

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
}
define_opendoc_tool!(OpendocChunkForEmbeddingTool, "opendoc_chunk_for_embedding", "Chunk document content using smart strategies (fixed token count, heading, page) for RAG input pipelines.", ChunkForEmbeddingParams, |p: ChunkForEmbeddingParams| {
    get_server().chunk_for_embedding(p.file_path, p.strategy, p.max_tokens, p.overlap)
});

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
define_opendoc_tool!(OpendocFillTemplateTool, "opendoc_fill_template", "Fill template placeholders in a document using a JSON context (supports nested structures, conditionals, loops).", FillTemplateParams, |p: FillTemplateParams| {
    get_server().fill_template(p.file_path, p.variables, p.password)
});

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
define_opendoc_tool!(OpendocValidateDocumentTool, "opendoc_validate_document", "Validate document integrity and return structural statistics (character count, paragraphs, sections, tables, images).", ValidateDocumentParams, |p: ValidateDocumentParams| {
    get_server().validate_document(p.file_path, p.password)
});

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
    #[schemars(description = "The target domain template: 'legal', 'financial', or 'timeline'")]
    pub template_type: String,
}
define_opendoc_tool!(
    OpendocExtractStructuredMetadataTool,
    "opendoc_extract_structured_metadata",
    "Extract structured domain metadata (legal, financial, timeline) using rule-based parsing.",
    ExtractStructuredMetadataParams,
    |p: ExtractStructuredMetadataParams| {
        get_server().extract_structured_metadata(p.file_path, p.template_type)
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
define_opendoc_tool!(OpendocConvertTool, "opendoc_convert", "Convert a document from one format to another (e.g. DOCX to PDF, HTML to XLSX, PDF to Markdown).", ConvertParams, |p: ConvertParams| {
    get_server().convert(p.source, p.target_format, p.output, p.password)
});

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
define_opendoc_tool!(OpendocSplitPdfTool, "opendoc_split_pdf", "Split a PDF document into a subset of pages specified by a start and end page (1-based, inclusive).", SplitPdfParams, |p: SplitPdfParams| {
    get_server().split_pdf(p.file_path, p.output_path, p.start_page, p.end_page, p.password)
});

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
define_opendoc_tool!(OpendocCreateFormattedPdfTool, "opendoc_create_formatted_pdf", "Create a highly structured multi-page PDF document with automatic word-wrap, headings, tables, margins, and page numbers.", CreateFormattedPdfParams, |p: CreateFormattedPdfParams| {
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
        p.margin_right
    )
});

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
define_opendoc_tool!(OpendocAnalyzeDocumentComplexityTool, "opendoc_analyze_document_complexity", "Analyze document properties to determine processing complexity (text density, scanned page heuristics, OCR recommendations).", AnalyzeDocumentComplexityParams, |p: AnalyzeDocumentComplexityParams| {
    get_server().analyze_document_complexity(p.file_path)
});

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
            "properties": {}
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
define_opendoc_tool!(OpendocRenderDocumentPagesTool, "opendoc_render_document_pages", "Render PDF/Office document pages directly into high-fidelity image paths for AI vision inspection.", RenderDocumentPagesParams, |p: RenderDocumentPagesParams| {
    get_server().render_document_pages(p.file_path, p.output_dir, p.dpi, p.pages)
});

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
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[tokio::test]
    async fn test_opendoc_check_ocr_available() {
        let tool = OpendocCheckOcrAvailableTool;
        assert_eq!(tool.name(), "opendoc_check_ocr_available");
        let result = tool.call(&serde_json::json!({})).await;
        assert!(result.is_ok(), "Expected tool call to succeed");
        let val = result.unwrap();
        assert!(val.get("available").is_some());
    }

    #[test]
    fn test_server_init() {
        let server = get_server();
        let res = server.check_ocr_available();
        assert!(!res.is_empty());
    }
}
