//! Command-line interface for ForgeKit agent.
//!
//! This module provides a CLI for interacting with the agent system,
//! including running full agent loops, planning mutations, and managing checkpoints.
//!
//! # Examples
//!
//! Run the full agent loop:
//!
//! ```bash
//! $ forge-agent run "Add authentication to all API endpoints"
//! ```
//!
//! Plan only (dry run):
//!
//! ```bash
//! $ forge-agent plan "Add feature flag system"
//! ```
//!
//! Show agent status:
//!
//! ```bash
//! $ forge-agent status
//! ```

#![allow(clippy::too_many_arguments)]

use forge_agent::Agent;
use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run().await
}

/// CLI arguments for the agent.
#[derive(Parser, Debug)]
#[command(name = "forge-agent")]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

/// Agent actions available via CLI.
#[derive(Subcommand, Debug)]
enum Action {
    /// Run the full agent loop: observe → constrain → plan → mutate → verify → commit
    Run {
        /// Natural language query describing what to do
        query: String,

        /// Enable runtime integration for coordinated file watching and caching
        #[arg(short, long, default_value = "false")]
        with_runtime: bool,

        /// Show detailed runtime metrics
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// Generate an execution plan without applying changes (dry run)
    Plan {
        /// Natural language query describing what to do
        query: String,
    },

    /// Show current agent status
    Status {
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Agent status display.
#[derive(Clone, Debug)]
pub struct Status {
    /// Whether agent is ready
    pub ready: bool,
    /// Current operation being performed
    pub current_operation: Option<String>,
    /// Number of items in queue
    pub queue_size: usize,
}

impl Status {
    /// Create a default "empty" status
    fn default() -> Self {
        Self {
            ready: false,
            current_operation: None,
            queue_size: 0,
        }
    }

    /// Create status from agent state
    fn from_agent(_agent: &Agent) -> Self {
        Self {
            ready: true,
            current_operation: Some("Idle".to_string()),
            queue_size: 0,
        }
    }
}

/// Entry point for the CLI.
pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.action {
        Action::Run { query, with_runtime, verbose } => {
            // Determine codebase path (default: current directory)
            let codebase_path = std::env::current_dir()?;

            // Run the full agent loop
            println!("🔄 Running agent loop...");

            if with_runtime {
                // Create agent with runtime for coordinated file watching
                let (agent, mut runtime) = Agent::with_runtime(&codebase_path).await?;
                println!("   Runtime integration enabled");

                match agent.run_with_runtime(&mut runtime, &query).await {
                    Ok(_) => {
                        // Display cache status
                        let cache = runtime.cache();
                        let cache_count = cache.map(|c| futures::executor::block_on(c.len())).unwrap_or(0);
                        println!("   Query cache: {} entries cached", cache_count);

                        println!("✅ Agent completed successfully");
                        println!("   Query: {}", query);
                        println!("   Changes applied and committed");

                        // Display runtime statistics
                        let stats = runtime.stats();

                        if verbose {
                            // Detailed verbose output
                            println!("\nRuntime Statistics (detailed):");
                            println!("  Cache size: {} entries", stats.cache_size);
                            println!("  Watch active: {}", if stats.watch_active { "yes" } else { "no" });
                            println!("  Watch directory: {}", runtime.config().watch_dir);
                            println!("  Reindex operations: {}", stats.reindex_count);
                            println!("  ---");
                            println!("  Metrics:");
                            println!("    Graph queries: {}", stats.metrics.graph_queries);
                            println!("    Searches: {}", stats.metrics.searches);
                            println!("    CFG analyses: {}", stats.metrics.cfg_analyses);
                            println!("    Cache hits: {}", runtime.metrics().count(forge_runtime::MetricKind::CacheHit));
                            println!("    Cache misses: {}", runtime.metrics().count(forge_runtime::MetricKind::CacheMiss));
                            println!("    Hit rate: {:.1}%", stats.metrics.cache_hit_rate * 100.0);
                        } else {
                            // Summary statistics (default)
                            println!("\nRuntime Statistics:");
                            println!("  Cache size: {} entries", stats.cache_size);
                            println!("  Watch active: {}", if stats.watch_active { "yes" } else { "no" });
                            println!("  Reindex operations: {}", stats.reindex_count);
                            println!("  Graph queries: {}", stats.metrics.graph_queries);
                            println!("  Cache hit rate: {:.1}%", stats.metrics.cache_hit_rate * 100.0);
                        }

                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Agent failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Create agent instance
                let agent = Agent::new(&codebase_path).await?;
                match agent.run(&query).await {
                    Ok(_) => {
                        println!("✅ Agent completed successfully");
                        println!("   Query: {}", query);
                        println!("   Changes applied and committed");
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Agent failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }

        Action::Plan { query } => {
            // Determine codebase path
            let codebase_path = std::env::current_dir()?;

            // Create agent instance
            let agent = Agent::new(codebase_path).await?;

            // Plan only - show what would be done
            println!("📋 Planning agent operations...");

            // Skip to planning phase
            let obs = agent.observe(&query).await?;
            let constrained = agent.constrain(obs, vec![]).await?;
            let plan = agent.plan(constrained).await?;

            // Display the plan
            println!("   Query: {}", query);
            println!("   Generated {} steps:", plan.steps.len());
            for (i, step) in plan.steps.iter().enumerate() {
                println!("     {}. {}", i + 1, step.description);
            }

            println!("   Estimated impact: {} files, complexity {}",
                plan.estimated_impact.affected_files.len(),
                plan.estimated_impact.complexity);

            Ok(())
        }

        Action::Status { verbose: _ } => {
            // Determine codebase path
            let codebase_path = std::env::current_dir()?;

            // Create agent instance
            let agent = Agent::new(&codebase_path).await?;

            // Get and display status
            let status = Status::from_agent(&agent);

            println!("📊 Agent Status:");
            println!("   Codebase: {}", codebase_path.display());
            println!("   Ready: {}", if status.ready { "✓" } else { "✗" });

            if let Some(op) = status.current_operation {
                println!("   Current: {}", op);
            }

            if status.queue_size > 0 {
                println!("   Queue: {} items", status.queue_size);
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cli_accepts_with_runtime_flag() {
        // Test that CLI parsing accepts --with-runtime flag
        let cli = Cli::try_parse_from(["forge-agent", "run", "test query", "--with-runtime"]);
        assert!(cli.is_ok());

        let cli = cli.unwrap();
        match cli.action {
            Action::Run { with_runtime, verbose, .. } => {
                assert!(with_runtime);
                assert!(!verbose);
            }
            _ => panic!("Expected Run action"),
        }
    }

    #[tokio::test]
    async fn test_cli_accepts_verbose_flag() {
        // Test that CLI parsing accepts --verbose flag
        let cli = Cli::try_parse_from(["forge-agent", "run", "test query", "--verbose"]);
        assert!(cli.is_ok());

        let cli = cli.unwrap();
        match cli.action {
            Action::Run { with_runtime, verbose, .. } => {
                assert!(!with_runtime);
                assert!(verbose);
            }
            _ => panic!("Expected Run action"),
        }
    }

    #[tokio::test]
    async fn test_cli_accepts_both_flags() {
        // Test that CLI parsing accepts both flags together
        let cli = Cli::try_parse_from(["forge-agent", "run", "test query", "--with-runtime", "--verbose"]);
        assert!(cli.is_ok());

        let cli = cli.unwrap();
        match cli.action {
            Action::Run { with_runtime, verbose, .. } => {
                assert!(with_runtime);
                assert!(verbose);
            }
            _ => panic!("Expected Run action"),
        }
    }

    #[tokio::test]
    async fn test_cli_runtime_creation() {
        // Verify runtime can be created successfully
        let temp = tempdir().unwrap();
        let result = Agent::with_runtime(temp.path()).await;
        assert!(result.is_ok());

        let (_agent, runtime) = result.unwrap();
        assert_eq!(runtime.codebase_path(), temp.path());
    }

    #[tokio::test]
    async fn test_cli_runtime_stats() {
        // Verify runtime stats can be retrieved
        let temp = tempdir().unwrap();
        let (_agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        let stats = runtime.stats();
        assert_eq!(stats.cache_size, 0);
        assert!(!stats.watch_active);
        assert_eq!(stats.reindex_count, 0);
    }
}
