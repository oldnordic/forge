#![cfg(feature = "envoy")]

use forgekit_agent::evidence::*;

#[test]
fn test_sha256_hex_deterministic() {
    let a = sha256_hex("hello");
    let b = sha256_hex("hello");
    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
}

#[test]
fn test_sha256_hex_known_value() {
    let hash = sha256_hex(b"");
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_commit_type_classify_feature() {
    assert_eq!(
        CommitType::classify("feat: add paging"),
        CommitType::Feature
    );
    assert_eq!(
        CommitType::classify("feat(cli): add flag"),
        CommitType::Feature
    );
}

#[test]
fn test_commit_type_classify_fix() {
    assert_eq!(CommitType::classify("fix: resolve panic"), CommitType::Fix);
}

#[test]
fn test_commit_type_classify_refactor() {
    assert_eq!(
        CommitType::classify("refactor: split module"),
        CommitType::Refactor
    );
}

#[test]
fn test_commit_type_classify_test() {
    assert_eq!(
        CommitType::classify("test: add integration tests"),
        CommitType::Test
    );
}

#[test]
fn test_commit_type_classify_docs() {
    assert_eq!(
        CommitType::classify("docs: update readme"),
        CommitType::Docs
    );
}

#[test]
fn test_commit_type_classify_release() {
    assert_eq!(CommitType::classify("release: v3.0.4"), CommitType::Release);
}

#[test]
fn test_commit_type_classify_bump_version() {
    assert_eq!(
        CommitType::classify("bump version to 3.0.4"),
        CommitType::Release
    );
    assert_eq!(
        CommitType::classify("chore: bump version to 3.0.4"),
        CommitType::Chore
    );
}

#[test]
fn test_commit_type_classify_ci() {
    assert_eq!(CommitType::classify("ci: fix workflow"), CommitType::Ci);
}

#[test]
fn test_commit_type_classify_style() {
    assert_eq!(CommitType::classify("style: cargo fmt"), CommitType::Style);
}

#[test]
fn test_commit_type_classify_merge() {
    assert_eq!(CommitType::classify("merge branch"), CommitType::Merge);
}

#[test]
fn test_commit_type_classify_unknown_defaults_to_feature() {
    assert_eq!(CommitType::classify("random message"), CommitType::Feature);
}

#[test]
fn test_extract_feature_tag() {
    assert_eq!(
        extract_feature_tag("feat(15-04): add paging"),
        Some("15-04".into())
    );
    assert_eq!(
        extract_feature_tag("fix(cli): resolve panic"),
        Some("cli".into())
    );
    assert_eq!(
        extract_feature_tag("refactor(core): split module"),
        Some("core".into())
    );
    assert_eq!(extract_feature_tag("chore: cleanup"), None);
    assert_eq!(extract_feature_tag(""), None);
}

#[test]
fn test_tool_category_default() {
    assert_eq!(ToolCategory::default(), ToolCategory::Other);
}

#[test]
fn test_commit_type_default() {
    assert_eq!(CommitType::default(), CommitType::Feature);
}

#[test]
fn test_severity_default() {
    assert_eq!(Severity::default(), Severity::Medium);
}

#[test]
fn test_fix_type_default() {
    assert_eq!(FixType::default(), FixType::CompileError);
}

#[test]
fn test_prompt_record_serialization() {
    let record = PromptRecord {
        role: "user".into(),
        sequence: 1,
        input_hash: sha256_hex("test"),
        input_tokens: Some(100),
        output_hash: Some(sha256_hex("response")),
        output_tokens: Some(50),
        latency_ms: Some(200),
        model: Some("claude-sonnet".into()),
        cost_usd: Some(0.003),
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: PromptRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.role, "user");
    assert_eq!(parsed.sequence, 1);
    assert_eq!(parsed.input_tokens, Some(100));
}

#[test]
fn test_tool_call_evidence_serialization() {
    let record = ToolCallEvidence {
        tool_name: "magellan_find".into(),
        tool_version: Some("3.1.8".into()),
        input_hash: sha256_hex("args"),
        input_summary: "--name pagerank".into(),
        output_hash: Some(sha256_hex("result")),
        output_summary: Some("found 3 refs".into()),
        exit_status: "success".into(),
        latency_ms: 42,
        input_tokens_est: Some(300),
        tool_category: ToolCategory::GroundedQuery,
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: ToolCallEvidence = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.tool_name, "magellan_find");
    assert_eq!(parsed.tool_category, ToolCategory::GroundedQuery);
}

#[test]
fn test_file_write_record_serialization() {
    let record = FileWriteRecord {
        file_path: "src/algo.rs".into(),
        file_id: sha256_hex("src/algo.rs"),
        before_hash: Some(sha256_hex("old")),
        after_hash: sha256_hex("new"),
        lines_added: 42,
        lines_deleted: 10,
        lines_changed: 5,
        write_type: "edit".into(),
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: FileWriteRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.file_path, "src/algo.rs");
    assert_eq!(parsed.lines_added, 42);
}

#[test]
fn test_commit_record_serialization() {
    let record = CommitRecord {
        commit_sha: "abc123".into(),
        parent_sha: Some("def456".into()),
        message: "feat(core): add pagerank".into(),
        author: "test".into(),
        files_changed: 3,
        lines_inserted: 100,
        lines_deleted: 20,
        commit_type: CommitType::Feature,
        feature_tag: Some("core".into()),
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: CommitRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.commit_type, CommitType::Feature);
    assert_eq!(parsed.feature_tag, Some("core".into()));
}

#[test]
fn test_fix_chain_record_serialization() {
    let record = FixChainRecord {
        bug_commit_sha: "abc".into(),
        fix_commit_sha: "def".into(),
        fix_type: FixType::LogicBug,
        severity: Severity::High,
        cycles_to_fix: 2,
        time_to_fix_ms: 5000,
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: FixChainRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.fix_type, FixType::LogicBug);
    assert_eq!(parsed.severity, Severity::High);
}

#[tokio::test]
async fn test_mock_evidence_recorder() {
    use std::sync::Arc;

    let recorder = Arc::new(MockEvidenceRecorder::new());
    let session_id = "test-session";

    recorder
        .record_prompt(
            session_id,
            &PromptRecord {
                role: "user".into(),
                sequence: 1,
                input_hash: sha256_hex("test"),
                ..Default::default()
            },
        )
        .await;

    recorder
        .record_tool_call(
            session_id,
            &ToolCallEvidence {
                tool_name: "magellan_find".into(),
                input_hash: sha256_hex("args"),
                input_summary: "--name foo".into(),
                exit_status: "success".into(),
                tool_category: ToolCategory::GroundedQuery,
                ..Default::default()
            },
        )
        .await;

    recorder
        .record_file_write(
            session_id,
            &FileWriteRecord {
                file_path: "src/main.rs".into(),
                file_id: sha256_hex("src/main.rs"),
                after_hash: sha256_hex("new"),
                write_type: "edit".into(),
                ..Default::default()
            },
        )
        .await;

    assert_eq!(recorder.prompts.lock().len(), 1);
    assert_eq!(recorder.tool_calls.lock().len(), 1);
    assert_eq!(recorder.file_writes.lock().len(), 1);

    assert_eq!(recorder.tool_calls.lock()[0].1.tool_name, "magellan_find");
    assert_eq!(
        recorder.tool_calls.lock()[0].1.tool_category,
        ToolCategory::GroundedQuery
    );
}

#[tokio::test]
async fn test_forge_session_lifecycle() {
    use std::sync::Arc;

    let recorder = Arc::new(MockEvidenceRecorder::new());
    let session = ForgeSession::new(
        recorder.clone() as Arc<dyn EvidenceRecorder>,
        "test-project",
        "test-tool",
        None,
    );

    let sid = session.session_id();
    assert!(!sid.is_empty());

    session.record_prompt(PromptRecord {
        role: "user".into(),
        input_hash: sha256_hex("prompt"),
        input_tokens: Some(1000),
        output_tokens: Some(500),
        cost_usd: Some(0.015),
        ..Default::default()
    });

    session.record_tool_call(ToolCallEvidence {
        tool_name: "llmgrep".into(),
        input_hash: sha256_hex("query"),
        input_summary: "search pagerank".into(),
        exit_status: "success".into(),
        tool_category: ToolCategory::GroundedQuery,
        input_tokens_est: Some(200),
        ..Default::default()
    });

    session.record_file_write(FileWriteRecord {
        file_path: "src/algo.rs".into(),
        file_id: sha256_hex("src/algo.rs"),
        after_hash: sha256_hex("content"),
        lines_added: 10,
        write_type: "edit".into(),
        ..Default::default()
    });

    session.end("success").await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let prompts = recorder.prompts.lock();
    let tool_calls = recorder.tool_calls.lock();
    let file_writes = recorder.file_writes.lock();

    assert!(prompts.len() >= 2, "should have init + recorded prompt");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(file_writes.len(), 1);

    assert_eq!(tool_calls[0].1.tool_category, ToolCategory::GroundedQuery);
    assert_eq!(file_writes[0].1.file_path, "src/algo.rs");
}

#[tokio::test]
async fn test_session_token_accumulation() {
    use std::sync::Arc;

    let recorder = Arc::new(MockEvidenceRecorder::new());
    let session = ForgeSession::new(
        recorder.clone() as Arc<dyn EvidenceRecorder>,
        "test",
        "test-tool",
        None,
    );

    session.record_prompt(PromptRecord {
        role: "user".into(),
        input_hash: sha256_hex("p1"),
        input_tokens: Some(500),
        output_tokens: Some(200),
        cost_usd: Some(0.01),
        ..Default::default()
    });

    session.record_prompt(PromptRecord {
        role: "assistant".into(),
        input_hash: sha256_hex("p2"),
        input_tokens: Some(300),
        output_tokens: Some(100),
        cost_usd: Some(0.005),
        ..Default::default()
    });

    session.end("success").await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let prompts = recorder.prompts.lock();
    let user_prompts: Vec<_> = prompts
        .iter()
        .filter(|(sid, _)| sid.starts_with("session-") || !sid.is_empty())
        .map(|(_, r)| r.clone())
        .collect();

    let total_input: u64 = user_prompts.iter().filter_map(|r| r.input_tokens).sum();
    let total_output: u64 = user_prompts.iter().filter_map(|r| r.output_tokens).sum();
    let total_cost: f64 = user_prompts.iter().filter_map(|r| r.cost_usd).sum();

    assert_eq!(total_input, 800);
    assert_eq!(total_output, 300);
    assert!((total_cost - 0.015).abs() < 0.0001);
}
