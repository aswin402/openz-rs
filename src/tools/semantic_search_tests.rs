use super::*;

#[tokio::test]
async fn test_sqlite_cache_storage_and_pruning() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_embed_cache_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let _db_path = temp_dir.join("embeddings_cache.db");

    let conn = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async { get_db_conn().unwrap() })
        .await;

    let fake_file = temp_dir.join("test_file.txt");
    std::fs::write(&fake_file, b"hello world").unwrap();
    let path_str = fake_file.to_string_lossy().to_string();

    conn.execute(
        "INSERT INTO file_cache (file_path, mtime_secs) VALUES (?1, ?2)",
        rusqlite::params![&path_str, 12345u64],
    )
    .unwrap();

    let dummy_emb = vec![0.1f32, 0.2f32, 0.3f32];
    let mut bytes = Vec::new();
    for val in &dummy_emb {
        bytes.extend_from_slice(&val.to_ne_bytes());
    }

    conn.execute(
        "INSERT INTO chunk_cache (file_path, chunk_index, chunk_text, embedding) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![&path_str, 0usize, "hello world", bytes],
    ).unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT chunk_index, chunk_text, embedding FROM chunk_cache WHERE file_path = ?1",
        )
        .unwrap();
    let mut rows = stmt
        .query_map([&path_str], |row: &rusqlite::Row<'_>| {
            let idx: usize = row.get(0)?;
            let text: String = row.get(1)?;
            let bytes: Vec<u8> = row.get(2)?;
            let mut embedding = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let array: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                embedding.push(f32::from_ne_bytes(array));
            }
            Ok((idx, text, embedding))
        })
        .unwrap();

    let (idx, text, emb) = rows.next().unwrap().unwrap();
    assert_eq!(idx, 0);
    assert_eq!(text, "hello world");
    assert_eq!(emb, dummy_emb);

    std::fs::remove_file(&fake_file).unwrap();

    prune_deleted_files(&conn).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM file_cache",
            [],
            |row: &rusqlite::Row<'_>| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);

    let chunk_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunk_cache",
            [],
            |row: &rusqlite::Row<'_>| row.get(0),
        )
        .unwrap();
    assert_eq!(chunk_count, 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
