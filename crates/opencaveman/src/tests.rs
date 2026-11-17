use crate::{
    all_level_descriptors, analyze_diff_for_commit, analyze_diff_for_review,
    builtin_caveman_skills, cavecrew_subagent_specs, classify_auto_clarity,
    compile_dynamic_auto_clarity, compile_static_directive, compress_file_with_backup,
    compress_markdown_deterministic, detect_command_intent, estimate_tokens, list_backups,
    load_stats_ledger, normalize_level_slug, normalize_review_output, record_turn_savings,
    restore_backup, scan_secret_exposure, validate_and_format_commit, verify_compression_fidelity,
    AutoClarityReason, CavemanCommandIntent, CavemanLevel, ReviewFinding,
};
use std::path::Path;

#[test]
fn level_descriptors_resolve_aliases_and_cycle_cleanly() {
    assert_eq!(all_level_descriptors().len(), 6);
    assert_eq!(CavemanLevel::parse_flexible("lite"), Some(CavemanLevel::Lite));
    assert_eq!(CavemanLevel::parse_flexible("ultracave"), Some(CavemanLevel::Ultra));
    assert_eq!(
        CavemanLevel::parse_flexible("megacave"),
        Some(CavemanLevel::SymbolicFull)
    );
    assert_eq!(
        CavemanLevel::parse_flexible("symbolic-ultra"),
        Some(CavemanLevel::SymbolicUltra)
    );
    assert_eq!(normalize_level_slug("ULTRACAVE"), "ultra");

    let start = CavemanLevel::Lite;
    let cycled = start
        .cycle_next()
        .cycle_next()
        .cycle_next()
        .cycle_next()
        .cycle_next()
        .cycle_next();
    assert_eq!(cycled, start);
    assert_eq!(start.cycle_prev(), CavemanLevel::SymbolicUltra);
}

#[test]
fn auto_clarity_and_command_intent_avoid_false_positives() {
    let normal_vim_question = "How do I exit vim normal mode without saving?";
    assert_eq!(detect_command_intent(normal_vim_question), None);

    assert_eq!(
        detect_command_intent("stop caveman"),
        Some(CavemanCommandIntent::DisableCaveman)
    );
    assert_eq!(
        detect_command_intent("normal mode"),
        Some(CavemanCommandIntent::DisableCaveman)
    );
    assert_eq!(
        detect_command_intent("caveman ultra"),
        Some(CavemanCommandIntent::EnableLevel(CavemanLevel::Ultra))
    );

    let sec_signal = classify_auto_clarity("Check this SQL injection and auth bypass vulnerability");
    assert!(sec_signal.suspend_compression);
    assert!(sec_signal.reasons.contains(&AutoClarityReason::SecurityWarning));

    let dyn_override = compile_dynamic_auto_clarity(true, "Should I run git reset --hard on main?");
    assert!(dyn_override.contains("Auto-Clarity Override Active"));

    let static_prompt = compile_static_directive(true, CavemanLevel::Full);
    assert!(static_prompt.contains("Mode: caveman (full)"));
    assert!(compile_static_directive(false, CavemanLevel::Full).is_empty());
}

#[test]
fn secret_scanner_blocks_env_files_and_live_tokens() {
    let env_reject = scan_secret_exposure(Some(Path::new("/repo/.env.production")), "PORT=8080");
    assert!(env_reject.is_some());

    let example_ok = scan_secret_exposure(
        Some(Path::new("/repo/.env.example")),
        "API_KEY=your_api_key_placeholder",
    );
    assert!(example_ok.is_none());

    let token_reject = scan_secret_exposure(
        Some(Path::new("/repo/NOTES.md")),
        "Token: sk-ant-api03-9f8e7d6c5b4a3210ZYXWVUTSRQPONM",
    );
    assert!(token_reject.is_some());
}

#[test]
fn deterministic_markdown_compression_preserves_code_urls_and_frontmatter_byte_exact() {
    let doc = "---\nname: sample\ntriggers: [\"build\"]\n---\n# Architecture Guide\n\nPlease note that in order to run the application, you basically really need to inspect `src/agent/build.rs` and visit [OpenZ Docs](https://openz.dev/docs/setup).\n\n```rust\nfn main() {\n    let the_value = \"in order to keep exact\";\n    println!(\"{the_value}\");\n}\n```\n";

    let compressed = compress_markdown_deterministic(doc, CavemanLevel::Ultra);
    let fidelity = verify_compression_fidelity(doc, &compressed);
    assert!(
        fidelity.passed,
        "fidelity violations: {:?}",
        fidelity.violations
    );
    assert!(compressed.contains("let the_value = \"in order to keep exact\";"));
    assert!(compressed.contains("https://openz.dev/docs/setup"));
    assert!(compressed.contains("`src/agent/build.rs`"));
    assert!(estimate_tokens(&compressed) < estimate_tokens(doc));
}

#[test]
fn out_of_tree_backup_and_restore_roundtrip_cleanly() {
    let tmp = std::env::temp_dir().join(format!(
        "opencaveman_test_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)
    ));
    let work_dir = tmp.join("repo");
    let runtime_dir = tmp.join("runtime");
    std::fs::create_dir_all(&work_dir).unwrap();

    let md_path = work_dir.join("CLAUDE.md");
    let original = "# Workspace Guidelines\n\nIt is important to note that in order to update the configuration, you should basically really verify `src/config/schema.rs` prior to committing changes.\n";
    std::fs::write(&md_path, original).unwrap();

    let outcome =
        compress_file_with_backup(&md_path, &runtime_dir, CavemanLevel::Full, None, false).unwrap();
    assert!(outcome.saved_tokens > 0);
    assert!(outcome.fidelity.passed);
    assert!(outcome.backup.is_some());

    // Ensure no `.bak` file polluted the repo directory
    let repo_files: Vec<String> = std::fs::read_dir(&work_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(repo_files, vec!["CLAUDE.md".to_string()]);

    let backups = list_backups(&runtime_dir);
    assert_eq!(backups.len(), 1);

    let restored = restore_backup(&runtime_dir, &backups[0].backup_id).unwrap();
    assert_eq!(restored.backup_id, backups[0].backup_id);
    assert_eq!(std::fs::read_to_string(&md_path).unwrap(), original);

    let snap = record_turn_savings(&runtime_dir, CavemanLevel::Full, "Bug in auth. Flip check.");
    assert!(snap.lifetime_output_tokens_saved > 0);
    let loaded = load_stats_ledger(&runtime_dir);
    assert_eq!(
        loaded.lifetime_total_tokens_saved,
        snap.lifetime_total_tokens_saved
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn commit_and_review_workflows_enforce_caveman_contracts() {
    let diff = "\
diff --git a/crates/opencaveman/src/level.rs b/crates/opencaveman/src/level.rs
--- a/crates/opencaveman/src/level.rs
+++ b/crates/opencaveman/src/level.rs
@@ -10,2 +10,4 @@
+pub fn normalize_level() -> &'static str {
+    \"full\"
+}
";
    let analysis = analyze_diff_for_commit(diff, None);
    assert_eq!(analysis.inferred_type, "feat");
    assert_eq!(analysis.inferred_scope.as_deref(), Some("opencaveman"));
    assert!(analysis.suggested_header.chars().count() <= 50);

    let formatted = validate_and_format_commit(
        "feat(opencaveman): Added ultra terse token compression support across every single module",
        &[
            "preserve AST code spans".to_string(),
            "Co-authored-by: Bot <bot@example.com>".to_string(),
        ],
    );
    assert!(formatted.subject_chars <= 50);
    assert!(formatted.subject.starts_with("feat(opencaveman): add "));
    assert_eq!(formatted.body_bullets.len(), 1);

    let hunks = analyze_diff_for_review(diff);
    assert_eq!(hunks.total_added, 3);
    assert_eq!(hunks.added_lines[0].1, 10);

    assert_eq!(normalize_review_output(&[]), "LGTM");
    let review_text = normalize_review_output(&[ReviewFinding {
        file: Some("src/auth.rs".to_string()),
        line: 42,
        severity: "security".to_string(),
        message: "unchecked token expiry. Validate `exp > now`".to_string(),
    }]);
    assert_eq!(
        review_text,
        "src/auth.rs:L42: security: unchecked token expiry. Validate `exp > now`."
    );

    assert_eq!(builtin_caveman_skills().len(), 4);
    assert_eq!(cavecrew_subagent_specs().len(), 3);
}
