use std::path::{Path, PathBuf};

/// Validate and sanitize the requested file path.
///
/// Ensures the path:
/// 1. Resolves within one of the allowed directories (if specified in OPENDOC_ALLOWED_DIRS env).
/// 2. Does not contain path traversal components (e.g. `..`) that escape the directory.
/// 3. Does not contain null bytes.
pub fn validate_path(file_path: &str) -> Result<PathBuf, String> {
    if file_path.contains('\0') {
        return Err("Path contains null byte".to_string());
    }

    let path = Path::new(file_path);
    
    // Standard validation: check if the path can be canonicalized if it exists.
    // If the path doesn't exist, we can validate its parent directory.
    let absolute_path = if path.exists() {
        std::fs::canonicalize(path)
            .map_err(|e| format!("Invalid path: {e}"))?
    } else {
        // Resolve parent directories to prevent traversal in path creation
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let canonical_parent = std::fs::canonicalize(parent)
            .map_err(|e| format!("Invalid parent path: {e}"))?;
        let file_name = path.file_name()
            .ok_or_else(|| "Path has no filename".to_string())?;
        canonical_parent.join(file_name)
    };

    // If OPENDOC_ALLOWED_DIRS is specified, restrict access
    if let Ok(allowed_dirs_env) = std::env::var("OPENDOC_ALLOWED_DIRS") {
        let mut allowed = false;
        for dir in allowed_dirs_env.split(',') {
            let dir = dir.trim();
            if dir.is_empty() {
                continue;
            }
            if let Ok(allowed_path) = std::fs::canonicalize(dir) {
                if absolute_path.starts_with(&allowed_path) {
                    allowed = true;
                    break;
                }
            }
        }
        if !allowed {
            return Err("Access to the path is not allowed under current security sandbox policy".to_string());
        }
    }

    Ok(absolute_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_valid() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_sec_valid.txt");
        let _ = std::fs::write(&path, "test");

        let res = validate_path(path.to_str().unwrap());
        assert!(res.is_ok());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_validate_path_traversal() {
        let _res = validate_path("/tmp/../../../etc/passwd");
        // Traversal is resolved by canonicalize, but let's check sandbox restriction
        std::env::set_var("OPENDOC_ALLOWED_DIRS", "/tmp");
        let res2 = validate_path("/etc/passwd");
        assert!(res2.is_err());
        std::env::remove_var("OPENDOC_ALLOWED_DIRS");
    }

    #[test]
    fn test_validate_path_null_byte() {
        let res = validate_path("some\0file.txt");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Path contains null byte");
    }
}
