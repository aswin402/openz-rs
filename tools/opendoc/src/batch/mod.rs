//! Batch processing — process entire directories of documents.
//!
//! Uses rayon for parallel execution.

pub mod archive;

use rayon::prelude::*;
use std::path::Path;

/// Result of a single batch conversion operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchResult {
    pub file: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Helper function to walk directory recursively or flat.
fn walk_dir(dir: &Path, pattern: &str, recursive: bool) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    let target_ext = pattern.trim_start_matches('*').trim_start_matches('.');

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                if let Ok(mut sub_files) = walk_dir(&path, pattern, recursive) {
                    files.append(&mut sub_files);
                }
            }
        } else {
            let matches = if target_ext.is_empty() {
                path.extension().is_some()
            } else {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case(target_ext))
                    .unwrap_or(false)
            };
            if matches {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Batch conversion: convert all files matching a pattern (flat walk, no password, no concurrency limit).
pub fn batch_convert(
    input_dir: &str,
    pattern: &str,
    target_format: &str,
    output_dir: &str,
) -> Vec<BatchResult> {
    batch_convert_extended(input_dir, pattern, target_format, output_dir, false, None, None)
}

/// Batch conversion with advanced options (recursive traversal, password decryption, and threadpool control).
pub fn batch_convert_extended(
    input_dir: &str,
    pattern: &str,
    target_format: &str,
    output_dir: &str,
    recursive: bool,
    password: Option<&str>,
    concurrency: Option<usize>,
) -> Vec<BatchResult> {
    let input_path = Path::new(input_dir);
    let files = match walk_dir(input_path, pattern, recursive) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let run_map = || {
        files
            .par_iter()
            .map(|path| {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();

                // Reconstruct relative directory structure
                let output_path = match path.strip_prefix(input_path) {
                    Ok(rel_path) => {
                        let rel_parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
                        let out_parent = Path::new(output_dir).join(rel_parent);
                        let _ = std::fs::create_dir_all(&out_parent);
                        out_parent.join(format!("{}.{}", stem, target_format))
                    }
                    Err(_) => Path::new(output_dir).join(format!("{}.{}", stem, target_format)),
                };

                let result = crate::converters::convert_with_password(
                    path.to_str().unwrap_or(""),
                    target_format,
                    output_path.to_str().unwrap_or(""),
                    password,
                );

                match result {
                    Ok(conv) => BatchResult {
                        file: filename.to_string(),
                        success: true,
                        output: conv.output,
                        error: None,
                    },
                    Err(e) => BatchResult {
                        file: filename.to_string(),
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                    },
                }
            })
            .collect::<Vec<BatchResult>>()
    };

    if let Some(limit) = concurrency {
        if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(limit).build() {
            pool.install(run_map)
        } else {
            run_map()
        }
    } else {
        run_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_convert_recursive() {
        let dir = std::env::temp_dir();
        // Use an atomic/unique subdirectory path to avoid collisions
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        let batch_in = dir.join(format!("test_batch_in_{}", id));
        let batch_out = dir.join(format!("test_batch_out_{}", id));
        
        let _ = std::fs::create_dir_all(&batch_in);
        let _ = std::fs::create_dir_all(batch_in.join("subdir"));

        // Create a flat text file
        let file1 = batch_in.join("doc1.txt");
        std::fs::write(&file1, "Hello from doc1").unwrap();

        // Create a nested text file
        let file2 = batch_in.join("subdir").join("doc2.txt");
        std::fs::write(&file2, "Hello from doc2").unwrap();

        // Run batch conversion recursively converting txt to md
        let results = batch_convert_extended(
            batch_in.to_str().unwrap(),
            "*.txt",
            "md",
            batch_out.to_str().unwrap(),
            true, // recursive
            None,
            Some(2), // concurrency limit
        );

        assert_eq!(results.len(), 2);
        
        let output1 = batch_out.join("doc1.md");
        let output2 = batch_out.join("subdir").join("doc2.md");

        assert!(output1.exists());
        assert!(output2.exists());

        // Clean up
        let _ = std::fs::remove_dir_all(batch_in);
        let _ = std::fs::remove_dir_all(batch_out);
    }
}
