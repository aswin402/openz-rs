use std::fs;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFileResult {
    pub path: String,
    pub format: String,
    pub status: String,
    pub size_bytes: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveDigestResult {
    pub success: bool,
    pub extracted_count: usize,
    pub parsed_files: Vec<ExtractedFileResult>,
    pub digest_path: String,
    pub output_dir: String,
}

/// Extract all files in a zip archive into a target directory.
fn extract_zip_to_dir(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip archive: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file from zip: {e}"))?;
        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create folder: {e}"))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)
                        .map_err(|e| format!("Failed to create folder: {e}"))?;
                }
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create output file: {e}"))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write output file: {e}"))?;
        }
    }
    Ok(())
}

/// Recursively extract nested zip files.
fn extract_recursive(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    // 1. Extract the initial zip file
    extract_zip_to_dir(zip_path, target_dir)?;

    // 2. Scan for nested zips and extract them recursively
    let mut found_zip = true;
    let mut processed_zips = std::collections::HashSet::new();

    while found_zip {
        found_zip = false;
        let mut zips_to_process = Vec::new();

        fn find_zips(dir: &Path, list: &mut Vec<PathBuf>) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        find_zips(&path, list);
                    } else if path.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("zip")).unwrap_or(false) {
                        list.push(path);
                    }
                }
            }
        }

        find_zips(target_dir, &mut zips_to_process);

        for zip_file in zips_to_process {
            if processed_zips.contains(&zip_file) {
                continue;
            }
            processed_zips.insert(zip_file.clone());

            let stem = zip_file.file_stem().unwrap_or_default().to_string_lossy();
            let parent = zip_file.parent().unwrap_or(target_dir);
            let nested_out = parent.join(format!("{}_extracted", stem));

            if let Err(e) = extract_zip_to_dir(&zip_file, &nested_out) {
                tracing::warn!("Failed to extract nested zip {:?}: {}", zip_file, e);
            } else {
                let _ = fs::remove_file(&zip_file);
                found_zip = true;
                break;
            }
        }
    }

    Ok(())
}

/// Recursively list all files in a directory.
fn list_files_recursive(dir: &Path, list: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                list_files_recursive(&path, list);
            } else {
                list.push(path);
            }
        }
    }
}

/// Generate the structured Markdown report of all extracted and parsed files.
fn generate_digest_report(
    archive_name: &str,
    parsed_files: &[ExtractedFileResult],
    file_contents: &[(String, String)],
) -> String {
    let mut report = String::new();
    report.push_str(&format!("# Archive Extract Digest: {}\n\n", archive_name));
    
    report.push_str("## Extraction Summary\n");
    report.push_str(&format!("- **Total Extracted & Processed Files**: {}\n", parsed_files.len()));
    let success_count = parsed_files.iter().filter(|f| f.status == "success").count();
    report.push_str(&format!("- **Successfully Parsed Documents**: {}\n\n", success_count));

    report.push_str("### File List & Status\n\n");
    report.push_str("| File Path | Format | Status | Details |\n");
    report.push_str("|---|---|---|---|\n");
    for f in parsed_files {
        let size_str = f.size_bytes.map(|s| format!("{} bytes", s)).unwrap_or_else(|| "-".to_string());
        if f.status == "success" {
            report.push_str(&format!("| `{}` | `{}` | ✅ Success | {} |\n", f.path, f.format, size_str));
        } else {
            let err_msg = f.error.as_deref().unwrap_or("Unknown error");
            report.push_str(&format!("| `{}` | `{}` | ❌ Failed | {} |\n", f.path, f.format, err_msg));
        }
    }
    report.push_str("\n---\n\n");

    if !file_contents.is_empty() {
        report.push_str("## Document Contents\n\n");
        for (rel_path, content) in file_contents {
            let anchor = rel_path
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>();
            report.push_str(&format!("### File: {}\n", rel_path));
            report.push_str(&format!("<a name=\"{}\"></a>\n\n", anchor));
            report.push_str(content);
            report.push_str("\n\n---\n\n");
        }
    }

    report
}

/// Recursively extract a zip archive, parse all documents inside it into Markdown,
/// and compile a structured Markdown report.
pub fn process_archive_digest(
    archive_path: &str,
    output_dir: Option<&str>,
) -> Result<ArchiveDigestResult, String> {
    let archive_file_path = Path::new(archive_path);
    if !archive_file_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path));
    }

    let archive_name = archive_file_path.file_name().unwrap_or_default().to_string_lossy();

    // Determine target output directory
    let target_dir = match output_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            let sys_temp = std::env::temp_dir();
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            sys_temp.join(format!("opendoc_archive_extract_{}_{}", id, std::process::id()))
        }
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    // 1. Recursively extract all nested zip files
    extract_recursive(archive_file_path, &target_dir)?;

    // 2. Scan and list all files in the extracted directory tree
    let mut files = Vec::new();
    list_files_recursive(&target_dir, &mut files);

    let mut parsed_results = Vec::new();
    let mut file_contents = Vec::new();

    // 3. Process each file
    for file_path in &files {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        
        // Skip hidden files, system files, or markdown digests/directories
        let filename = file_path.file_name().unwrap_or_default().to_string_lossy();
        if filename.starts_with('.') || filename == "digest.md" {
            continue;
        }

        // Supported document formats
        let is_doc = matches!(
            ext.as_str(),
            "docx" | "doc" | "pptx" | "ppt" | "pdf" | "xlsx" | "xls" | "html" | "htm" | "md" | "markdown" | "csv" | "txt" | "text"
        );

        if !is_doc {
            continue;
        }

        // Relative path to include in the report
        let rel_path = match file_path.strip_prefix(&target_dir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => filename.to_string(),
        };

        match crate::handlers::load_to_ir(&file_path.to_string_lossy()) {
            Ok(doc) => {
                let markdown_content = doc.to_markdown();
                parsed_results.push(ExtractedFileResult {
                    path: rel_path.clone(),
                    format: ext.clone(),
                    status: "success".to_string(),
                    size_bytes: Some(markdown_content.len()),
                    error: None,
                });
                file_contents.push((rel_path, markdown_content));
            }
            Err(e) => {
                parsed_results.push(ExtractedFileResult {
                    path: rel_path,
                    format: ext,
                    status: "failed".to_string(),
                    size_bytes: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    // 4. Generate the compiled Markdown digest report
    let digest_text = generate_digest_report(&archive_name, &parsed_results, &file_contents);
    let digest_file_path = target_dir.join("digest.md");
    fs::write(&digest_file_path, &digest_text)
        .map_err(|e| format!("Failed to write digest report: {e}"))?;

    Ok(ArchiveDigestResult {
        success: true,
        extracted_count: parsed_results.len(),
        parsed_files: parsed_results,
        digest_path: digest_file_path.to_string_lossy().to_string(),
        output_dir: target_dir.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_digest_lifecycle() {
        let temp_dir = std::env::temp_dir();
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let test_root = temp_dir.join(format!("test_archive_{}", id));
        let _ = fs::create_dir_all(&test_root);

        // 1. Create a zip archive using zip writer
        let zip_path = test_root.join("test_archive.zip");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        // Add a text document
        zip.start_file("doc1.txt", options).unwrap();
        use std::io::Write;
        zip.write_all(b"Hello world from txt document").unwrap();

        // Add a folder and document
        zip.start_file("folder/doc2.md", options).unwrap();
        zip.write_all(b"# MD Header\nSome markdown text content").unwrap();

        zip.finish().unwrap();

        // 2. Process digest
        let out_dir = test_root.join("extracted");
        let result = process_archive_digest(
            zip_path.to_str().unwrap(),
            Some(out_dir.to_str().unwrap()),
        ).unwrap();

        assert!(result.success);
        assert_eq!(result.extracted_count, 2);
        
        let digest_path = Path::new(&result.digest_path);
        assert!(digest_path.exists());

        let digest_content = fs::read_to_string(digest_path).unwrap();
        assert!(digest_content.contains("test_archive.zip"));
        assert!(digest_content.contains("doc1.txt"));
        assert!(digest_content.contains("folder/doc2.md"));
        assert!(digest_content.contains("Hello world from txt document"));
        assert!(digest_content.contains("Some markdown text content"));

        // Clean up
        let _ = fs::remove_dir_all(test_root);
    }
}
