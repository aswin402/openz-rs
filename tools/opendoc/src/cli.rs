//! CLI interface using clap.

#[cfg(feature = "cli")]
mod imp {
    use clap::{Parser, Subcommand};

    /// opendoc-mcp: Rust-native Document Intelligence Engine
    #[derive(Parser)]
    #[command(name = "opendoc")]
    #[command(about = "Document Intelligence for AI Agents", long_about = None)]
    pub struct Cli {
        #[command(subcommand)]
        pub command: Commands,
    }

    #[derive(Subcommand)]
    pub enum Commands {
        /// Start the MCP server (default)
        Serve {
            /// Log level
            #[arg(short, long, default_value = "info")]
            log_level: String,
        },
        /// Convert a document
        Convert {
            /// Source file path
            source: String,
            /// Target format (pdf, md, html, csv, txt, json)
            target: String,
            /// Output file path (optional)
            #[arg(short, long)]
            output: Option<String>,
        },
        /// Extract text from a document
        Extract {
            /// Source file path
            source: String,
            /// Output file path
            #[arg(short, long)]
            output: Option<String>,
        },
        /// Batch convert all files in a directory
        Batch {
            /// Input directory
            input_dir: String,
            /// Target format
            target: String,
            /// Output directory
            #[arg(short, long, default_value = "./output")]
            output_dir: String,
            /// File pattern (e.g., "*.docx")
            #[arg(short, long, default_value = "*")]
            pattern: String,
            /// Recursive traversal
            #[arg(short, long)]
            recursive: bool,
            /// Optional password for encrypted files
            #[arg(short, long)]
            password: Option<String>,
            /// Optional thread concurrency limit
            #[arg(short, long)]
            concurrency: Option<usize>,
        },
        /// Merge multiple PDFs
        Merge {
            /// Source files
            sources: Vec<String>,
            /// Output file
            #[arg(short, long)]
            output: String,
        },
        /// Validate a document
        Validate {
            /// File path
            path: String,
        },
        /// Show document info
        Info {
            /// File path
            path: String,
        },
        /// Document diff
        Diff {
            /// First file
            file_a: String,
            /// Second file
            file_b: String,
        },
        /// List supported formats
        Formats,
        /// Recursively unpack a ZIP archive and generate a Markdown digest report
        Digest {
            /// ZIP archive file path
            archive: String,
            /// Output directory (optional)
            #[arg(short, long)]
            output_dir: Option<String>,
        },
        /// Extract structured metadata (legal, financial, timeline)
        Metadata {
            /// File path to document
            path: String,
            /// Template type (legal, financial, timeline)
            template: String,
        },
        /// Validate a PDF file for PDF/A compliance
        ValidatePdfA {
            /// File path to PDF document
            path: String,
        },
    }

    pub fn run() -> anyhow::Result<()> {
        let cli = Cli::parse();
        match cli.command {
            Commands::Serve { log_level } => {
                let filter = tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));
                tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .init();
                tracing::info!("Starting opendoc-mcp server...");
                let server = crate::server::OpendocServer::new();
                tokio::runtime::Runtime::new()?.block_on(server.run())?;
            }
            Commands::Convert { source, target, output } => {
                let out = output.unwrap_or_else(|| {
                    let stem = std::path::Path::new(&source)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    format!("{}.{}", stem, target)
                });
                match crate::converters::convert(&source, &target, &out) {
                    Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Commands::Extract { source, output } => {
                let doc = crate::handlers::load_to_ir(&source)?;
                let text: Vec<&str> = doc.paragraphs.iter().map(|p| p.text.as_str()).collect();
                let content = text.join("\n");
                if let Some(out) = output {
                    std::fs::write(&out, &content)?;
                    println!("Extracted text to {}", out);
                } else {
                    println!("{}", content);
                }
            }
            Commands::Batch {
                input_dir,
                target,
                output_dir,
                pattern,
                recursive,
                password,
                concurrency,
            } => {
                std::fs::create_dir_all(&output_dir)?;
                let results = crate::batch::batch_convert_extended(
                    &input_dir,
                    &pattern,
                    &target,
                    &output_dir,
                    recursive,
                    password.as_deref(),
                    concurrency,
                );
                println!("{}", serde_json::to_string_pretty(&results).unwrap());
            }
            Commands::Merge { sources, output } => {
                let result = crate::handlers::pdf::merge_pdfs(&sources, &output);
                println!("{}", result);
            }
            Commands::Validate { path } => {
                let doc = crate::handlers::load_to_ir(&path)?;
                let result = crate::validators::validate_document(&doc);
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            }
            Commands::Info { path } => {
                let doc = crate::handlers::load_to_ir(&path)?;
                let info = serde_json::json!({
                    "format": doc.format,
                    "paragraphs": doc.paragraphs.len(),
                    "tables": doc.tables.len(),
                    "images": doc.images.len(),
                    "sections": doc.sections.len(),
                    "estimated_tokens": doc.estimate_tokens(),
                    "metadata": doc.metadata,
                });
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
            }
            Commands::Diff { file_a, file_b } => {
                let doc_a = crate::handlers::load_to_ir(&file_a)?;
                let doc_b = crate::handlers::load_to_ir(&file_b)?;
                let diff = crate::engine::diff::diff_documents(&doc_a, &doc_b);
                println!("{}", serde_json::to_string_pretty(&diff).unwrap());
            }
            Commands::Formats => {
                let formats = serde_json::json!({
                    "supported_formats": ["docx", "pptx", "pdf", "xlsx", "html", "md", "csv", "txt"],
                    "read": ["docx", "pptx", "pdf", "xlsx", "html", "md", "csv", "txt"],
                    "write": ["docx", "pdf", "txt", "json", "xlsx"],
                    "convert": {
                        "docx": ["pdf", "md", "html", "json"],
                        "pptx": ["md", "pdf", "json"],
                        "pdf": ["txt", "md", "json"],
                        "xlsx": ["csv", "json", "xlsx"],
                        "html": ["json", "xlsx"],
                        "md": ["json", "xlsx"],
                        "csv": ["json", "xlsx"],
                        "txt": ["json", "xlsx"]
                    }
                });
                println!("{}", serde_json::to_string_pretty(&formats).unwrap());
            }
            Commands::Digest { archive, output_dir } => {
                match crate::batch::archive::process_archive_digest(&archive, output_dir.as_deref()) {
                    Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Commands::Metadata { path, template } => {
                let doc = crate::handlers::load_to_ir(&path)?;
                match template.to_lowercase().as_str() {
                    "legal" => {
                        let res = crate::engine::extract::extract_legal(&doc);
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    }
                    "financial" => {
                        let res = crate::engine::extract::extract_financial(&doc);
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    }
                    "timeline" => {
                        let res = crate::engine::extract::extract_timeline(&doc);
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    }
                    other => eprintln!("Error: Unsupported template type '{}'. Supported: 'legal', 'financial', 'timeline'.", other),
                }
            }
            Commands::ValidatePdfA { path } => {
                match crate::validators::pdf_a::validate_pdf_a(std::path::Path::new(&path)) {
                    Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "cli"))]
mod imp {
    pub fn run() -> anyhow::Result<()> {
        anyhow::bail!("CLI feature not enabled (build with --features cli)")
    }
}

pub use imp::*;
