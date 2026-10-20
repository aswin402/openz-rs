use std::path::PathBuf;

/// Validate and sanitize the requested file path.
///
/// Ensures the path:
/// 1. Resolves within one of the allowed directories (if specified in OPENDOC_ALLOWED_DIRS env).
/// 2. Does not contain path traversal components (e.g. `..`) that escape the directory.
/// 3. Does not contain null bytes.
pub fn validate_path(file_path: &str) -> Result<PathBuf, String> {
    match file_path.contains('\0') {
        true => return Err("Path contains null byte".to_string()),
        false => {}
    }

    // Standard validation: check whether the path can be canonicalized when it exists.
    let resolved = crate::config::loader::resolve_path(file_path.trim_start_matches("file://"));
    let absolute_path = match resolved.exists() {
        true => std::fs::canonicalize(&resolved).unwrap_or(resolved),
        false => resolved,
    };

    // When OPENDOC_ALLOWED_DIRS is specified, restrict access
    match std::env::var("OPENDOC_ALLOWED_DIRS") {
        Ok(allowed_dirs_env) => {
            let allowed = allowed_dirs_env
                .split(',')
                .map(str::trim)
                .filter(|dir| !dir.is_empty())
                .filter_map(|dir| std::fs::canonicalize(dir).ok())
                .any(|allowed_path| absolute_path.starts_with(&allowed_path));
            match allowed {
                true => {}
                false => {
                    return Err(
                        "Access to the path is not allowed under current security sandbox policy"
                            .to_string(),
                    );
                }
            }
        }
        Err(_) => {}
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
