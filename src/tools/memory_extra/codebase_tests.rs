use super::*;
use crate::tools::graph_memory::{test_lock, with_db};
use crate::tools::memory_extra::coordinator::{MemoryCoordinator, MemoryScope};
use crate::tools::Tool;
use rusqlite::params;
use serde_json::json;

struct TestEnvLock;

impl TestEnvLock {
    fn acquire() -> Self {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => break,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        TestEnvLock
    }
}

impl Drop for TestEnvLock {
    fn drop(&mut self) {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        let _ = std::fs::remove_file(lock_path);
    }
}

#[tokio::test]
async fn test_index_codebase_captures_rust_impl_trait_methods() {
    let _l = test_lock().lock().await;
    let dir = std::env::temp_dir().join(format!("openz_code_index_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("sample.rs"),
        r#"
pub trait Runner {
    fn run(&self);
}

pub struct Worker;

impl Runner for Worker {
    async fn run(&self) {
        helper();
    }
}

fn helper() {}
"#,
    )
    .unwrap();

    let scope = format!("test_code_index_{}", uuid::Uuid::new_v4());
    let tool = IndexCodebaseTool;
    tool.call(&json!({ "path": dir, "sessionId": scope }))
        .await
        .unwrap();

    let indexed = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT element_type, name, signature FROM code_elements WHERE session_id = ?1 ORDER BY start_line",
        )?;
        let rows = stmt.query_map(params![scope], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }
        Ok(values)
    })
    .unwrap();

    assert!(indexed
        .iter()
        .any(|(typ, name, _)| typ == "Trait" && name == "Runner"));
    assert!(indexed
        .iter()
        .any(|(typ, name, _)| typ == "Struct" && name == "Worker"));
    assert!(indexed.iter().any(|(typ, name, sig)| typ == "ImplBlock"
        && name == "impl_Worker"
        && sig.contains("Runner for Worker")));
    assert!(indexed
        .iter()
        .any(|(typ, name, _)| typ == "Function" && name == "run"));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_memory_stats_uses_coordinator_snapshot() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_stats_coord_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let marker = format!("stats-coordinator-marker-{}", uuid::Uuid::new_v4());
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());
    coordinator
        .write_semantic(
            &format!("{} should appear in stats", marker),
            0.9,
            &memory_scope,
        )
        .await
        .unwrap();

    let stats_tool = MemoryStatsTool;
    let before = stats_tool
        .call(&json!({ "sessionId": scope }))
        .await
        .unwrap();
    let current_workspace = crate::tools::shared_memory::get_current_workspace();
    crate::tools::shared_memory::with_db(|conn| {
        conn.execute(
            "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
             VALUES (?1, ?2, '[]', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?3, '[]', 0.8, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 1, 0.05)",
            params![format!("{}-current-cognitive", scope), format!("{} current cognitive", marker), current_workspace],
        )?;
        conn.execute(
            "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
             VALUES (?1, ?2, '[]', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'foreign-workspace', '[]', 0.8, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 1, 0.05)",
            params![format!("{}-foreign-cognitive", scope), format!("{} foreign cognitive", marker)],
        )?;
        Ok(())
    })
    .unwrap();

    let stats = stats_tool
        .call(&json!({ "sessionId": scope }))
        .await
        .unwrap();

    assert!(stats["semanticFacts"].as_i64().unwrap() >= 1);
    assert!(stats["semanticFactsWithEmbeddings"].as_i64().unwrap() >= 1);
    assert_eq!(stats["coordinator"], true);
    assert_eq!(
        stats["cognitiveMemories"].as_i64().unwrap()
            - before["cognitiveMemories"].as_i64().unwrap(),
        1
    );
    assert!(stats["totalActive"].as_i64().unwrap() >= stats["semanticFacts"].as_i64().unwrap());
}

#[tokio::test]
async fn test_memory_stats_counts_session_skills_and_working_layers() {
    let _env_lock = TestEnvLock::acquire();
    let _l = test_lock().lock().await;
    let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
    let config_dir = std::env::temp_dir().join(format!(
        "openz_stats_runtime_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::env::set_var("OPENZ_CONFIG_DIR", &config_dir);

    let scope = format!("test_stats_layers_{}", uuid::Uuid::new_v4());
    let sessions_dir = config_dir.join("sessions");
    let manager = crate::session::SessionManager::new(sessions_dir);
    let mut session = crate::session::Session::new(&scope);
    session.metadata.insert(
        "memory".to_string(),
        serde_json::Value::String("session layer memory fact".to_string()),
    );
    manager.save(&session).await.unwrap();

    crate::agent::skills::save_skill("stats_layer_skill", "skill layer memory fact").unwrap();

    let working_tool = crate::tools::memory_extra::working::SetWorkingMemoryTool;
    working_tool
        .call(&json!({
            "key": "stats-working-key",
            "value": "stats-working-value",
            "ttl": 300,
            "sessionId": scope
        }))
        .await
        .unwrap();

    let stats_tool = MemoryStatsTool;
    let stats = stats_tool
        .call(&json!({ "sessionId": scope }))
        .await
        .unwrap();

    assert!(stats["sessionMetadataMemories"].as_i64().unwrap() >= 1);
    assert!(stats["skillsMemories"].as_i64().unwrap() >= 1);
    assert!(stats["workingMemory"].as_i64().unwrap() >= 1);
    assert!(stats["totalActive"].as_i64().unwrap() >= 3);

    let _ = std::fs::remove_dir_all(&config_dir);
    if let Some(prev) = previous_config_dir {
        std::env::set_var("OPENZ_CONFIG_DIR", prev);
    } else {
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
}

#[tokio::test]
async fn test_compress_context() {
    let tool = CompressContextTool;
    let res = tool.call(&json!({
        "text": "This is the first important sentence. This is the second one. Third sentence is here. Fourth and final one.",
        "ratio": 0.5
    })).await.unwrap();
    assert!(res["compressedLength"].as_u64().unwrap() > 0);
    assert!(
        res["originalLength"].as_u64().unwrap() > res["compressedLength"].as_u64().unwrap()
    );
}
