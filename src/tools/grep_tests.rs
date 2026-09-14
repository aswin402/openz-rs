use super::*;

#[test]
fn scoped_search_dir_clamps_ancestor_to_git_root() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_grep_scope_test_{}", uuid::Uuid::new_v4()));
    let repo = temp_dir.join("repo");
    let src = repo.join("src");
    std::fs::create_dir_all(repo.join(".git"))?;
    std::fs::create_dir_all(&src)?;

    let original = std::env::current_dir()?;
    std::env::set_current_dir(&repo)?;
    let scoped = GrepSearchTool::scoped_search_dir(temp_dir.clone());
    std::env::set_current_dir(original)?;

    assert_eq!(scoped.canonicalize()?, repo.canonicalize()?);
    let _ = std::fs::remove_dir_all(temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_grep_search() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_grep_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let file_path = temp_dir.join("test.txt");
    std::fs::write(
        &file_path,
        "Hello world!\nThis is a grep test.\nHave a nice day!",
    )?;

    let tool = GrepSearchTool;
    let args = json!({
        "query": "grep",
        "dir": temp_dir.to_str().unwrap()
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    let results = res["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["line"], 2);
    assert_eq!(results[0]["content"], "This is a grep test.");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
