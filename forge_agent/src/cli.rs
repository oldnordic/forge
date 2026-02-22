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

use crate::Agent;
use clap::{Parser, Subcommand};

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
        // TODO: Phase 3.1 - Implement --with-runtime flag to create ForgeRuntime
        // and call agent.run_with_runtime() instead of agent.run()
        #[arg(short, long, default_value = "false")]
        with_runtime: bool,
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
    fn from_agent(agent: &Agent) -> Self {
        let op = agent.current_operation.clone()
            .unwrap_or_else(|| "Idle".to_string());
        Self {
            ready: true,
            current_operation: Some(op),
            queue_size: 0,
        }
    }
}

/// Entry point for the CLI.
pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.action {
        Action::Run { query, with_runtime } => {
            // Determine codebase path (default: current directory)
            let codebase_path = std::env::current_dir()?;

            // Run the full agent loop
            println!("🔄 Running agent loop...");

            // TODO: Phase 3.1 - Implement full runtime integration
            // For now, with_runtime flag is accepted but not fully utilized
            if with_runtime {
                // Create agent with runtime for coordinated file watching
                let (agent, mut runtime) = Agent::with_runtime(&codebase_path).await?;
                println!("   Runtime integration enabled");
                match agent.run_with_runtime(&mut runtime, &query).await {
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
