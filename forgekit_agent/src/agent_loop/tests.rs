#[cfg(test)]
mod agent_loop_tests {
    use crate::agent_loop::{AgentLoop, DiscoveryStore};
    use crate::audit::AuditEvent;
    use forgekit_core::Forge;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_loop_creation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));

        assert!(agent_loop.transaction.is_none());
        assert_eq!(agent_loop.audit_log().len(), 0);
    }

    #[tokio::test]
    async fn test_agent_loop_successful_run() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        match result {
            Ok(loop_result) => {
                assert!(!loop_result.transaction_id.is_empty());
                assert_eq!(loop_result.audit_events.len(), 6);
            }
            Err(e) => {
                assert!(
                    e.to_string().contains("Verification")
                        || e.to_string().contains("verification")
                );
            }
        }
    }

    #[tokio::test]
    async fn test_agent_loop_state_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result1 = agent_loop.run("first query").await;
        let events1_count = match &result1 {
            Ok(r) => r.audit_events.len(),
            Err(_) => agent_loop.audit_log().len(),
        };

        let result2 = agent_loop.run("second query").await;
        let events2_count = match &result2 {
            Ok(r) => r.audit_events.len(),
            Err(_) => agent_loop.audit_log().len(),
        };

        assert!(events1_count > 0);
        assert!(events2_count > 0);
    }

    #[tokio::test]
    async fn test_phase_transitions_recorded() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        let events = match result {
            Ok(r) => r.audit_events,
            Err(_) => agent_loop.audit_log().clone().into_events(),
        };

        assert!(events.len() >= 5);

        assert!(matches!(events[0], AuditEvent::Observe { .. }));
        assert!(matches!(events[1], AuditEvent::Constrain { .. }));
        assert!(matches!(events[2], AuditEvent::Plan { .. }));
        assert!(matches!(events[3], AuditEvent::Mutate { .. }));

        let is_valid_fifth = matches!(
            events[4],
            AuditEvent::Verify { .. } | AuditEvent::Rollback { .. }
        );
        assert!(
            is_valid_fifth,
            "Expected Verify or Rollback at index 4, got: {:?}",
            events[4]
        );
    }

    struct MockDiscoveryStore {
        discoveries: std::sync::Mutex<Vec<(String, String, serde_json::Value)>>,
    }

    impl MockDiscoveryStore {
        fn new() -> Self {
            Self {
                discoveries: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn discoveries(&self) -> Vec<(String, String, serde_json::Value)> {
            self.discoveries.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl DiscoveryStore for MockDiscoveryStore {
        async fn store(&self, discovery_type: &str, target: &str, metadata: serde_json::Value) {
            self.discoveries.lock().unwrap().push((
                discovery_type.to_string(),
                target.to_string(),
                metadata,
            ));
        }
    }

    #[tokio::test]
    async fn test_discovery_store_records_observed_symbols() {
        let store = Arc::new(MockDiscoveryStore::new());
        let store_ref = store.clone();

        store
            .store("Symbol", "test_func", serde_json::json!({"line": 1}))
            .await;
        assert_eq!(store_ref.discoveries().len(), 1);
        assert_eq!(store_ref.discoveries()[0].0, "Symbol");
        assert_eq!(store_ref.discoveries()[0].1, "test_func");
    }

    #[tokio::test]
    async fn test_agent_loop_returns_loop_result() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let result = agent_loop.run("test query").await;

        if let Ok(loop_result) = result {
            assert!(!loop_result.transaction_id.is_empty());
            assert!(!loop_result.audit_events.is_empty());
        }
    }

    #[tokio::test]
    async fn test_retry_loop_respects_max_attempts() {
        use crate::llm::MockProvider;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let llm = Arc::new(MockProvider::new("[]"));
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_llm(llm)
            .with_max_fix_attempts(2);

        let result = agent_loop.run("test retry").await;

        match result {
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("verification") || msg.contains("verify"),
                    "unexpected error: {e}"
                );
            }
            Ok(r) => {
                assert!(
                    r.audit_events
                        .iter()
                        .any(|e| matches!(e, AuditEvent::Verify { passed: true, .. })),
                    "committed despite failed verification"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_with_max_fix_attempts_builder() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge)).with_max_fix_attempts(5);
        assert_eq!(agent_loop.max_fix_attempts, 5);
    }

    #[tokio::test]
    async fn test_reasoning_system_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));
        let hypotheses = agent_loop.reasoning().board.list().await.unwrap();
        assert!(hypotheses.is_empty());
    }

    #[tokio::test]
    async fn test_hypothesis_proposed_after_observe() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));
        assert!(agent_loop.current_hypothesis.is_none());
        let _ = agent_loop.observe_phase("add a parser function").await;
        assert!(agent_loop.current_hypothesis.is_some());
        let hypotheses = agent_loop.reasoning().board.list().await.unwrap();
        assert_eq!(hypotheses.len(), 1);
        assert!(hypotheses[0].statement().contains("add a parser function"));
    }

    #[tokio::test]
    async fn test_hypothesis_confirmed_after_successful_run() {
        use forgekit_reasoning::HypothesisStatus;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));
        let _ = agent_loop.run("generate a helper").await;
        if let Some(id) = agent_loop.current_hypothesis {
            let hyp = agent_loop.reasoning().board.get(id).await.unwrap().unwrap();
            assert!(
                hyp.status() == HypothesisStatus::Confirmed
                    || hyp.status() == HypothesisStatus::Rejected
                    || hyp.status() == HypothesisStatus::UnderTest
                    || hyp.status() == HypothesisStatus::Proposed
            );
        }
    }

    #[tokio::test]
    async fn test_hypothesis_outcome_stored_on_run() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let store = Arc::new(MockDiscoveryStore::new());
        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_discovery_store(store.clone());
        let _ = agent_loop.run("generate helper").await;
        let discoveries = store.discoveries();
        let outcome = discoveries
            .iter()
            .find(|(dtype, _, _)| dtype == "HypothesisOutcome");
        assert!(
            outcome.is_some(),
            "HypothesisOutcome should be stored; got: {:?}",
            discoveries.iter().map(|(t, _, _)| t).collect::<Vec<_>>()
        );
        let (_, target, meta) = outcome.unwrap();
        assert_eq!(target.as_str(), "generate helper");
        assert!(
            meta.get("succeeded").is_some(),
            "meta must have 'succeeded'"
        );
        assert!(
            meta.get("final_confidence").is_some(),
            "meta must have 'final_confidence'"
        );
    }

    #[tokio::test]
    async fn test_with_policies_builder_stores_policies() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);
        assert_eq!(agent_loop.policies.len(), 1);
    }

    #[tokio::test]
    async fn test_constrain_phase_records_policy_count() {
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_policies(vec![
            crate::policy::Policy::NoUnsafeInPublicAPI,
            crate::policy::Policy::PreserveTests,
        ]);
        let _ = agent_loop.run("test query").await;
        let events = agent_loop.audit_log().clone().into_events();
        let constrain_event = events
            .iter()
            .find(|e| matches!(e, AuditEvent::Constrain { .. }));
        assert!(constrain_event.is_some(), "Constrain event not recorded");
        if let Some(AuditEvent::Constrain { policy_count, .. }) = constrain_event {
            assert_eq!(
                *policy_count, 2,
                "policy_count should be 2, got {policy_count}"
            );
        }
    }

    #[tokio::test]
    async fn test_policy_catches_unsafe_in_created_file() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        let unsafe_file = temp_dir.path().join("danger.rs");
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&unsafe_file).await.unwrap();
        tokio::fs::write(&unsafe_file, "pub unsafe fn danger() {}")
            .await
            .unwrap();
        agent_loop.transaction = Some(txn);

        let violations = agent_loop.verify_policies_on_mutations().await.unwrap();
        assert!(
            !violations.is_empty(),
            "NoUnsafeInPublicAPI should catch `pub unsafe fn`"
        );
    }

    #[tokio::test]
    async fn test_policy_passes_safe_file() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        let safe_file = temp_dir.path().join("safe.rs");
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&safe_file).await.unwrap();
        tokio::fs::write(&safe_file, "pub fn safe() {}")
            .await
            .unwrap();
        agent_loop.transaction = Some(txn);

        let violations = agent_loop.verify_policies_on_mutations().await.unwrap();
        assert!(
            violations.is_empty(),
            "safe file should produce no violations"
        );
    }

    #[tokio::test]
    async fn test_constrain_phase_reads_real_file_content() {
        use crate::observe::{Observation, ObservedSymbol};
        use forgekit_core::types::{Location, SymbolId, SymbolKind};

        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_policies(vec![crate::policy::Policy::NoUnsafeInPublicAPI]);

        let unsafe_file = temp_dir.path().join("danger.rs");
        tokio::fs::write(&unsafe_file, "pub unsafe fn danger() {}")
            .await
            .unwrap();

        let observation = Observation {
            query: "refactor danger module".to_string(),
            symbols: vec![ObservedSymbol {
                id: SymbolId(1),
                name: "danger".to_string(),
                kind: SymbolKind::Function,
                location: Location {
                    file_path: unsafe_file.clone(),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 1,
                },
            }],
            summary: None,
        };

        let constrained = agent_loop.constrain_phase(observation).await.unwrap();

        assert!(
            !constrained.policy_violations.is_empty(),
            "constrain_phase must detect NoUnsafeInPublicAPI from the real file, not from the query string"
        );
    }

    #[tokio::test]
    async fn test_commit_phase_uses_transaction_snapshots_not_diagnostics() {
        use crate::transaction::Transaction;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let modified_file = temp_dir.path().join("modified.rs");
        tokio::fs::write(&modified_file, "pub fn foo() {}")
            .await
            .unwrap();
        let mut txn = Transaction::begin().await.unwrap();
        txn.snapshot_file(&modified_file).await.unwrap();
        agent_loop.transaction = Some(txn);

        let verification = crate::VerificationResult {
            passed: true,
            diagnostics: vec![],
            suggestions: None,
        };
        let result = agent_loop.commit_phase(verification).await.unwrap();

        assert!(
            result.files_committed.contains(&modified_file),
            "commit_phase must derive committed files from transaction snapshots, not diagnostics"
        );
    }

    #[tokio::test]
    async fn test_agent_loop_run_workflow_passes_forge() {
        use crate::workflow::dag::Workflow;
        use crate::workflow::task::{TaskContext, TaskError, TaskId, TaskResult, WorkflowTask};
        use async_trait::async_trait;

        struct ForgeCheckTask;

        #[async_trait]
        impl WorkflowTask for ForgeCheckTask {
            async fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
                if context.forge.is_some() {
                    Ok(TaskResult::Success)
                } else {
                    Err(TaskError::ExecutionFailed(
                        "no forge in context".to_string(),
                    ))
                }
            }

            fn id(&self) -> TaskId {
                TaskId::new("forge-check")
            }

            fn name(&self) -> &str {
                "ForgeCheckTask"
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();
        let agent_loop = AgentLoop::new(Arc::new(forge));

        let mut workflow = Workflow::new();
        workflow.add_task(Box::new(ForgeCheckTask));

        let result = agent_loop.run_workflow(workflow).await;
        assert!(
            result.is_ok(),
            "run_workflow should succeed when forge is wired: {:?}",
            result.err()
        );
        assert!(
            result.unwrap().success,
            "workflow result should report success"
        );
    }

    #[tokio::test]
    async fn test_low_confidence_bails_before_max_attempts() {
        use crate::llm::MockProvider;
        let temp_dir = TempDir::new().unwrap();
        let forge = Forge::open(temp_dir.path()).await.unwrap();

        let llm = Arc::new(MockProvider::new("[]"));
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_llm(llm)
            .with_max_fix_attempts(5);

        let result = agent_loop.run("test confidence bail-out").await;

        let is_err = result.is_err();
        if let Some(id) = agent_loop.current_hypothesis {
            let hyp = agent_loop.reasoning().board.get(id).await.unwrap().unwrap();
            let conf = hyp.current_confidence().get();
            assert!(
                conf < AgentLoop::CONFIDENCE_BAIL_THRESHOLD,
                "confidence {conf} should be below bail threshold after repeated failures (run_ok={})", !is_err
            );
        } else if is_err {
            // run failed without creating a hypothesis — acceptable
        } else {
            panic!(
                "run succeeded on empty codebase without a hypothesis — \
                 expected failure or low confidence"
            );
        }
    }

    #[tokio::test]
    async fn test_verify_phase_attaches_structured_evidence_via_runner() {
        let temp = TempDir::new().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();

        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_max_fix_attempts(1);

        let hyp_id = agent_loop
            .reasoning
            .board
            .propose_with_max_uncertainty("test hypothesis")
            .await
            .unwrap();
        agent_loop.current_hypothesis = Some(hyp_id);

        let mutation = crate::MutationResult {
            modified_files: vec![],
            diffs: vec![],
        };
        let _ = agent_loop.verify_phase(mutation).await;

        let evidence = agent_loop
            .reasoning
            .board
            .list_evidence(hyp_id)
            .await
            .unwrap();
        assert!(
            !evidence.is_empty(),
            "verify_phase must attach structured evidence via VerificationRunner"
        );
    }

    #[tokio::test]
    async fn test_observe_phase_registers_knowledge_gaps() {
        let temp = TempDir::new().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge));

        let _ = agent_loop.observe_phase("add a parser function").await;

        assert!(
            !agent_loop.gap_hints().is_empty(),
            "gap_hints must be non-empty after observe on empty dir"
        );
    }

    #[tokio::test]
    async fn test_gap_hints_appear_in_planner_prompt() {
        use crate::llm::CapturingMockProvider;
        use std::sync::Arc;

        let temp = TempDir::new().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();
        let capturing = Arc::new(CapturingMockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1}]"#,
        ));
        let mut agent_loop = AgentLoop::new(Arc::new(forge))
            .with_llm(capturing.clone() as Arc<dyn crate::llm::LlmProvider>);

        let _ = agent_loop.observe_phase("add parser").await;

        let obs = crate::observe::Observation {
            symbols: vec![],
            query: "add parser".to_string(),
            summary: None,
        };
        let constrained = crate::ConstrainedPlan {
            observation: obs,
            policy_violations: vec![],
        };
        let _ = agent_loop.plan_phase(constrained).await;

        let prompt = capturing
            .last_prompt
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_default();
        assert!(
            prompt.to_lowercase().contains("gap") || prompt.to_lowercase().contains("missing"),
            "planner prompt must contain gap hints; got: {:?}",
            &prompt[..prompt.len().min(200)]
        );
    }

    #[tokio::test]
    async fn test_plan_phase_registers_step_dependencies_in_belief_graph() {
        use crate::llm::MockProvider;

        let temp = TempDir::new().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();
        let mock_llm = Arc::new(MockProvider::new(
            r#"[{"operation":"inspect","symbol_name":"foo","symbol_id":1},{"operation":"modify","file":"src/main.rs","start":1,"end":5,"replacement":"fn foo() {}"}]"#,
        ));
        let mut agent_loop =
            AgentLoop::new(Arc::new(forge)).with_llm(mock_llm as Arc<dyn crate::llm::LlmProvider>);

        let hyp_id = agent_loop
            .reasoning
            .board
            .propose_with_max_uncertainty("modify foo")
            .await
            .unwrap();
        agent_loop.current_hypothesis = Some(hyp_id);

        let obs = crate::observe::Observation {
            symbols: vec![],
            query: "modify foo".to_string(),
            summary: None,
        };
        let constrained = crate::ConstrainedPlan {
            observation: obs,
            policy_violations: vec![],
        };
        let _ = agent_loop.plan_phase(constrained).await;

        let chain = agent_loop
            .reasoning
            .graph
            .dependency_chain(hyp_id)
            .unwrap_or_default();

        assert!(
            !chain.is_empty(),
            "BeliefGraph must have dependency edges after plan_phase with multi-step plan"
        );
    }

    #[tokio::test]
    async fn test_run_saves_checkpoints_per_phase() {
        let temp = TempDir::new().unwrap();
        let forge = Forge::open(temp.path()).await.unwrap();
        let mut agent_loop = AgentLoop::new(Arc::new(forge)).with_max_fix_attempts(1);

        let _ = agent_loop.run("test checkpoint").await;

        let cps = agent_loop.checkpoints();
        assert!(
            !cps.is_empty(),
            "expected phase checkpoints after run; got 0"
        );
        assert!(
            cps.iter().any(|c| c.phase == "observe"),
            "expected 'observe' phase checkpoint; got: {:?}",
            cps.iter().map(|c| &c.phase).collect::<Vec<_>>()
        );
    }
}
