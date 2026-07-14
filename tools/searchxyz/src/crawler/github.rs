use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tokio::sync::Mutex;

use crate::error::SearchXyzError;
use crate::extractor::ExtractedContent;
use crate::graph::KnowledgeGraph;
use crate::index::SearchIndex;

const DEFAULT_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "go", "c", "cpp", "h", "java", "swift", "kt", "sh", "pl", "rb", "php",
    "md", "txt", "rst", "adoc", "toml", "json", "yaml", "yml", "ini", "conf",
];

const DEFAULT_EXCLUDE_PATHS: &[&str] = &[
    ".git",
    ".github",
    "target",
    "node_modules",
    "dist",
    "build",
    "vendor",
    "bin",
    "obj",
];

const MAX_FILE_SIZE_BYTES: u64 = 256 * 1024; // 256 KB
const DEFAULT_MAX_REPO_FILES: usize = 2_000;
const MAX_REPO_FILES_CAP: usize = 10_000;
const DEFAULT_MAX_REPO_BYTES: u64 = 20 * 1024 * 1024;
const MAX_REPO_BYTES_CAP: u64 = 200 * 1024 * 1024;
const DEFAULT_GIT_TIMEOUT_SECS: u64 = 60;
const MAX_GIT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy)]
pub struct GithubIngestLimits {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub git_timeout_secs: u64,
}

impl Default for GithubIngestLimits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_REPO_FILES,
            max_total_bytes: DEFAULT_MAX_REPO_BYTES,
            git_timeout_secs: DEFAULT_GIT_TIMEOUT_SECS,
        }
    }
}

impl GithubIngestLimits {
    pub fn from_options(
        max_files: Option<usize>,
        max_total_bytes: Option<u64>,
        git_timeout_secs: Option<u64>,
    ) -> Self {
        Self {
            max_files: max_files
                .unwrap_or(DEFAULT_MAX_REPO_FILES)
                .clamp(1, MAX_REPO_FILES_CAP),
            max_total_bytes: max_total_bytes
                .unwrap_or(DEFAULT_MAX_REPO_BYTES)
                .clamp(1, MAX_REPO_BYTES_CAP),
            git_timeout_secs: git_timeout_secs
                .unwrap_or(DEFAULT_GIT_TIMEOUT_SECS)
                .clamp(1, MAX_GIT_TIMEOUT_SECS),
        }
    }
}

fn run_command_with_timeout(
    cmd: &mut Command,
    timeout_secs: u64,
    url: &str,
    action: &str,
) -> Result<Output, SearchXyzError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| SearchXyzError::CrawlFailed {
        url: url.to_string(),
        reason: format!("Failed to spawn {action}: {e}"),
    })?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        if child
            .try_wait()
            .map_err(|e| SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Failed while waiting for {action}: {e}"),
            })?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|e| SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!("Failed to collect {action} output: {e}"),
                });
        }

        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("{action} timed out after {timeout_secs}s"),
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn enforce_ingest_limits(
    repo_dir: &Path,
    relative_files: &[PathBuf],
    limits: &GithubIngestLimits,
    url: &str,
) -> Result<(), SearchXyzError> {
    if relative_files.len() > limits.max_files {
        return Err(SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: format!(
                "GitHub ingest file count limit exceeded: files={}, max_files={}",
                relative_files.len(),
                limits.max_files
            ),
        });
    }

    let mut total_bytes = 0_u64;
    for rel in relative_files {
        let path = repo_dir.join(rel);
        if let Ok(metadata) = fs::metadata(&path) {
            total_bytes = total_bytes.saturating_add(metadata.len());
            if total_bytes > limits.max_total_bytes {
                return Err(SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!(
                        "GitHub ingest byte limit exceeded: bytes={}, max_total_bytes={}",
                        total_bytes, limits.max_total_bytes
                    ),
                });
            }
        }
    }

    Ok(())
}

/// Parse a GitHub URL to extract Owner, Repo, and optional Branch
pub fn parse_github_url(url: &str) -> Option<(String, String, Option<String>)> {
    let parsed = url::Url::parse(url).ok()?;

    // Check hostname
    let host = parsed.host_str()?;
    if host != "github.com" && host != "www.github.com" {
        return None;
    }

    let mut segments = parsed.path_segments()?;
    let owner = segments.next()?.to_string();
    let mut repo = segments.next()?.to_string();

    if repo.ends_with(".git") {
        repo = repo[..repo.len() - 4].to_string();
    }

    // Check if there is a branch specified, e.g. /tree/branch-name or /blob/branch-name
    let next_segment = segments.next();
    let branch = if next_segment == Some("tree") || next_segment == Some("blob") {
        segments.next().map(|b| b.to_string())
    } else {
        None
    };

    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some((owner, repo, branch))
}

/// Recursively walk the directory, filtering files by extension and path exclusion
fn visit_dirs(
    dir: &Path,
    base_dir: &Path,
    include_exts: &[String],
    exclude_paths: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check if path should be excluded
            if let Ok(rel_path) = path.strip_prefix(base_dir) {
                if exclude_paths
                    .iter()
                    .any(|ex| rel_path.components().any(|c| c.as_os_str() == ex.as_str()))
                {
                    continue;
                }
            }

            if path.is_dir() {
                visit_dirs(&path, base_dir, include_exts, exclude_paths, files)?;
            } else if path.is_file() {
                // Check file size first
                if let Ok(metadata) = fs::metadata(&path) {
                    if metadata.len() > MAX_FILE_SIZE_BYTES {
                        continue;
                    }
                }

                // Filter by extension
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if include_exts.iter().any(|ie| ie.to_lowercase() == ext_lower) {
                        files.push(path);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Clone a repository using git and index its contents
pub async fn clone_and_index_repo(
    index: &SearchIndex,
    graph: &Mutex<KnowledgeGraph>,
    url: &str,
    branch: Option<&str>,
    include_exts: Option<&[String]>,
    exclude_paths: Option<&[String]>,
    limits: Option<GithubIngestLimits>,
) -> Result<String, SearchXyzError> {
    let limits = limits.unwrap_or_default();

    // 1. Parse GitHub URL
    let (owner, repo_name, parsed_branch) =
        parse_github_url(url).ok_or_else(|| SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: "Invalid GitHub repository URL".to_string(),
        })?;

    let resolved_branch = branch.or(parsed_branch.as_deref());
    let repo_base_url = format!("https://github.com/{}/{}", owner, repo_name);

    // Resolve target repository directory name (persistent directory)
    let branch_sanitized = resolved_branch
        .unwrap_or("default")
        .replace(['/', '\\'], "_");
    let repo_dir_name = format!("{}_{}_{}", owner, repo_name, branch_sanitized);
    let repo_dir = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".searchxyz")
        .join("repos")
        .join(repo_dir_name);

    let include_exts_vec: Vec<String> = match include_exts {
        Some(exts) => exts.iter().map(|s| s.to_string()).collect(),
        None => DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
    };

    let exclude_paths_vec: Vec<String> = match exclude_paths {
        Some(paths) => paths.iter().map(|s| s.to_string()).collect(),
        None => DEFAULT_EXCLUDE_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };

    let mut files_to_delete = Vec::new();
    let mut files_to_upsert = Vec::new();
    let mut is_incremental = false;

    let repo_git_dir = repo_dir.join(".git");
    if repo_dir.exists() && repo_git_dir.exists() {
        is_incremental = true;
        // 1. Get active branch
        let mut active_branch_cmd = Command::new("git");
        active_branch_cmd
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_dir);
        let active_branch_output = run_command_with_timeout(
            &mut active_branch_cmd,
            limits.git_timeout_secs,
            url,
            "git rev-parse --abbrev-ref",
        )?;
        if !active_branch_output.status.success() {
            let stderr = String::from_utf8_lossy(&active_branch_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git rev-parse --abbrev-ref HEAD failed: {}", stderr.trim()),
            });
        }
        let active_branch = String::from_utf8_lossy(&active_branch_output.stdout)
            .trim()
            .to_string();

        // 2. Get old commit hash
        let mut old_commit_cmd = Command::new("git");
        old_commit_cmd
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_dir);
        let old_commit_output = run_command_with_timeout(
            &mut old_commit_cmd,
            limits.git_timeout_secs,
            url,
            "git rev-parse HEAD",
        )?;
        if !old_commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&old_commit_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git rev-parse HEAD (old) failed: {}", stderr.trim()),
            });
        }
        let old_commit = String::from_utf8_lossy(&old_commit_output.stdout)
            .trim()
            .to_string();
        if old_commit.is_empty() {
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: "git rev-parse HEAD (old) returned an empty commit hash".to_string(),
            });
        }

        // 3. Fetch from remote
        tracing::info!(repo = %repo_base_url, branch = %active_branch, "Fetching latest changes");
        let mut fetch_cmd = Command::new("git");
        fetch_cmd
            .args(["fetch", "--depth", "1", "origin", &active_branch])
            .current_dir(&repo_dir);
        let fetch_output =
            run_command_with_timeout(&mut fetch_cmd, limits.git_timeout_secs, url, "git fetch")?;
        if !fetch_output.status.success() {
            let stderr = String::from_utf8_lossy(&fetch_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git fetch failed: {}", stderr.trim()),
            });
        }

        // 4. Reset hard to FETCH_HEAD
        let mut reset_cmd = Command::new("git");
        reset_cmd
            .args(["reset", "--hard", "FETCH_HEAD"])
            .current_dir(&repo_dir);
        let reset_output =
            run_command_with_timeout(&mut reset_cmd, limits.git_timeout_secs, url, "git reset")?;
        if !reset_output.status.success() {
            let stderr = String::from_utf8_lossy(&reset_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git reset failed: {}", stderr.trim()),
            });
        }

        // 5. Get new commit hash
        let mut rev_parse_cmd = Command::new("git");
        rev_parse_cmd
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_dir);
        let new_commit_output = run_command_with_timeout(
            &mut rev_parse_cmd,
            limits.git_timeout_secs,
            url,
            "git rev-parse",
        )?;
        if !new_commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&new_commit_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git rev-parse HEAD (new) failed: {}", stderr.trim()),
            });
        }
        let new_commit = String::from_utf8_lossy(&new_commit_output.stdout)
            .trim()
            .to_string();
        if new_commit.is_empty() {
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: "git rev-parse HEAD (new) returned an empty commit hash".to_string(),
            });
        }

        // 6. Get diff list of files
        let mut diff_cmd = Command::new("git");
        diff_cmd
            .args([
                "diff",
                "--no-renames",
                "--name-status",
                &old_commit,
                &new_commit,
            ])
            .current_dir(&repo_dir);
        let diff_output =
            run_command_with_timeout(&mut diff_cmd, limits.git_timeout_secs, url, "git diff")?;
        if !diff_output.status.success() {
            let stderr = String::from_utf8_lossy(&diff_output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git diff failed: {}", stderr.trim()),
            });
        }

        let diff_str = String::from_utf8_lossy(&diff_output.stdout);
        for line in diff_str.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                let status = parts[0];
                let file_path = parts[1];

                let rel_path = Path::new(file_path);
                let is_excluded = exclude_paths_vec
                    .iter()
                    .any(|ex| rel_path.components().any(|c| c.as_os_str() == ex.as_str()));

                if is_excluded {
                    continue;
                }

                let path = repo_dir.join(file_path);
                let exceeds_limit = if status != "D" {
                    if let Ok(metadata) = fs::metadata(&path) {
                        metadata.len() > MAX_FILE_SIZE_BYTES
                    } else {
                        false
                    }
                } else {
                    false
                };

                if exceeds_limit {
                    if status == "M" {
                        // Purge any stale version from index since the new version exceeds size limit
                        files_to_delete.push(file_path.to_string());
                    }
                    continue;
                }

                let has_valid_ext = rel_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| {
                        let ext_lower = ext.to_lowercase();
                        include_exts_vec
                            .iter()
                            .any(|ie| ie.to_lowercase() == ext_lower)
                    })
                    .unwrap_or(false);

                if !has_valid_ext {
                    continue;
                }

                match status {
                    "D" => {
                        files_to_delete.push(file_path.to_string());
                    }
                    "M" => {
                        files_to_delete.push(file_path.to_string());
                        files_to_upsert.push(file_path.to_string());
                    }
                    "A" => {
                        files_to_upsert.push(file_path.to_string());
                    }
                    _ => {}
                }
            }
        }
    } else {
        // Clean clone
        if repo_dir.exists() {
            let _ = fs::remove_dir_all(&repo_dir);
        }
        fs::create_dir_all(&repo_dir).map_err(|e| SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: format!("Failed to create repository directory: {}", e),
        })?;

        let mut cmd = Command::new("git");
        cmd.arg("clone").arg("--depth").arg("1");

        if let Some(b) = resolved_branch {
            if !b
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
            {
                return Err(SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!("Invalid/unsafe branch name: {}", b),
                });
            }
            cmd.arg("--branch").arg(b);
        }

        cmd.arg(&repo_base_url).arg(&repo_dir);

        tracing::info!(repo = %repo_base_url, branch = ?resolved_branch, dest = ?repo_dir, "Cloning repository");
        let output = run_command_with_timeout(&mut cmd, limits.git_timeout_secs, url, "git clone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("git clone failed: {}", stderr.trim()),
            });
        }

        let mut file_paths = Vec::new();
        if let Err(e) = visit_dirs(
            &repo_dir,
            &repo_dir,
            &include_exts_vec,
            &exclude_paths_vec,
            &mut file_paths,
        ) {
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Directory traversal failed: {}", e),
            });
        }

        for path in file_paths {
            if let Ok(rel_path) = path.strip_prefix(&repo_dir) {
                files_to_upsert.push(rel_path.to_string_lossy().to_string());
            }
        }
    }

    let budget_files: Vec<PathBuf> = files_to_upsert.iter().map(PathBuf::from).collect();
    enforce_ingest_limits(&repo_dir, &budget_files, &limits, url)?;

    // Process deletions
    let branch_for_url = resolved_branch.unwrap_or("main");

    for rel_path_str in &files_to_delete {
        let file_url = format!("{}/blob/{}/{}", repo_base_url, branch_for_url, rel_path_str);
        if let Err(e) = index.delete_document(&file_url).await {
            tracing::warn!(url = %file_url, error = %e, "Failed to delete file from search index");
        }
        {
            let mut g = graph.lock().await;
            g.prune_node(&file_url);
        }
    }

    // Process additions / updates
    let mut indexed_count = 0;
    let mut readme_content = String::new();
    let mut file_list_summary = String::new();

    for rel_path_str in &files_to_upsert {
        let path = repo_dir.join(rel_path_str);
        let file_content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let file_url = format!("{}/blob/{}/{}", repo_base_url, branch_for_url, rel_path_str);
        let file_title = format!("{} ({})", rel_path_str, repo_name);

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        let formatted_markdown = format!(
            "### File: `{}`\n\n```{}\n{}\n```",
            rel_path_str, ext, file_content
        );

        let extracted = ExtractedContent {
            url: file_url.clone(),
            title: file_title.clone(),
            description: format!("Source code file from {} repository", repo_name),
            content_markdown: formatted_markdown,
            links: Vec::new(),
        };

        if let Err(e) = index.add_document(&extracted, "github").await {
            tracing::warn!(url = %file_url, error = %e, "Failed to index GitHub file");
        }

        {
            let mut g = graph.lock().await;
            g.extract_heuristics(&file_url, &file_title, &file_content);
        }

        let file_name_lower = Path::new(rel_path_str)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_lowercase();
        if file_name_lower == "readme.md" && Path::new(rel_path_str).parent() == Some(Path::new(""))
        {
            readme_content = file_content.clone();
        }

        file_list_summary.push_str(&format!("- [`{}`]({})\n", rel_path_str, file_url));
        indexed_count += 1;
    }

    let summary = if is_incremental {
        let actual_deleted_count = files_to_delete
            .iter()
            .filter(|f| !files_to_upsert.contains(f))
            .count();
        let mut summary = format!(
            "# Codebase Synced (Incremental): {}/{}\n\n\
            **Source:** {}\n\
            **Branch:** {}\n\
            **Files Updated/Added:** {} files modified or added.\n\
            **Files Deleted:** {} files removed.\n\n",
            owner, repo_name, repo_base_url, branch_for_url, indexed_count, actual_deleted_count
        );

        if !readme_content.is_empty() {
            summary.push_str("## Repository README (Updated)\n\n");
            summary.push_str(&readme_content);
            summary.push_str("\n\n---\n\n");
        }

        if !file_list_summary.is_empty() {
            summary.push_str("## Updated/Added Files\n\n");
            summary.push_str(&file_list_summary);
        }
        summary
    } else {
        let mut summary = format!(
            "# Codebase Ingested: {}/{}\n\n\
            **Source:** {}\n\
            **Branch:** {}\n\
            **Files Indexed:** {} matching files successfully parsed and added to search index.\n\n",
            owner, repo_name, repo_base_url, branch_for_url, indexed_count
        );

        if !readme_content.is_empty() {
            summary.push_str("## Repository README\n\n");
            summary.push_str(&readme_content);
            summary.push_str("\n\n---\n\n");
        }

        summary.push_str("## Ingested Codebase Files\n\n");
        summary.push_str(&file_list_summary);
        summary
    };

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    struct EnvGuard {
        old_home: Option<String>,
        old_config_count: Option<String>,
        old_config_key_0: Option<String>,
        old_config_value_0: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref val) = self.old_home {
                std::env::set_var("HOME", val);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(ref val) = self.old_config_count {
                std::env::set_var("GIT_CONFIG_COUNT", val);
            } else {
                std::env::remove_var("GIT_CONFIG_COUNT");
            }
            if let Some(ref val) = self.old_config_key_0 {
                std::env::set_var("GIT_CONFIG_KEY_0", val);
            } else {
                std::env::remove_var("GIT_CONFIG_KEY_0");
            }
            if let Some(ref val) = self.old_config_value_0 {
                std::env::set_var("GIT_CONFIG_VALUE_0", val);
            } else {
                std::env::remove_var("GIT_CONFIG_VALUE_0");
            }
        }
    }

    #[test]
    fn test_parse_github_url() {
        assert_eq!(
            parse_github_url("https://github.com/tokio-rs/tokio"),
            Some(("tokio-rs".to_string(), "tokio".to_string(), None))
        );
        assert_eq!(
            parse_github_url("https://github.com/tokio-rs/tokio.git"),
            Some(("tokio-rs".to_string(), "tokio".to_string(), None))
        );
        assert_eq!(
            parse_github_url("https://github.com/tokio-rs/tokio/tree/v1.36.0"),
            Some((
                "tokio-rs".to_string(),
                "tokio".to_string(),
                Some("v1.36.0".to_string())
            ))
        );
        assert_eq!(
            parse_github_url("https://github.com/tokio-rs/tokio/blob/main/src/lib.rs"),
            Some((
                "tokio-rs".to_string(),
                "tokio".to_string(),
                Some("main".to_string())
            ))
        );
        assert_eq!(parse_github_url("https://google.com"), None);
    }

    #[test]
    fn test_visit_dirs() {
        let temp = std::env::temp_dir().join("test_visit_dirs_searchxyz");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let src = temp.join("src");
        fs::create_dir_all(&src).unwrap();

        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        fs::write(src.join("helper.rs"), "fn helper() {}").unwrap();
        fs::write(temp.join("README.md"), "# Test Repo").unwrap();

        // Ignored path
        let target = temp.join("target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("output.rs"), "some compiled output").unwrap();

        let include_exts: Vec<String> = vec!["rs".to_string(), "md".to_string()];
        let exclude_paths: Vec<String> = vec!["target".to_string()];

        let mut files = Vec::new();
        visit_dirs(&temp, &temp, &include_exts, &exclude_paths, &mut files).unwrap();

        let mut file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        file_names.sort();

        assert_eq!(
            file_names,
            vec![
                "README.md".to_string(),
                "helper.rs".to_string(),
                "main.rs".to_string()
            ]
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_enforce_ingest_limits_rejects_too_many_files() {
        let temp = std::env::temp_dir().join(format!(
            "test_github_ingest_limits_{}",
            rand::rng().random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("a.rs"), "fn a() {}").unwrap();
        fs::write(temp.join("b.rs"), "fn b() {}").unwrap();

        let files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let err = enforce_ingest_limits(
            &temp,
            &files,
            &GithubIngestLimits {
                max_files: 1,
                max_total_bytes: 1024,
                git_timeout_secs: 30,
            },
            "https://github.com/example/repo",
        )
        .unwrap_err();

        assert!(err.to_string().contains("file count limit"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn test_incremental_git_sync() {
        // Setup temporary directories
        let temp_dir = std::env::temp_dir().join(format!(
            "test_incremental_git_sync_home_{}",
            rand::rng().random::<u32>()
        ));
        let dummy_origin = std::env::temp_dir().join(format!(
            "test_incremental_git_sync_origin_{}",
            rand::rng().random::<u32>()
        ));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&dummy_origin);
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(&dummy_origin).unwrap();

        // Initialize dummy origin repo
        let run_git = |args: &[&str], dir: &Path| {
            let output = Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .expect("failed to execute git");
            assert!(
                output.status.success(),
                "git command {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };

        // Create main branch explicitly
        run_git(&["init", "-b", "main"], &dummy_origin);
        run_git(&["config", "user.name", "Test User"], &dummy_origin);
        run_git(&["config", "user.email", "test@example.com"], &dummy_origin);

        // Add initial files to dummy origin
        let file_a = dummy_origin.join("a.rs");
        let file_b = dummy_origin.join("b.rs");
        let readme = dummy_origin.join("README.md");
        fs::write(&file_a, "fn a() {}").unwrap();
        fs::write(&file_b, "fn b() {}").unwrap();
        fs::write(&readme, "# Dummy Project").unwrap();

        run_git(&["add", "."], &dummy_origin);
        run_git(&["commit", "-m", "initial commit"], &dummy_origin);

        // Prepare SearchIndex and KnowledgeGraph
        let index_dir = temp_dir.join("index");
        fs::create_dir_all(&index_dir).unwrap();
        let index_config = crate::config::IndexConfig {
            path: index_dir,
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };
        let index = SearchIndex::open(&index_config).unwrap();
        let graph = Mutex::new(KnowledgeGraph::new());

        let old_home = std::env::var("HOME").ok();
        let old_config_count = std::env::var("GIT_CONFIG_COUNT").ok();
        let old_config_key_0 = std::env::var("GIT_CONFIG_KEY_0").ok();
        let old_config_value_0 = std::env::var("GIT_CONFIG_VALUE_0").ok();

        let _env_guard = EnvGuard {
            old_home: old_home.clone(),
            old_config_count: old_config_count.clone(),
            old_config_key_0: old_config_key_0.clone(),
            old_config_value_0: old_config_value_0.clone(),
        };

        // Redirect home directory (so ~/.searchxyz is written into our temp_dir)
        std::env::set_var("HOME", &temp_dir);

        // Configure git to redirect github.com to our local dummy_origin
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        let local_url = dummy_origin.to_string_lossy().to_string();
        std::env::set_var("GIT_CONFIG_KEY_0", format!("url.{}.insteadOf", local_url));
        std::env::set_var(
            "GIT_CONFIG_VALUE_0",
            "https://github.com/dummy-owner/dummy-repo",
        );

        // First indexing (clean clone)
        let repo_url = "https://github.com/dummy-owner/dummy-repo";
        let result = clone_and_index_repo(&index, &graph, repo_url, Some("main"), None, None, None)
            .await
            .unwrap();

        println!("RESULT OF INDEXING:\n{}", result);

        index.reload().unwrap();

        // Assert initial index state
        let (docs, total) = index.list_documents(Some("github"), 100, 0).unwrap();
        // total is 4 because README.md is chunked into 2 docs (total 2) + a.rs (1) + b.rs (1) = 4
        assert_eq!(
            total, 4,
            "Expected 4 indexed document chunks initially, got: {:?}",
            docs
        );

        let mut urls: Vec<String> = docs.iter().map(|d| d.url.clone()).collect();
        urls.sort();

        let expected_a_url = format!("{}/blob/main/a.rs", repo_url);
        let expected_b_url = format!("{}/blob/main/b.rs", repo_url);
        let expected_readme_url = format!("{}/blob/main/README.md", repo_url);

        assert!(urls.contains(&expected_a_url));
        assert!(urls.contains(&expected_b_url));
        assert!(urls.contains(&format!("{}#chunk-0", expected_readme_url)));
        assert!(urls.contains(&format!("{}#chunk-1", expected_readme_url)));

        // Modify the dummy origin:
        // - Modify a.rs
        // - Delete b.rs
        // - Add c.rs
        fs::write(&file_a, "fn a_modified() {}").unwrap();
        fs::remove_file(&file_b).unwrap();
        let file_c = dummy_origin.join("c.rs");
        fs::write(&file_c, "fn c() {}").unwrap();

        run_git(&["add", "-A"], &dummy_origin);
        run_git(&["commit", "-m", "second commit"], &dummy_origin);

        // Run updating index
        let _result2 =
            clone_and_index_repo(&index, &graph, repo_url, Some("main"), None, None, None)
                .await
                .unwrap();

        index.reload().unwrap();

        // Verify changes in the search index
        let (docs2, total2) = index.list_documents(Some("github"), 100, 0).unwrap();
        let mut urls2: Vec<String> = docs2.iter().map(|d| d.url.clone()).collect();
        urls2.sort();

        let expected_c_url = format!("{}/blob/main/c.rs", repo_url);

        // total2 is 4 because README.md is chunked into 2 docs (total 2) + modified a.rs (1) + added c.rs (1) = 4
        assert_eq!(
            total2, 4,
            "Expected 4 indexed document chunks after update, got: {:?}",
            urls2
        );
        assert!(urls2.contains(&expected_a_url));
        assert!(
            !urls2.contains(&expected_b_url),
            "Deleted file b.rs should be gone"
        );
        assert!(
            urls2.contains(&expected_c_url),
            "New file c.rs should be indexed"
        );
        assert!(urls2.contains(&format!("{}#chunk-0", expected_readme_url)));
        assert!(urls2.contains(&format!("{}#chunk-1", expected_readme_url)));

        // Clean up env vars automatically via EnvGuard

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&dummy_origin);
    }
}
