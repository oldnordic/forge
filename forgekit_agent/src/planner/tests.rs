use super::*;

#[tokio::test]
async fn test_planner_creation() {
    let _planner = Planner::new();
}

#[tokio::test]
async fn test_generate_steps() {
    let planner = Planner::new();

    let observation = crate::observe::Observation {
        query: "test".to_string(),
        symbols: vec![],
        summary: None,
    };

    let steps = planner.generate_steps(&observation).await.unwrap();
    // Should succeed (even if empty)
    assert!(steps.is_empty());
}

#[tokio::test]
async fn test_detect_conflicts_empty() {
    let planner = Planner::new();

    let steps = vec![];
    let conflicts = planner.detect_conflicts(&steps).unwrap();
    assert!(conflicts.is_empty());
}

#[tokio::test]
async fn test_order_steps() {
    let planner = Planner::new();

    let mut steps = vec![
        PlanStep {
            description: "Delete foo".to_string(),
            operation: PlanOperation::Delete {
                name: "foo".to_string(),
                file: None,
            },
        },
        PlanStep {
            description: "Rename foo to bar".to_string(),
            operation: PlanOperation::Rename {
                old: "foo".to_string(),
                new: "bar".to_string(),
                file: None,
            },
        },
    ];

    planner.order_steps(&mut steps).unwrap();

    // Rename should now come before Delete
    assert!(matches!(steps[0].operation, PlanOperation::Rename { .. }));
    assert!(matches!(steps[1].operation, PlanOperation::Delete { .. }));
}

#[tokio::test]
async fn test_generate_rollback() {
    let planner = Planner::new();

    let steps = vec![PlanStep {
        description: "Create file".to_string(),
        operation: PlanOperation::Create {
            path: "/tmp/test.rs".to_string(),
            content: "fn test() {}".to_string(),
        },
    }];

    let rollback = planner.generate_rollback(&steps);

    assert_eq!(rollback.len(), 1);
    assert_eq!(rollback[0].description, "Rollback: Create file");
    assert!(matches!(
        rollback[0].operation,
        RollbackOperation::Delete { .. }
    ));
}

#[tokio::test]
async fn test_estimate_impact() {
    let planner = Planner::new();

    let steps = vec![PlanStep {
        description: "Create test.rs".to_string(),
        operation: PlanOperation::Create {
            path: "/tmp/test.rs".to_string(),
            content: "fn test() {}".to_string(),
        },
    }];

    let _impact = planner.estimate_impact(&steps).await.unwrap();
}

#[tokio::test]
async fn test_planner_no_llm_uses_regex() {
    let planner = Planner::new();
    assert!(planner.llm.is_none());

    let observation = crate::observe::Observation {
        query: "rename old_func to new_func".to_string(),
        symbols: vec![],
        summary: None,
    };

    let steps = planner.generate_steps(&observation).await.unwrap();
    // No symbols → regex path produces empty steps, but doesn't error
    assert!(steps.is_empty());
}

#[tokio::test]
async fn test_planner_llm_generates_steps() {
    use std::sync::Arc;

    // MockProvider returns valid JSON steps
    let json_steps = r#"[{"operation":"inspect","symbol_name":"auth_middleware","symbol_id":42}]"#;
    let mock = Arc::new(crate::llm::MockProvider::new(json_steps));

    let planner = Planner::new().with_llm(mock);
    assert!(planner.llm.is_some());

    let observation = crate::observe::Observation {
        query: "where is the auth middleware?".to_string(),
        symbols: vec![],
        summary: None,
    };

    let steps = planner.generate_steps(&observation).await.unwrap();
    assert_eq!(steps.len(), 1);
    assert!(matches!(
        &steps[0].operation,
        PlanOperation::Inspect { symbol_name, .. } if symbol_name == "auth_middleware"
    ));
}

#[tokio::test]
async fn test_planner_llm_fallback_on_parse_error() {
    use std::sync::Arc;

    // MockProvider returns garbage — should fall back to regex
    let mock = Arc::new(crate::llm::MockProvider::new("not valid json at all"));

    let planner = Planner::new().with_llm(mock);

    let observation = crate::observe::Observation {
        query: "inspect test query".to_string(),
        symbols: vec![],
        summary: None,
    };

    // Should NOT error — falls back to regex detect_intent
    let steps = planner.generate_steps(&observation).await.unwrap();
    // No symbols matched, regex produces Inspect with no targets → empty
    assert!(steps.is_empty());
}

// ── Task 2: attempt history ──────────────────────────────────────────

#[tokio::test]
async fn test_generate_fix_steps_accepts_previous_steps() {
    use crate::llm::MockProvider;
    let llm = Arc::new(MockProvider::new(
        r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
    ));
    let planner = Planner::new().with_llm(llm);
    let obs = crate::observe::Observation {
        query: "fix the error".to_string(),
        symbols: vec![],
        summary: None,
    };
    let prev = vec![PlanStep {
        description: "Previous attempt".to_string(),
        operation: PlanOperation::Create {
            path: "src/foo.rs".to_string(),
            content: "fn foo() {}".to_string(),
        },
    }];
    // Should compile and run — previous_steps accepted without error
    let result = planner
        .generate_fix_steps(&obs, &["compile error".to_string()], &prev)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_generate_fix_steps_empty_previous_no_error() {
    use crate::llm::MockProvider;
    let llm = Arc::new(MockProvider::new("[]"));
    let planner = Planner::new().with_llm(llm);
    let obs = crate::observe::Observation {
        query: "fix".to_string(),
        symbols: vec![],
        summary: None,
    };
    let result = planner
        .generate_fix_steps(&obs, &["error".to_string()], &[])
        .await;
    assert!(result.is_ok());
}

// ── Gap 7: retry memoization ─────────────────────────────────────────

#[tokio::test]
async fn test_dedup_filters_repeated_fix_step() {
    use crate::llm::MockProvider;
    let llm = Arc::new(MockProvider::new(
        r#"[{"operation":"modify","file":"src/lib.rs","start":10,"end":20,"replacement":"fixed"}]"#,
    ));
    let planner = Planner::new().with_llm(llm);
    let obs = crate::observe::Observation {
        query: "fix error".to_string(),
        symbols: vec![],
        summary: None,
    };
    let prev = vec![PlanStep {
        description: "Modify src/lib.rs:10-20".to_string(),
        operation: PlanOperation::Modify {
            file: "src/lib.rs".to_string(),
            start: 10,
            end: 20,
            replacement: "fixed".to_string(),
        },
    }];
    let result = planner
        .generate_fix_steps(&obs, &["error".to_string()], &prev)
        .await
        .unwrap();
    assert!(
        result.is_empty(),
        "repeated operation must be filtered out, got: {result:?}"
    );
}

#[tokio::test]
async fn test_dedup_keeps_new_fix_step() {
    use crate::llm::MockProvider;
    let llm = Arc::new(MockProvider::new(
        r#"[{"operation":"modify","file":"src/lib.rs","start":30,"end":40,"replacement":"new_fix"}]"#,
    ));
    let planner = Planner::new().with_llm(llm);
    let obs = crate::observe::Observation {
        query: "fix error".to_string(),
        symbols: vec![],
        summary: None,
    };
    let prev = vec![PlanStep {
        description: "Modify src/lib.rs:10-20".to_string(),
        operation: PlanOperation::Modify {
            file: "src/lib.rs".to_string(),
            start: 10,
            end: 20,
            replacement: "old_fix".to_string(),
        },
    }];
    let result = planner
        .generate_fix_steps(&obs, &["error".to_string()], &prev)
        .await
        .unwrap();
    assert_eq!(result.len(), 1, "new operation must pass through dedup");
}

#[tokio::test]
async fn test_dedup_mixed_new_and_repeated() {
    use crate::llm::MockProvider;
    let llm = Arc::new(MockProvider::new(
        r#"[
            {"operation":"modify","file":"src/lib.rs","start":10,"end":20,"replacement":"already_tried"},
            {"operation":"create","path":"src/new.rs","content":"fn new_fix() {}"}
        ]"#,
    ));
    let planner = Planner::new().with_llm(llm);
    let obs = crate::observe::Observation {
        query: "fix error".to_string(),
        symbols: vec![],
        summary: None,
    };
    let prev = vec![PlanStep {
        description: "Modify src/lib.rs:10-20".to_string(),
        operation: PlanOperation::Modify {
            file: "src/lib.rs".to_string(),
            start: 10,
            end: 20,
            replacement: "already_tried".to_string(),
        },
    }];
    let result = planner
        .generate_fix_steps(&obs, &["error".to_string()], &prev)
        .await
        .unwrap();
    assert_eq!(result.len(), 1, "only the new operation should remain");
    assert!(
        matches!(&result[0].operation, PlanOperation::Create { path, .. } if path == "src/new.rs"),
        "remaining step should be the Create, got: {:?}",
        result[0].operation
    );
}

// ── INT-11: Planner retry memoization ───────────────────────────────

#[tokio::test]
async fn test_planner_memoizes_attempted_operations() {
    use crate::llm::MockProvider;
    // LLM returns an Inspect step
    let mock = Arc::new(MockProvider::new(
        r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
    ));
    let mut planner = Planner::new().with_llm(mock);
    let obs = crate::observe::Observation {
        query: "fix it".to_string(),
        symbols: vec![],
        summary: None,
    };

    // First call: should return the step
    let first = planner.fix_once(&obs, &["err".to_string()]).await.unwrap();
    assert_eq!(first.len(), 1);

    // Second call: LLM returns same step — planner should filter it as already tried
    let second = planner.fix_once(&obs, &["err".to_string()]).await.unwrap();
    assert!(
        second.is_empty(),
        "second call should deduplicate already-attempted operations"
    );
}

// ── INT-3: AgentContext prefix in LLM prompts ────────────────────────

#[tokio::test]
async fn test_planner_context_prefix_in_generate_steps() {
    use crate::llm::CapturingMockProvider;
    let mock = Arc::new(CapturingMockProvider::new(
        r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
    ));
    let ctx = crate::context::AgentContext::from_path(std::path::Path::new("/tmp/test-proj"));
    let planner = Planner::new().with_context(&ctx).with_llm(mock.clone());

    let obs = crate::observe::Observation {
        query: "find the bug".to_string(),
        symbols: vec![],
        summary: None,
    };
    let _ = planner.generate_steps(&obs).await.unwrap();

    let captured = mock.last_prompt.lock().unwrap().clone().unwrap_or_default();
    assert!(
        captured.contains("[Project:"),
        "prompt should contain context prefix, got: {captured:?}"
    );
}

#[tokio::test]
async fn test_planner_context_prefix_in_generate_fix_steps() {
    use crate::llm::CapturingMockProvider;
    let mock = Arc::new(CapturingMockProvider::new(
        r#"[{"operation":"inspect","symbol_name":"bar","symbol_id":2}]"#,
    ));
    let ctx = crate::context::AgentContext::from_path(std::path::Path::new("/tmp/test-proj"));
    let planner = Planner::new().with_context(&ctx).with_llm(mock.clone());

    let obs = crate::observe::Observation {
        query: "fix compile error".to_string(),
        symbols: vec![],
        summary: None,
    };
    let _ = planner
        .generate_fix_steps(&obs, &["error: mismatched types".to_string()], &[])
        .await
        .unwrap();

    let captured = mock.last_prompt.lock().unwrap().clone().unwrap_or_default();
    assert!(
        captured.contains("[Project:"),
        "fix prompt should contain context prefix, got: {captured:?}"
    );
}

// ── INT-10: CodeGenerator enriches Create steps ──────────────────────

#[tokio::test]
async fn test_planner_enriches_create_step_via_generator() {
    use crate::generate::Generator;
    use crate::llm::MockProvider;

    // Planning LLM returns a Create step with bare description as content
    let planning_llm = Arc::new(MockProvider::new(
        r#"[{"operation":"create","path":"src/auth.rs","content":"authentication handler"}]"#,
    ));
    // Generator LLM returns actual Rust code
    let gen_llm = Arc::new(MockProvider::new(
        "fn authenticate(token: &str) -> bool { true }",
    ));

    let temp_dir = tempfile::TempDir::new().unwrap();
    let forge = forgekit_core::Forge::open(temp_dir.path()).await.unwrap();
    let generator = Arc::new(Generator::new(Arc::new(forge), gen_llm));

    let planner = Planner::new()
        .with_llm(planning_llm)
        .with_generator(generator);

    let obs = crate::observe::Observation {
        query: "add authentication".to_string(),
        symbols: vec![],
        summary: None,
    };

    let steps = planner.generate_steps(&obs).await.unwrap();
    assert_eq!(steps.len(), 1);

    if let PlanOperation::Create { content, .. } = &steps[0].operation {
        assert!(
            content.contains("fn authenticate"),
            "content should be generated code, got: {content:?}"
        );
    } else {
        panic!("expected Create step, got: {:?}", steps[0].operation);
    }
}
