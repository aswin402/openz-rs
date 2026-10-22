fn workspace_file(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

#[test]
fn release_version_surfaces_match_cargo_package_version() {
    let version = env!("CARGO_PKG_VERSION");
    let readme = workspace_file("README.md");
    let onpkg = workspace_file("onpkg.json");
    let changelog = workspace_file("CHANGELOG.md");
    let cli_changelog = workspace_file("src/cli/changelog.rs");

    assert!(
        readme.contains(&format!("badge/version-{version}-orange.svg"))
            || readme.contains(&format!("alt=\"Version {version}\"")),
        "README.md badge must match Cargo package version {version}"
    );
    assert!(
        onpkg.contains(&format!("\"version\": \"{version}\"")),
        "onpkg.json project.version must match Cargo package version {version}"
    );
    assert!(
        changelog.contains(&format!("### v{version} (Latest Release)")),
        "CHANGELOG.md latest release heading must match Cargo package version {version}"
    );
    assert!(
        cli_changelog.contains("env!(\"CARGO_PKG_VERSION\")"),
        "CLI changelog current release must derive from CARGO_PKG_VERSION instead of a hardcoded version"
    );
    assert!(
        cli_changelog.contains("Current Release"),
        "CLI changelog must still render a current release label"
    );
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => collect_rs_files(&path, out),
            false => match path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                true => out.push(path),
                false => {}
            },
        }
    }
}

fn raw_string_hashes(bytes: &[u8], i: usize) -> Option<usize> {
    let prev_ok = match i == 0 {
        true => true,
        false => {
            let prev = bytes[i - 1];
            !is_ident_char(prev)
                || ((prev == b'b' || prev == b'c') && (i < 2 || !is_ident_char(bytes[i - 2])))
        }
    };
    match prev_ok {
        false => None,
        true => {
            let hashes = bytes[i + 1..].iter().take_while(|&&b| b == b'#').count();
            match bytes.get(i + 1 + hashes) == Some(&b'"') {
                true => Some(hashes),
                false => None,
            }
        }
    }
}

fn scan_rust_source_for_forbidden_tokens(src: &str) -> (usize, usize, usize) {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut bad_if = 0usize;
    let mut bad_else = 0usize;
    let mut bad_regex = 0usize;

    while i < n {
        let b = bytes[i];
        match (b, bytes.get(i + 1).copied()) {
            (b'/', Some(b'/')) => {
                i += 2;
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            (b'/', Some(b'*')) => {
                i += 2;
                let mut depth = 1usize;
                while i < n && depth > 0 {
                    match (bytes[i], bytes.get(i + 1).copied()) {
                        (b'/', Some(b'*')) => {
                            depth += 1;
                            i += 2;
                        }
                        (b'*', Some(b'/')) => {
                            depth -= 1;
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
            }
            (b'\'', Some(b'\\')) => {
                let close_offset = bytes
                    .get(i + 3..(i + 14).min(n))
                    .and_then(|slice| slice.iter().position(|&c| c == b'\''));
                match close_offset {
                    Some(offset) => i += 4 + offset,
                    None => i += 1,
                }
            }
            (b'\'', _) => match bytes.get(i + 2) == Some(&b'\'') {
                true => i += 3,
                false => i += 1,
            },
            (b'"', _) => {
                i += 1;
                while i < n {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            (b'r', _) => match raw_string_hashes(bytes, i) {
                Some(hashes) => {
                    i += 2 + hashes;
                    while i < n {
                        let is_end = bytes[i] == b'"'
                            && bytes[i + 1..].iter().take_while(|&&c| c == b'#').count()
                                >= hashes;
                        match is_end {
                            true => {
                                i += 1 + hashes;
                                break;
                            }
                            false => i += 1,
                        }
                    }
                }
                None => {
                    let len = bytes[i..].iter().take_while(|&&c| is_ident_char(c)).count();
                    let ident = &src[i..i + len];
                    match ident {
                        "regex" | "Regex" => bad_regex += 1,
                        _ => {}
                    }
                    i += len;
                }
            },
            _ => match is_ident_start(b) {
                true => {
                    let len = bytes[i..].iter().take_while(|&&c| is_ident_char(c)).count();
                    let ident = &src[i..i + len];
                    match ident {
                        "if" => bad_if += 1,
                        "else" => bad_else += 1,
                        "regex" | "Regex" => bad_regex += 1,
                        _ => {}
                    }
                    i += len;
                }
                false => i += 1,
            },
        }
    }

    (bad_if, bad_else, bad_regex)
}

#[test]
fn codebase_enforces_zero_if_else_and_zero_regex() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut rs_files = Vec::new();
    collect_rs_files(&src_dir, &mut rs_files);
    rs_files.sort();

    assert!(
        rs_files.len() >= 615,
        "expected at least 615 Rust source files in src/, found {}",
        rs_files.len()
    );

    let mut violations = Vec::new();
    for path in &rs_files {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let (bad_if, bad_else, bad_regex) = scan_rust_source_for_forbidden_tokens(&content);
        match (bad_if, bad_else, bad_regex) {
            (0, 0, 0) => {}
            _ => {
                let rel = path.strip_prefix(manifest_dir).unwrap_or(path);
                violations.push(format!(
                    "{}: if={bad_if}, else={bad_else}, regex={bad_regex}",
                    rel.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Zero-if/else and zero-regex invariant violated in {} file(s):\n{}",
        violations.len(),
        violations.join("\n")
    );

    let cargo_toml = workspace_file("Cargo.toml");
    assert!(
        !cargo_toml.lines().any(|line| line.trim_start().starts_with("regex =")),
        "Cargo.toml must not declare regex as a direct dependency"
    );
}
