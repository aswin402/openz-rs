use super::*;

#[cfg(test)]
mod hash_tests {
    use super::*;

    #[test]
    fn hash_changes_when_extra_metadata_changes() {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_1"));

        let h1 =
            Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_2"));
        let h2 =
            Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");

        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_ignores_hash_field_itself() {
        let mut extra = serde_json::Map::new();
        extra.insert("name".to_string(), serde_json::json!("read_file"));
        let h1 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        extra.insert("hash".to_string(), serde_json::json!("old"));
        let h2 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        assert_eq!(h1, h2);
    }

    #[test]
    fn legacy_hash_chain_still_verifies() {
        let mut session = Session::new("test");
        session.messages.push(Message {
            role: "user".to_string(),
            content: "hello".to_string(),
            timestamp: Some("2026-07-06T00:00:00Z".to_string()),
            extra: serde_json::Map::new(),
        });
        let legacy = legacy_message_hash("user", "hello", Some("2026-07-06T00:00:00Z"), "");
        session.messages[0].set_hash(legacy);

        assert!(session.verify_hash_chain().is_ok());
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    use anyhow::Result;
    use std::time::{Duration, SystemTime};

    #[cfg(unix)]
    fn set_modified_for_test(path: &std::path::Path, modified: SystemTime) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let duration = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let times = [
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
        ];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_modified_for_test(_path: &std::path::Path, _modified: SystemTime) -> Result<()> {
        Ok(())
    }

    #[test]
    fn acquire_lock_removes_stale_corrupt_lock_path() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_session_stale_lock_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock("session")?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn acquire_lock_preserves_stale_regular_lock_file() -> Result<()> {
        let dir = std::env::temp_dir().join(format!(
            "openz_session_stale_regular_lock_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::write(&lock_path, b"old")?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock("session")?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[tokio::test]
    async fn acquire_lock_async_removes_stale_corrupt_lock_path() -> Result<()> {
        let dir = std::env::temp_dir().join(format!(
            "openz_session_async_stale_lock_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock_async("session").await?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn acquire_lock_preserves_fresh_corrupt_lock_path() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_session_fresh_lock_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;

        let result = manager.acquire_lock("session");

        assert!(result.is_err());
        assert!(lock_path.is_dir());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}

#[cfg(test)]
mod delete_tests {
    use super::*;

    #[test]
    fn delete_removes_sanitized_session_file() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_delete_session_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let mut session = Session::new("ws:control");
        session.add_message("user", "hello");
        session.populate_hashes();
        let path = manager.file_path(&session.key);
        fs::write(&path, serde_json::to_string_pretty(&session)?)?;

        assert!(path.exists());
        assert!(manager.delete("ws:control")?);
        assert!(!path.exists());
        assert!(!manager.delete("ws:control")?);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn archive_moves_sanitized_session_file_to_archive_dir() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_archive_session_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let mut session = Session::new("telegram:42");
        session.add_message("user", "archive me");
        session.populate_hashes();
        let path = manager.file_path(&session.key);
        fs::write(&path, serde_json::to_string_pretty(&session)?)?;

        let archived = manager.archive("telegram:42")?.expect("session archived");

        assert!(!path.exists());
        assert!(archived.path.exists());
        assert!(archived.path.starts_with(dir.join("archive")));
        assert_eq!(archived.session.key, "telegram:42");
        assert_eq!(archived.session.messages.len(), 1);
        assert!(manager.archive("telegram:42")?.is_none());

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn list_summaries_extracts_and_sorts() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_summary_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());

        let mut s1 = Session::new("chat:1");
        s1.add_message("user", "Hello world from chat 1");
        let path1 = manager.file_path(&s1.key);
        fs::write(&path1, serde_json::to_string(&s1)?)?;

        let mut s2 = Session::new("chat:2");
        s2.add_message("user", "Second message here");
        s2.add_message("assistant", "Response");
        let path2 = manager.file_path(&s2.key);
        fs::write(&path2, serde_json::to_string(&s2)?)?;

        let summaries = manager.list_summaries();
        assert_eq!(summaries.len(), 2);
        assert!(summaries
            .iter()
            .any(|s| s.key == "chat:1" && s.first_user_message == Some("Hello world from chat 1".into())));
        assert!(summaries
            .iter()
            .any(|s| s.key == "chat:2" && s.message_count == 2));

        let summary1 = summaries.iter().find(|s| s.key == "chat:1").unwrap();
        assert_eq!(summary1.preview_title(10, "fallback"), "Hello w...");

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn list_summaries_paginated_returns_slice_and_total() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_page_summary_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());

        for i in 1..=5 {
            let mut s = Session::new(&format!("session:{}", i));
            s.add_message("user", &format!("hello from {}", i));
            s.updated_at = Utc::now() + chrono::Duration::seconds(i);
            let path = manager.file_path(&s.key);
            fs::write(&path, serde_json::to_string(&s)?)?;
        }

        let (page1, total1) = manager.list_summaries_paginated(0, 2);
        assert_eq!(total1, 5);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].key, "session:5");
        assert_eq!(page1[1].key, "session:4");

        let (page2, total2) = manager.list_summaries_paginated(2, 2);
        assert_eq!(total2, 5);
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].key, "session:3");
        assert_eq!(page2[1].key, "session:2");

        let (page3, total3) = manager.list_summaries_paginated(4, 2);
        assert_eq!(total3, 5);
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].key, "session:1");

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
