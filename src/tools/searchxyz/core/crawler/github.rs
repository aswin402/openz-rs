use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tokio::sync::Mutex;

use crate::tools::searchxyz::core::error::SearchXyzError;
use crate::tools::searchxyz::core::extractor::ExtractedContent;
use crate::tools::searchxyz::core::graph::KnowledgeGraph;
use crate::tools::searchxyz::core::index::SearchIndex;

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
        match child
            .try_wait()
            .map_err(|e| SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Failed while waiting for {action}: {e}"),
            })?
            .is_some()
        {
            true => {
                return child
                    .wait_with_output()
                    .map_err(|e| SearchXyzError::CrawlFailed {
                        url: url.to_string(),
                        reason: format!("Failed to collect {action} output: {e}"),
                    });
            }
            false => {}
        }

        match std::time::Instant::now() >= deadline {
            true => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!("{action} timed out after {timeout_secs}s"),
                });
            }
            false => {}
        }

        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn run_git_checked(
    repo_dir: &Path,
    args: &[&str],
    timeout_secs: u64,
    url: &str,
    action: &str,
    failure_label: &str,
) -> Result<String, SearchXyzError> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(repo_dir);
    let output = run_command_with_timeout(&mut cmd, timeout_secs, url, action)?;
    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        false => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("{failure_label} failed: {}", stderr.trim()),
            })
        }
    }
}

fn run_git_nonempty_rev(
    repo_dir: &Path,
    args: &[&str],
    timeout_secs: u64,
    url: &str,
    action: &str,
    failure_label: &str,
) -> Result<String, SearchXyzError> {
    let rev = run_git_checked(repo_dir, args, timeout_secs, url, action, failure_label)?;
    match rev.is_empty() {
        true => Err(SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: format!("{failure_label} returned an empty commit hash"),
        }),
        false => Ok(rev),
    }
}

fn enforce_ingest_limits(
    repo_dir: &Path,
    relative_files: &[PathBuf],
    limits: &GithubIngestLimits,
    url: &str,
) -> Result<(), SearchXyzError> {
    match relative_files.len() > limits.max_files {
        true => {
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!(
                    "GitHub ingest file count limit exceeded: files={}, max_files={}",
                    relative_files.len(),
                    limits.max_files
                ),
            });
        }
        false => {}
    }

    let mut total_bytes = 0_u64;
    for rel in relative_files {
        let path = repo_dir.join(rel);
        match fs::metadata(&path) {
            Ok(metadata) => {
                total_bytes = total_bytes.saturating_add(metadata.len());
                match total_bytes > limits.max_total_bytes {
                    true => {
                        return Err(SearchXyzError::CrawlFailed {
                            url: url.to_string(),
                            reason: format!(
                                "GitHub ingest byte limit exceeded: bytes={}, max_total_bytes={}",
                                total_bytes, limits.max_total_bytes
                            ),
                        });
                    }
                    false => {}
                }
            }
            Err(_) => {}
        }
    }

    Ok(())
}

/// Parse a GitHub URL to extract Owner, Repo, and optional Branch
pub fn parse_github_url(url: &str) -> Option<(String, String, Option<String>)> {
    let parsed = url::Url::parse(url).ok()?;

    // Check hostname
    let host = parsed.host_str()?;
    match matches!(host, "github.com" | "www.github.com") {
        true => {}
        false => return None,
    }

    let mut segments = parsed.path_segments()?;
    let owner = segments.next()?.to_string();
    let repo_raw = segments.next()?;
    let repo = repo_raw.strip_suffix(".git").unwrap_or(repo_raw).to_string();

    // Check if there is a branch specified, e.g. /tree/branch-name or /blob/branch-name
    let branch = match segments.next() {
        Some("tree") | Some("blob") => segments.next().map(|b| b.to_string()),
        _ => None,
    };

    match owner.is_empty() || repo.is_empty() {
        true => None,
        false => Some((owner, repo, branch)),
    }
}

fn is_excluded_rel_path(rel_path: &Path, exclude_paths: &[String]) -> bool {
    exclude_paths
        .iter()
        .any(|ex| rel_path.components().any(|c| c.as_os_str() == ex.as_str()))
}

fn has_included_extension(path: &Path, include_exts: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext_lower = ext.to_lowercase();
            include_exts.iter().any(|ie| ie.to_lowercase() == ext_lower)
        })
        .unwrap_or(false)
}

/// Recursively walk the directory, filtering files by extension and path exclusion
fn visit_dirs(
    dir: &Path,
    base_dir: &Path,
    include_exts: &[String],
    exclude_paths: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    match dir.is_dir() {
        false => return Ok(()),
        true => {}
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let excluded = path
            .strip_prefix(base_dir)
            .ok()
            .is_some_and(|rel_path| is_excluded_rel_path(rel_path, exclude_paths));
        match excluded {
            true => continue,
            false => {}
        }

        match (path.is_dir(), path.is_file()) {
            (true, _) => {
                visit_dirs(&path, base_dir, include_exts, exclude_paths, files)?;
            }
            (false, true) => {
                let too_large = fs::metadata(&path)
                    .ok()
                    .is_some_and(|m| m.len() > MAX_FILE_SIZE_BYTES);
                match !too_large && has_included_extension(&path, include_exts) {
                    true => files.push(path),
                    false => {}
                }
            }
            (false, false) => {}
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
    match repo_dir.exists() && repo_git_dir.exists() {
        true => {
            is_incremental = true;
            // 1. Get active branch
            let active_branch = run_git_checked(
                &repo_dir,
                &["rev-parse", "--abbrev-ref", "HEAD"],
                limits.git_timeout_secs,
                url,
                "git rev-parse --abbrev-ref",
                "git rev-parse --abbrev-ref HEAD",
            )?;

            // 2. Get old commit hash
            let old_commit = run_git_nonempty_rev(
                &repo_dir,
                &["rev-parse", "HEAD"],
                limits.git_timeout_secs,
                url,
                "git rev-parse HEAD",
                "git rev-parse HEAD (old)",
            )?;

            // 3. Fetch from remote
            tracing::info!(repo = %repo_base_url, branch = %active_branch, "Fetching latest changes");
            run_git_checked(
                &repo_dir,
                &["fetch", "--depth", "1", "origin", &active_branch],
                limits.git_timeout_secs,
                url,
                "git fetch",
                "git fetch",
            )?;

            // 4. Reset hard to FETCH_HEAD
            run_git_checked(
                &repo_dir,
                &["reset", "--hard", "FETCH_HEAD"],
                limits.git_timeout_secs,
                url,
                "git reset",
                "git reset",
            )?;

            // 5. Get new commit hash
            let new_commit = run_git_nonempty_rev(
                &repo_dir,
                &["rev-parse", "HEAD"],
                limits.git_timeout_secs,
                url,
                "git rev-parse",
                "git rev-parse HEAD (new)",
            )?;

            // 6. Get diff list of files
            let diff_str = run_git_checked(
                &repo_dir,
                &[
                    "diff",
                    "--no-renames",
                    "--name-status",
                    &old_commit,
                    &new_commit,
                ],
                limits.git_timeout_secs,
                url,
                "git diff",
                "git diff",
            )?;

            for line in diff_str.lines() {
                let parts: Vec<&str> = line.splitn(2, '\t').collect();
                match parts.as_slice() {
                    [status, file_path] => {
                        let rel_path = Path::new(file_path);
                        match is_excluded_rel_path(rel_path, &exclude_paths_vec) {
                            true => continue,
                            false => {}
                        }

                        let path = repo_dir.join(file_path);
                        let exceeds_limit = (*status != "D")
                            && fs::metadata(&path)
                                .ok()
                                .is_some_and(|m| m.len() > MAX_FILE_SIZE_BYTES);

                        match exceeds_limit {
                            true => {
                                match *status == "M" {
                                    true => files_to_delete.push((*file_path).to_string()),
                                    false => {}
                                }
                                continue;
                            }
                            false => {}
                        }

                        match has_included_extension(rel_path, &include_exts_vec) {
                            false => continue,
                            true => {}
                        }

                        match *status {
                            "D" => {
                                files_to_delete.push((*file_path).to_string());
                            }
                            "M" => {
                                files_to_delete.push((*file_path).to_string());
                                files_to_upsert.push((*file_path).to_string());
                            }
                            "A" => {
                                files_to_upsert.push((*file_path).to_string());
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        false => {
            // Clean clone
            match repo_dir.exists() {
                true => {
                    let _ = fs::remove_dir_all(&repo_dir);
                }
                false => {}
            }
            fs::create_dir_all(&repo_dir).map_err(|e| SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Failed to create repository directory: {}", e),
            })?;

            let mut cmd = Command::new("git");
            cmd.arg("clone").arg("--depth").arg("1");

            match resolved_branch {
                Some(b) => {
                    let safe_branch = b
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/');
                    match safe_branch {
                        false => {
                            return Err(SearchXyzError::CrawlFailed {
                                url: url.to_string(),
                                reason: format!("Invalid/unsafe branch name: {}", b),
                            });
                        }
                        true => {
                            cmd.arg("--branch").arg(b);
                        }
                    }
                }
                None => {}
            }

            cmd.arg(&repo_base_url).arg(&repo_dir);

            tracing::info!(repo = %repo_base_url, branch = ?resolved_branch, dest = ?repo_dir, "Cloning repository");
            let output =
                run_command_with_timeout(&mut cmd, limits.git_timeout_secs, url, "git clone")?;

            match output.status.success() {
                true => {}
                false => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(SearchXyzError::CrawlFailed {
                        url: url.to_string(),
                        reason: format!("git clone failed: {}", stderr.trim()),
                    });
                }
            }

            let mut file_paths = Vec::new();
            visit_dirs(
                &repo_dir,
                &repo_dir,
                &include_exts_vec,
                &exclude_paths_vec,
                &mut file_paths,
            )
            .map_err(|e| SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Directory traversal failed: {}", e),
            })?;

            for path in file_paths {
                match path.strip_prefix(&repo_dir) {
                    Ok(rel_path) => {
                        files_to_upsert.push(rel_path.to_string_lossy().to_string());
                    }
                    Err(_) => {}
                }
            }
        }
    }

    let budget_files: Vec<PathBuf> = files_to_upsert.iter().map(PathBuf::from).collect();
    enforce_ingest_limits(&repo_dir, &budget_files, &limits, url)?;

    // Process deletions
    let branch_for_url = resolved_branch.unwrap_or("main");

    for rel_path_str in &files_to_delete {
        let file_url = format!("{}/blob/{}/{}", repo_base_url, branch_for_url, rel_path_str);
        match index.delete_document(&file_url).await {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(url = %file_url, error = %e, "Failed to delete file from search index");
            }
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

        match index.add_document(&extracted, "github").await {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(url = %file_url, error = %e, "Failed to index GitHub file");
            }
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
        match file_name_lower == "readme.md"
            && Path::new(rel_path_str).parent() == Some(Path::new(""))
        {
            true => {
                readme_content = file_content.clone();
            }
            false => {}
        }

        file_list_summary.push_str(&format!("- [`{}`]({})\n", rel_path_str, file_url));
        indexed_count += 1;
    }

    let summary = match is_incremental {
        true => {
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

            match readme_content.is_empty() {
                true => {}
                false => {
                    summary.push_str("## Repository README (Updated)\n\n");
                    summary.push_str(&readme_content);
                    summary.push_str("\n\n---\n\n");
                }
            }

            match file_list_summary.is_empty() {
                true => {}
                false => {
                    summary.push_str("## Updated/Added Files\n\n");
                    summary.push_str(&file_list_summary);
                }
            }
            summary
        }
        false => {
            let mut summary = format!(
                "# Codebase Ingested: {}/{}\n\n\
                **Source:** {}\n\
                **Branch:** {}\n\
                **Files Indexed:** {} matching files successfully parsed and added to search index.\n\n",
                owner, repo_name, repo_base_url, branch_for_url, indexed_count
            );

            match readme_content.is_empty() {
                true => {}
                false => {
                    summary.push_str("## Repository README\n\n");
                    summary.push_str(&readme_content);
                    summary.push_str("\n\n---\n\n");
                }
            }

            summary.push_str("## Ingested Codebase Files\n\n");
            summary.push_str(&file_list_summary);
            summary
        }
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

    fn restore_env(key: &str, old: &Option<String>) {
        match old {
            Some(val) => std::env::set_var(key, val),
            None => std::env::remove_var(key),
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            restore_env("HOME", &self.old_home);
            restore_env("GIT_CONFIG_COUNT", &self.old_config_count);
            restore_env("GIT_CONFIG_KEY_0", &self.old_config_key_0);
            restore_env("GIT_CONFIG_VALUE_0", &self.old_config_value_0);
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
            rand::thread_rng().gen::<u32>()
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
            rand::thread_rng().gen::<u32>()
        ));
        let dummy_origin = std::env::temp_dir().join(format!(
            "test_incremental_git_sync_origin_{}",
            rand::thread_rng().gen::<u32>()
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
        let index_config = crate::tools::searchxyz::core::config::IndexConfig {
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
