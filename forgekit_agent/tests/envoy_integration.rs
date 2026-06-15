//! Live integration tests against a local envoy + atheneum server.
//!
//! Requires envoy running at localhost:9876.
//! Run with: cargo test --features envoy --test envoy_integration -- --nocapture

#[cfg(feature = "envoy")]
mod envoy_tests {
    use forgekit_agent::envoy::{EnvoyClient, EnvoyConfig};

    fn client() -> EnvoyClient {
        EnvoyClient::new(EnvoyConfig {
            url: "http://localhost:9876".to_string(),
            agent_name: "forge-test-agent".to_string(),
        })
    }

    async fn envoy_available() -> bool {
        client().is_healthy().await
    }

    // ── Health ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_health_check() {
        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable at localhost:9876");
            return;
        }
        assert!(client().is_healthy().await);
    }

    // ── Agent registration ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_register_agent() {
        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable");
            return;
        }
        let c = client();
        let agent_id = c.register().await.expect("registration failed");
        println!("Registered as agent_id: {agent_id}");
        assert!(!agent_id.is_empty());
    }

    // ── Atheneum: round-trip discovery ────────────────────────────────────────

    #[tokio::test]
    async fn test_store_and_query_discovery() {
        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable");
            return;
        }
        let c = client();
        let target = format!("forge-test-symbol-{}", uuid_fragment());

        let id = c
            .store_discovery(
                "Symbol",
                &target,
                serde_json::json!({
                    "file": "src/lib.rs",
                    "line": 42,
                    "signature": "pub fn test_fn()",
                    "complexity": 3
                }),
            )
            .await
            .expect("store_discovery failed");

        println!("Stored discovery id={id} for target={target}");
        assert!(id > 0);

        let discoveries = c
            .query_discoveries(&target)
            .await
            .expect("query_discoveries failed");

        println!("Got {} discoveries for {target}", discoveries.len());
        assert!(
            !discoveries.is_empty(),
            "Should find the discovery we just stored"
        );
    }

    // ── Atheneum: knowledge endpoint ──────────────────────────────────────────

    #[tokio::test]
    async fn test_query_knowledge() {
        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable");
            return;
        }
        let c = client();
        let target = format!("forge-knowledge-{}", uuid_fragment());

        // Store first so we have something to find
        c.store_discovery("Symbol", &target, serde_json::json!({"info": "test"}))
            .await
            .expect("store failed");

        let knowledge = c
            .query_knowledge(&target)
            .await
            .expect("query_knowledge failed");

        println!("Knowledge for {target}: {} entries", knowledge.len());
        // Knowledge may aggregate differently from raw discoveries; just check no error
    }

    // ── Atheneum: handoff round-trip ──────────────────────────────────────────

    #[tokio::test]
    async fn test_handoff_roundtrip() {
        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable");
            return;
        }
        let c = client();

        let id = c
            .store_handoff(
                "forge-test-agent", // hand off to ourselves for test simplicity
                serde_json::json!({
                    "task": "review_add_function",
                    "symbols": ["add", "subtract"],
                    "context": "arithmetic module"
                }),
            )
            .await
            .expect("store_handoff failed");

        println!("Stored handoff id={id}");
        assert!(id > 0);
    }

    // ── KnowledgeSource trait impl ────────────────────────────────────────────

    #[tokio::test]
    async fn test_knowledge_source_trait() {
        use forgekit_agent::observe::KnowledgeSource;

        if !envoy_available().await {
            eprintln!("SKIP: envoy not reachable");
            return;
        }
        let c = client();

        // Unknown target returns None (not empty-vec — trait contract)
        let result = c.query("totally-nonexistent-symbol-xyz123").await;
        println!("Unknown target result: {result:?}");
        assert!(
            result.is_none(),
            "Unknown target should return None from KnowledgeSource"
        );
    }

    // ── EnvoyConfig from_file ─────────────────────────────────────────────────

    #[test]
    fn test_config_from_missing_file() {
        use forgekit_agent::envoy::EnvoyConfig;
        let result = EnvoyConfig::from_file(std::path::Path::new("/nonexistent/.forge.toml"))
            .expect("io error");
        assert!(result.is_none());
    }

    #[test]
    fn test_config_from_toml() {
        use forgekit_agent::envoy::EnvoyConfig;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".forge.toml");
        std::fs::write(
            &path,
            r#"
[envoy]
url = "http://localhost:9876"
agent_name = "my-forge"
"#,
        )
        .unwrap();

        let config = EnvoyConfig::from_file(&path)
            .expect("io error")
            .expect("should parse envoy section");

        assert_eq!(config.url, "http://localhost:9876");
        assert_eq!(config.agent_name, "my-forge");
    }

    fn uuid_fragment() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        format!("{t:08x}")
    }
}

#[cfg(not(feature = "envoy"))]
#[test]
fn envoy_feature_not_enabled() {
    eprintln!("envoy feature not enabled; skipping envoy tests");
}
