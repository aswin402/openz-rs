use super::*;

#[test]
fn find_files_source_does_not_follow_symlinks_with_fd() {
    let source = std::fs::read_to_string("src/tools/filesystem.rs").unwrap();
    let follow_symlinks_arg = ["cmd.arg(\"", "-L", "\")"].join("");
    assert!(
        !source.contains(&follow_symlinks_arg),
        "find_files must not pass fd -L because it crosses symlink boundaries"
    );
}

fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {:?} failed: {}{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[tokio::test]
async fn zenflow_edit_failure_preserves_unrelated_dirty_file() -> Result<()> {
    let repo =
        std::env::temp_dir().join(format!("openz_zenflow_rollback_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&repo)?;
    run_git(&repo, &["init"])?;
    run_git(
        &repo,
        &["config", "user.email", "openz-test@example.invalid"],
    )?;
    run_git(&repo, &["config", "user.name", "OpenZ Test"])?;

    let target = repo.join("target.txt");
    let unrelated = repo.join("unrelated.txt");
    std::fs::write(&target, "target initial\n")?;
    std::fs::write(&unrelated, "unrelated initial\n")?;
    run_git(&repo, &["add", "."])?;
    run_git(&repo, &["commit", "-m", "initial"])?;

    std::fs::write(&target, "target dirty before edit\n")?;
    std::fs::write(&unrelated, "unrelated dirty must survive\n")?;

    let provider = std::sync::Arc::new(
        crate::providers::mock::MockProvider::new()
            .with_default(crate::providers::mock::MockResponse::text("")),
    );
    let tool = ZenflowEditTool { provider };
    let result = crate::config::loader::ACTIVE_WORKSPACE
        .scope(repo.clone(), async {
            tool.call(&serde_json::json!({
                "path": "target.txt",
                "content": "target attempted edit\n",
                "compile_command": "false"
            }))
            .await
        })
        .await?;

    assert_eq!(result["status"], "error");
    assert_eq!(
        std::fs::read_to_string(&target)?,
        "target dirty before edit\n"
    );
    assert_eq!(
        std::fs::read_to_string(&unrelated)?,
        "unrelated dirty must survive\n"
    );

    let _ = std::fs::remove_dir_all(&repo);
    Ok(())
}

#[test]
fn filesystem_tool_schemas_document_supported_aliases() {
    let read_schema = ReadFileTool.parameters().to_string();
    assert!(read_schema.contains("filePath"));
    assert!(read_schema.contains("file_path"));
    assert!(read_schema.contains("startLine"));
    assert!(read_schema.contains("endLine"));

    let write_schema = WriteFileTool.parameters().to_string();
    assert!(write_schema.contains("filePath"));
    assert!(write_schema.contains("file_path"));

    let replace_schema = ReplaceLinesTool.parameters().to_string();
    assert!(replace_schema.contains("filePath"));
    assert!(replace_schema.contains("startLine"));
    assert!(replace_schema.contains("content"));

    let find_schema = FindFilesTool.parameters().to_string();
    assert!(find_schema.contains("glob"));
    assert!(find_schema.contains("directory"));
}

#[tokio::test]
async fn filesystem_tools_accept_argument_aliases() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_fs_alias_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let file_path = temp_dir.join("alias_match.txt");

    let write = WriteFileTool;
    let write_res = write
        .call(&serde_json::json!({
            "filePath": file_path.to_str().unwrap(),
            "content": "one\ntwo\nthree\n"
        }))
        .await?;
    assert_eq!(write_res["status"], "success");

    let read = ReadFileTool;
    let read_res = read
        .call(&serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "startLine": 2,
            "endLine": 2
        }))
        .await?;
    assert_eq!(read_res, serde_json::Value::String("two".to_string()));

    let replace = ReplaceLinesTool;
    let replace_res = replace
        .call(&serde_json::json!({
            "filePath": file_path.to_str().unwrap(),
            "startLine": 2,
            "endLine": 2,
            "content": "TWO"
        }))
        .await?;
    assert_eq!(replace_res["status"], "success");
    assert_eq!(std::fs::read_to_string(&file_path)?, "one\nTWO\nthree\n");

    let patch = diffy::create_patch("one\nTWO\nthree\n", "one\nTWO\nTHREE\n");
    let patch_res = PatchFileTool
        .call(&serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "patch": patch.to_string()
        }))
        .await?;
    assert_eq!(patch_res["status"], "success");
    assert_eq!(std::fs::read_to_string(&file_path)?, "one\nTWO\nTHREE\n");

    let list = ListDirTool;
    let list_res = list
        .call(&serde_json::json!({
            "filePath": temp_dir.to_str().unwrap()
        }))
        .await?;
    assert_eq!(
        list_res["canonical_path"].as_str().unwrap(),
        temp_dir.canonicalize()?.to_string_lossy()
    );
    assert!(list_res["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| { entry["name"] == "alias_match.txt" }));

    let find = FindFilesTool;
    let find_res = find
        .call(&serde_json::json!({
            "glob": "alias_*.txt",
            "directory": temp_dir.to_str().unwrap()
        }))
        .await?;
    assert_eq!(find_res["status"], "success");
    assert!(find_res["results"].as_array().unwrap().iter().any(|entry| {
        entry
            .as_str()
            .unwrap_or_default()
            .ends_with("alias_match.txt")
    }));

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_find_files() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_find_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let file_path = temp_dir.join("match_this.txt");
    std::fs::write(&file_path, "Hello world!")?;

    let tool = FindFilesTool;
    let args = serde_json::json!({
        "pattern": "*match*",
        "dir": temp_dir.to_str().unwrap()
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    let results = res["results"].as_array().unwrap();
    assert!(results.len() >= 1);

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_replace_lines() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_replace_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let file_path = temp_dir.join("test.txt");
    std::fs::write(&file_path, "line 1\nline 2\nline 3")?;

    let tool = ReplaceLinesTool;
    let args = serde_json::json!({
        "path": file_path.to_str().unwrap(),
        "start_line": 2,
        "end_line": 2,
        "replacement": "replaced line 2"
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");

    let updated = std::fs::read_to_string(&file_path)?;
    assert_eq!(updated, "line 1\nreplaced line 2\nline 3");

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn zenflow_edit_source_does_not_use_hard_reset() {
    let source = std::fs::read_to_string("src/tools/filesystem.rs").unwrap();
    let hard_reset = ["git reset", "--hard", "HEAD~1"].join(" ");
    assert!(
        !source.contains(&format!("run_cmd(\"{}", hard_reset)),
        "zenflow_edit must not use destructive worktree-wide rollback"
    );
}

#[tokio::test]
async fn test_filesystem_stringified_lines_and_target_file_aliases() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_fs_string_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let file_path = temp_dir.join("string_coercion.txt");

    // Test write_file with target_file and code alias
    let write = WriteFileTool;
    let write_res = write
        .call(&serde_json::json!({
            "target_file": file_path.to_str().unwrap(),
            "code": "alpha\nbeta\ngamma\ndelta\n"
        }))
        .await?;
    assert_eq!(write_res["status"], "success");

    // Test read_file with targetFile and string startLine/endLine
    let read = ReadFileTool;
    let read_res = read
        .call(&serde_json::json!({
            "targetFile": file_path.to_str().unwrap(),
            "startLine": "2",
            "endLine": "3"
        }))
        .await?;
    assert_eq!(read_res, serde_json::Value::String("beta\ngamma".to_string()));

    // Test replace_lines with string numbers and 0-index clamping
    let replace = ReplaceLinesTool;
    let replace_res = replace
        .call(&serde_json::json!({
            "target_file": file_path.to_str().unwrap(),
            "start_line": "0",
            "end_line": "1",
            "new_content": "ALPHA_MODIFIED"
        }))
        .await?;
    assert_eq!(replace_res["status"], "success");
    let content = std::fs::read_to_string(&file_path)?;
    assert!(content.starts_with("ALPHA_MODIFIED\nbeta\n"));

    // Test list_dir with empty args (defaults to current dir)
    let list = ListDirTool;
    let list_res = list.call(&serde_json::json!({})).await?;
    assert!(list_res["entries"].as_array().is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_read_file_hashline_and_replace_lines_drift_relocation() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_hashline_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let file_path = temp_dir.join("drifted.rs");
    std::fs::write(
        &file_path,
        "// header 1\n// header 2\nfn compute() -> i32 {\n    41\n}\n",
    )?;

    let read_res = ReadFileTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "include_hashes": true
        }))
        .await?;
    let text = read_res.as_str().unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5);

    let line4_hash = line_content_hash("    41");
    assert!(lines[3].starts_with(&format!("4:{}|     41", line4_hash)));

    // Simulate stale line number (model passes start_line=2 instead of 4, but with expected_hash of line 4)
    let replace_res = ReplaceLinesTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "start_line": 2,
            "end_line": 2,
            "expected_hash": line4_hash,
            "replacement": "    42"
        }))
        .await?;
    assert_eq!(replace_res["status"], "success");
    assert_eq!(replace_res["relocated"], true);
    assert_eq!(replace_res["applied_start_line"], 4);
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "// header 1\n// header 2\nfn compute() -> i32 {\n    42\n}\n"
    );

    // Anchor text relocation test
    let anchor_res = ReplaceLinesTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "start_line": 1,
            "end_line": 1,
            "anchor_text": "    42",
            "replacement": "    100"
        }))
        .await?;
    assert_eq!(anchor_res["status"], "success");
    assert_eq!(anchor_res["relocated"], true);
    assert_eq!(anchor_res["applied_start_line"], 4);
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "// header 1\n// header 2\nfn compute() -> i32 {\n    100\n}\n"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_patch_file_fuzzy_unified_diff_and_search_replace_modes() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_fuzzy_patch_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let file_path = temp_dir.join("module.rs");
    std::fs::write(
        &file_path,
        "// extra comment 1\n// extra comment 2\nfn greet() {\n    println!(\"hello\");\n}\n",
    )?;

    // 1. Fuzzy unified diff with wrong line numbers in @@ header and slightly different indentation in context
    let fuzzy_diff = "@@ -1,3 +1,3 @@\n fn greet() {\n-  println!(\"hello\");\n+    println!(\"hello world\");\n }\n";
    let res1 = PatchFileTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "patch": fuzzy_diff
        }))
        .await?;
    assert_eq!(res1["status"], "success");
    assert_eq!(res1["mode"], "fuzzy_diff");
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "// extra comment 1\n// extra comment 2\nfn greet() {\n    println!(\"hello world\");\n}\n"
    );

    // 2. Direct target + replacement search-and-replace mode
    let res2 = PatchFileTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "target": "    println!(\"hello world\");",
            "replacement": "    println!(\"salutations\");"
        }))
        .await?;
    assert_eq!(res2["status"], "success");
    assert_eq!(res2["mode"], "search_replace");
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "// extra comment 1\n// extra comment 2\nfn greet() {\n    println!(\"salutations\");\n}\n"
    );

    // 3. Conflict-marker SEARCH/REPLACE block inside patch string
    let marker_patch = "<<<<<<< SEARCH\nfn greet() {\n    println!(\"salutations\");\n}\n=======\nfn greet() {\n    println!(\"done\");\n}\n>>>>>>> REPLACE";
    let res3 = PatchFileTool
        .call(&serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "patch": marker_patch
        }))
        .await?;
    assert_eq!(res3["status"], "success");
    assert_eq!(res3["mode"], "search_replace");
    assert_eq!(
        std::fs::read_to_string(&file_path)?,
        "// extra comment 1\n// extra comment 2\nfn greet() {\n    println!(\"done\");\n}\n"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}


