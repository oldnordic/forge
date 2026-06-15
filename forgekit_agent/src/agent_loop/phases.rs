use super::*;

impl AgentLoop {
    pub(super) fn save_phase_checkpoint(&mut self, phase: &str) {
        self.checkpoints.push(AgentLoopCheckpoint {
            phase: phase.to_string(),
            timestamp: Utc::now(),
        });
    }

    pub(super) async fn analyze_gaps(&mut self, observation: &crate::observe::Observation) {
        use forgekit_reasoning::{GapCriticality, GapType, KnowledgeGapAnalyzer};

        let board = Arc::new(self.reasoning.board.clone());
        let graph = Arc::new(self.reasoning.graph.clone());
        let mut analyzer = KnowledgeGapAnalyzer::new(board, graph);

        if observation.symbols.is_empty() {
            let _ = analyzer
                .register_gap(
                    "No symbols found in observation — codebase may be empty or query too vague"
                        .to_string(),
                    GapCriticality::High,
                    GapType::MissingInformation,
                    self.current_hypothesis,
                )
                .await;
        }

        let suggestions = analyzer.get_suggestions(true).await;
        self.gap_hints = suggestions
            .into_iter()
            .map(|s| format!("gap[{:.2}]: {}", s.priority, s.rationale))
            .collect();
    }

    pub(super) async fn observe_phase(
        &mut self,
        query: &str,
    ) -> Result<Observation, crate::AgentError> {
        let mut observer =
            crate::observe::Observer::new((*self.forge).clone()).with_context(&self.context);
        if let Some(ref llm) = self.llm {
            observer = observer.with_llm(llm.clone());
        }
        let observation = observer
            .gather(query)
            .await
            .map_err(|e| crate::AgentError::ObservationFailed(e.to_string()))?;

        let symbol_count = observation.symbols.len();

        self.audit_log
            .record(AuditEvent::Observe {
                timestamp: Utc::now(),
                query: query.to_string(),
                symbol_count,
            })
            .await
            .map_err(|e| crate::AgentError::ObservationFailed(e.to_string()))?;

        self.last_observation = Some(observation.clone());

        if let Ok(id) = self
            .reasoning
            .board
            .propose_with_max_uncertainty(format!("task: {}", query))
            .await
        {
            self.current_hypothesis = Some(id);
        }

        self.analyze_gaps(&observation).await;

        Ok(observation)
    }

    pub(crate) async fn constrain_phase(
        &mut self,
        observation: Observation,
    ) -> Result<ConstrainedPlan, crate::AgentError> {
        let validator = PolicyValidator::new((*self.forge).clone());
        let policies = self.policies.clone();
        let mut all_violations = Vec::new();

        let mut seen = std::collections::HashSet::new();
        for symbol in &observation.symbols {
            let path = &symbol.location.file_path;
            if !seen.insert(path.clone()) {
                continue;
            }
            let content = match tokio::fs::read_to_string(path).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            let diff = crate::policy::Diff {
                file_path: path.clone(),
                original: content.clone(),
                modified: content,
                changes: Vec::new(),
            };
            let report = validator
                .validate(&diff, &policies)
                .await
                .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;
            all_violations.extend(report.violations);
        }

        let policy_count = policies.len();
        let violation_count = all_violations.len();

        self.audit_log
            .record(AuditEvent::Constrain {
                timestamp: Utc::now(),
                policy_count,
                violations: violation_count,
            })
            .await
            .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;

        Ok(ConstrainedPlan {
            observation,
            policy_violations: all_violations,
        })
    }

    pub(super) async fn plan_phase(
        &mut self,
        constrained: ConstrainedPlan,
    ) -> Result<ExecutionPlan, crate::AgentError> {
        let mut planner = crate::planner::Planner::new()
            .with_context(&self.context)
            .with_gap_hints(&self.gap_hints);
        if let Some(ref llm) = self.llm {
            planner = planner.with_llm(llm.clone()).with_generator(Arc::new(
                crate::generate::Generator::new(self.forge.clone(), llm.clone()),
            ));
        }

        let steps = planner
            .generate_steps(&constrained.observation)
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        let impact = planner
            .estimate_impact(&steps)
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;
        let estimated_files = impact.affected_files.len();

        let conflicts = planner
            .detect_conflicts(&steps)
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        if !conflicts.is_empty() {
            let details: Vec<String> = conflicts
                .iter()
                .map(|c| match &c.reason {
                    crate::planner::ConflictReason::OverlappingRegion { start, end } => {
                        format!("{} at lines {}-{}", c.file, start, end)
                    }
                })
                .collect();
            return Err(crate::AgentError::PlanningFailed(format!(
                "Found {} conflicts in plan: {}",
                conflicts.len(),
                details.join("; ")
            )));
        }

        let mut ordered_steps = steps;
        planner
            .order_steps(&mut ordered_steps)
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        let step_count = ordered_steps.len();

        if let Some(id) = self.current_hypothesis {
            let _ = self
                .reasoning
                .board
                .set_status(id, forgekit_reasoning::HypothesisStatus::UnderTest)
                .await;
            let (lh, lnh) = if ordered_steps.is_empty() {
                (0.2, 0.8)
            } else {
                (0.8, 0.2)
            };
            let _ = self.reasoning.board.update_with_evidence(id, lh, lnh).await;

            for step in &ordered_steps {
                let step_desc = format!("step: {:?}", step.operation);
                if let Ok(step_id) = self
                    .reasoning
                    .board
                    .propose_with_max_uncertainty(step_desc)
                    .await
                {
                    let _ = self.reasoning.add_dependency(id, step_id).await;
                }
            }
        }

        let rollback = planner.generate_rollback(&ordered_steps);

        self.audit_log
            .record(AuditEvent::Plan {
                timestamp: Utc::now(),
                step_count,
                estimated_files,
            })
            .await
            .map_err(|e| crate::AgentError::PlanningFailed(e.to_string()))?;

        Ok(ExecutionPlan {
            steps: ordered_steps,
            estimated_impact: impact,
            rollback_plan: rollback,
        })
    }

    pub(super) async fn mutate_phase(
        &mut self,
        plan: ExecutionPlan,
    ) -> Result<MutationResult, crate::AgentError> {
        let mut mutator = crate::mutate::Mutator::new();
        mutator
            .begin_transaction()
            .await
            .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;

        for step in &plan.steps {
            mutator
                .apply_step(step)
                .await
                .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;
        }

        let transaction = mutator.into_transaction()?;
        let modified_files: Vec<std::path::PathBuf> = transaction
            .snapshots()
            .iter()
            .map(|s| s.path.clone())
            .collect();
        let diffs: Vec<String> = transaction
            .snapshots()
            .iter()
            .filter(|s| !s.original_content.is_empty())
            .map(|s| {
                let path_display = s.path.display();
                format!(
                    "--- {}\n+++ {}\n{} bytes original",
                    path_display,
                    path_display,
                    s.original_content.len()
                )
            })
            .collect();
        self.transaction = Some(transaction);

        let files_modified: Vec<String> = modified_files
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        self.audit_log
            .record(AuditEvent::Mutate {
                timestamp: Utc::now(),
                files_modified,
            })
            .await
            .map_err(|e| crate::AgentError::MutationFailed(e.to_string()))?;

        Ok(MutationResult {
            modified_files,
            diffs,
        })
    }

    pub(super) async fn verify_phase(
        &mut self,
        result: MutationResult,
    ) -> Result<VerificationResult, crate::AgentError> {
        let mut verifier = crate::verify::Verifier::with_forge((*self.forge).clone());
        if let Some(ref llm) = self.llm {
            verifier = verifier.with_llm(llm.clone());
        }

        let report = if result.modified_files.is_empty() {
            verifier
                .verify(&self.codebase_path)
                .await
                .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?
        } else {
            verifier
                .verify_changes(&self.codebase_path, &result.modified_files, &result.diffs)
                .await
                .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?
        };

        if let Some(hyp_id) = self.current_hypothesis {
            let runner =
                forgekit_reasoning::VerificationRunner::new(Arc::new(self.reasoning.board.clone()), 1);

            let check_cmd = format!(
                "cargo check --manifest-path={}/Cargo.toml --message-format=short 2>&1",
                self.codebase_path.display()
            );
            let (on_pass, on_fail) = if report.passed {
                (
                    Some(forgekit_reasoning::verification::check::PassAction::SetStatus(
                        forgekit_reasoning::HypothesisStatus::Confirmed,
                    )),
                    None,
                )
            } else {
                (
                    None,
                    Some(forgekit_reasoning::verification::check::FailAction::SetStatus(
                        forgekit_reasoning::HypothesisStatus::Rejected,
                    )),
                )
            };

            if let Ok(check_id) = runner
                .register_check(
                    "cargo-check".to_string(),
                    hyp_id,
                    forgekit_reasoning::verification::check::VerificationCommand::ShellCommand(
                        check_cmd,
                    ),
                    std::time::Duration::from_secs(60),
                    on_pass,
                    on_fail,
                )
                .await
            {
                runner.execute_checks(vec![check_id]).await;
            }
        }

        let policy_violations = self
            .verify_policies_on_mutations()
            .await
            .unwrap_or_default();
        let final_passed = report.passed && policy_violations.is_empty();
        let final_diagnostic_count = report.diagnostics.len() + policy_violations.len();

        self.audit_log
            .record(AuditEvent::Verify {
                timestamp: Utc::now(),
                passed: final_passed,
                diagnostic_count: final_diagnostic_count,
            })
            .await
            .map_err(|e| crate::AgentError::VerificationFailed(e.to_string()))?;

        #[cfg(feature = "envoy")]
        if let Some(ref session) = self.session {
            for diag in &report.diagnostics {
                session.record_test_run(crate::evidence::TestRunRecord {
                    test_name: diag.message.split(':').next().unwrap_or("unknown").into(),
                    result: "fail".into(),
                    logs_summary: Some(diag.message.chars().take(500).collect()),
                    ..Default::default()
                });
            }
            if report.passed {
                session.record_test_run(crate::evidence::TestRunRecord {
                    test_name: "verification_gate".into(),
                    result: "pass".into(),
                    ..Default::default()
                });
            }
        }

        let mut all_diagnostics: Vec<String> = report
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect();
        all_diagnostics.extend(policy_violations);

        Ok(VerificationResult {
            passed: final_passed,
            diagnostics: all_diagnostics,
            suggestions: report.suggestions,
        })
    }

    pub(crate) async fn commit_phase(
        &mut self,
        _verification: VerificationResult,
    ) -> Result<CommitResult, crate::AgentError> {
        let files: Vec<std::path::PathBuf> = self
            .transaction
            .as_ref()
            .map(|txn| txn.snapshots().iter().map(|s| s.path.clone()).collect())
            .unwrap_or_default();

        let committer = crate::commit::Committer::new();
        let message = format!("forge: apply changes ({} files)", files.len());
        let commit_report = committer
            .finalize(&self.codebase_path, &files, &message)
            .await
            .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;

        if let Some(txn) = self.transaction.take() {
            txn.commit()
                .await
                .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;
        }

        self.audit_log
            .record(AuditEvent::Commit {
                timestamp: Utc::now(),
                transaction_id: commit_report.transaction_id.clone(),
            })
            .await
            .map_err(|e| crate::AgentError::CommitFailed(e.to_string()))?;

        #[cfg(feature = "envoy")]
        if let Some(ref session) = self.session {
            if commit_report.git_committed {
                let sha = std::process::Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(&self.codebase_path)
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                let msg = &message;
                let ct = crate::evidence::CommitType::classify(msg);
                session.record_commit(crate::evidence::CommitRecord {
                    commit_sha: sha,
                    message: msg.clone(),
                    author: "forge".into(),
                    files_changed: commit_report.files_committed.len() as u64,
                    commit_type: ct,
                    feature_tag: crate::evidence::extract_feature_tag(msg),
                    ..Default::default()
                });
            }
        }

        Ok(CommitResult {
            transaction_id: commit_report.transaction_id,
            files_committed: commit_report.files_committed,
            git_committed: commit_report.git_committed,
        })
    }

    pub(crate) async fn verify_policies_on_mutations(
        &self,
    ) -> Result<Vec<String>, crate::AgentError> {
        let Some(ref txn) = self.transaction else {
            return Ok(Vec::new());
        };
        if self.policies.is_empty() {
            return Ok(Vec::new());
        }
        let validator = crate::policy::PolicyValidator::new((*self.forge).clone());
        let mut violations = Vec::new();
        for snapshot in txn.snapshots() {
            let modified = tokio::fs::read_to_string(&snapshot.path)
                .await
                .unwrap_or_default();
            let diff = crate::policy::Diff {
                file_path: snapshot.path.clone(),
                original: snapshot.original_content.clone(),
                modified,
                changes: Vec::new(),
            };
            let report = validator
                .validate(&diff, &self.policies)
                .await
                .map_err(|e| crate::AgentError::PolicyViolation(e.to_string()))?;
            violations.extend(report.violations.iter().map(|v| v.message.clone()));
        }
        Ok(violations)
    }

    pub(super) async fn store_hypothesis_outcome(&self, query: &str, succeeded: bool) {
        let Some(ref store) = self.discovery_store else {
            return;
        };
        let Some(id) = self.current_hypothesis else {
            return;
        };
        let hyp = self.reasoning.board.get(id).await.ok().flatten();
        let confidence = hyp
            .as_ref()
            .map(|h| h.current_confidence().get())
            .unwrap_or(0.5);
        let status = hyp.map(|h| format!("{:?}", h.status()));
        store
            .store(
                "HypothesisOutcome",
                query,
                serde_json::json!({
                    "hypothesis_id": id.to_string(),
                    "succeeded": succeeded,
                    "final_confidence": confidence,
                    "status": status,
                }),
            )
            .await;
    }

    pub(super) async fn store_observation_discoveries(&self, observation: &Observation) {
        if let Some(ref store) = self.discovery_store {
            for symbol in &observation.symbols {
                let metadata = serde_json::json!({
                    "kind": format!("{:?}", symbol.kind),
                    "file": symbol.location.file_path,
                    "line": symbol.location.line_number,
                });
                store.store("Symbol", &symbol.name, metadata).await;
            }
        }
    }

    pub(super) async fn record_rollback(&mut self, error: &crate::AgentError) {
        if let Some(txn) = self.transaction.take() {
            if let Err(e) = txn.rollback().await {
                tracing::error!("Transaction rollback failed: {e}");
            }
        }

        let phase = error.phase_label();

        if let Err(e) = self
            .audit_log
            .record(AuditEvent::Rollback {
                timestamp: Utc::now(),
                reason: error.to_string(),
                phase: phase.to_string(),
            })
            .await
        {
            tracing::error!("Failed to record rollback audit event: {e}");
        }
    }

    #[cfg(test)]
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }
}
