use super::*;
use crate::tools::graph_memory::{test_lock, with_db};
use rusqlite::params;

#[tokio::test]
async fn test_memory_coordinator_write_recall_stats_and_forget() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_coord_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let marker = format!("coordinator-marker-{}", uuid::Uuid::new_v4());
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());

    let write = coordinator
        .write_semantic(
            &format!("{} should be recalled through coordinator", marker),
            0.9,
            &memory_scope,
        )
        .await
        .unwrap();
    assert_eq!(write.layer, MemoryLayer::Semantic);

    let recalled = coordinator.recall(&marker, 5, &memory_scope).await.unwrap();
    assert!(recalled.iter().any(|item| item.text.contains(&marker)));

    let stats = coordinator.stats(&memory_scope).await.unwrap();
    assert!(stats.semantic_facts >= 1);
    assert!(stats.total_active >= stats.semantic_facts);

    let deleted = coordinator.forget(&marker, &memory_scope).await.unwrap();
    assert!(deleted.semantic_facts_expired >= 1);

    let after = coordinator.recall(&marker, 5, &memory_scope).await.unwrap();
    assert!(!after.iter().any(|item| item.text.contains(&marker)));
}

#[tokio::test]
async fn test_memory_coordinator_auto_importance_and_exclusive_relation_resolution() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_coord_conflict_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());
    let semantic_id = format!("coord-auto-{}", uuid::Uuid::new_v4());

    coordinator
        .write_semantic_with_id(
            &semantic_id,
            "Auto importance scoring should be applied",
            -1.0,
            &memory_scope,
        )
        .await
        .unwrap();
    let importance = with_db(|conn| {
        conn.query_row(
            "SELECT importance FROM semantic_metadata WHERE node_id = ?1 AND session_id = ?2 AND valid_until IS NULL",
            params![semantic_id, scope],
            |r| r.get::<_, f64>(0),
        )
        .map_err(anyhow::Error::from)
    })
    .unwrap();
    assert!(
        (importance - 0.333).abs() < 0.08,
        "fresh auto importance should match memory_rs scorer baseline"
    );

    let first = coordinator
        .write_graph_relation("OpenZ", "current_job", "Prototype", &memory_scope)
        .await
        .unwrap();
    assert!(first.created);
    let second = coordinator
        .write_graph_relation("OpenZ", "current_job", "Production", &memory_scope)
        .await
        .unwrap();
    assert!(second.created);
    assert_eq!(second.conflicts_resolved, 1);

    let active_targets = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT to_name FROM graph_edges WHERE from_name = 'OpenZ' AND relation_type = 'current_job' AND session_id = ?1 AND valid_until IS NULL ORDER BY to_name",
        )?;
        let rows = stmt.query_map(params![scope], |row| row.get::<_, String>(0))?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }
        Ok(values)
    })
    .unwrap();
    assert_eq!(active_targets, vec!["Production".to_string()]);
}

#[tokio::test]
async fn test_memory_coordinator_resolves_semantic_similarity_conflicts_by_importance() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_coord_semantic_similarity_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());

    coordinator
        .write_semantic_with_id(
            "semantic-sim-old",
            "OpenZ uses Rust for agent runtime",
            0.2,
            &memory_scope,
        )
        .await
        .unwrap();
    coordinator
        .write_semantic_with_id(
            "semantic-sim-new",
            "OpenZ uses Rust for the agent runtime",
            0.9,
            &memory_scope,
        )
        .await
        .unwrap();

    let active = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT node_id FROM semantic_metadata WHERE raw_text LIKE 'OpenZ uses Rust for%agent runtime%' AND session_id = ?1 AND valid_until IS NULL ORDER BY node_id",
        )?;
        let rows = stmt.query_map(params![scope], |row| row.get::<_, String>(0))?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }
        Ok(values)
    })
    .unwrap();
    assert_eq!(active, vec!["semantic-sim-new".to_string()]);
}

#[tokio::test]
async fn test_memory_coordinator_resolves_semantic_slot_conflicts() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_coord_semantic_conflict_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());

    coordinator
        .write_semantic_with_id(
            "semantic-job-old",
            "OpenZ current job is Prototype",
            -1.0,
            &memory_scope,
        )
        .await
        .unwrap();
    coordinator
        .write_semantic_with_id(
            "semantic-job-new",
            "OpenZ current job is Production",
            -1.0,
            &memory_scope,
        )
        .await
        .unwrap();

    let active = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT node_id FROM semantic_metadata WHERE raw_text LIKE 'OpenZ current job is %' AND session_id = ?1 AND valid_until IS NULL ORDER BY node_id",
        )?;
        let rows = stmt.query_map(params![scope], |row| row.get::<_, String>(0))?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }
        Ok(values)
    })
    .unwrap();
    assert_eq!(active, vec!["semantic-job-new".to_string()]);
}

#[tokio::test]
async fn test_memory_coordinator_writes_semantic_ids_and_graph_relations() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_coord_write_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let coordinator = MemoryCoordinator::default();
    let memory_scope = MemoryScope::session(scope.clone());
    let semantic_id = format!("coord-explicit-{}", uuid::Uuid::new_v4());

    let semantic = coordinator
        .write_semantic_with_id(
            &semantic_id,
            "Coordinator explicit semantic write",
            0.77,
            &memory_scope,
        )
        .await
        .unwrap();
    assert_eq!(semantic.id, semantic_id);
    assert_eq!(semantic.layer, MemoryLayer::Semantic);

    let relation = coordinator
        .write_graph_relation("CoordinatorA", "uses", "CoordinatorB", &memory_scope)
        .await
        .unwrap();
    assert_eq!(relation.layer, MemoryLayer::Graph);
    assert!(relation.created);

    let found = with_db(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE from_name = 'CoordinatorA' AND to_name = 'CoordinatorB' AND relation_type = 'uses' AND session_id = ?1 AND valid_until IS NULL",
            params![scope],
            |r| r.get::<_, i64>(0),
        )
        .map_err(anyhow::Error::from)
    })
    .unwrap();
    assert_eq!(found, 1);
}
