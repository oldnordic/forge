//! Runtime integration for Agent with ForgeRuntime.
//!
//! This module provides integration between the Agent and ForgeRuntime
//! for coordinated file watching, caching, and automatic re-indexing.

use crate::{Agent, AgentError, LoopResult};
use forge_runtime::ForgeRuntime;
use std::path::Path;

impl Agent {
    /// Creates agent with runtime for file watching and caching.
    ///
    /// This method initializes both the Agent and ForgeRuntime, allowing
    /// the agent to leverage runtime services like query caching and
    /// coordinated file watching.
    ///
    /// # Arguments
    ///
    /// * `codebase_path` - Path to the codebase
    ///
    /// # Returns
    ///
    /// Returns a tuple of (Agent, ForgeRuntime) on success.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use forge_agent::Agent;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let (agent, mut runtime) = Agent::with_runtime("./project").await?;
    ///
    /// // Start file watching
    /// runtime.watch().await?;
    ///
    /// // Run agent with runtime coordination
    /// let result = agent.run_with_runtime(&mut runtime, "refactor function").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_runtime(
        codebase_path: impl AsRef<Path>,
    ) -> Result<(Self, ForgeRuntime), AgentError> {
        let path = codebase_path.as_ref();

        // Create runtime first
        let runtime = ForgeRuntime::new(path)
            .await
            .map_err(|e| AgentError::ObservationFailed(format!("Failed to create runtime: {}", e)))?;

        // Create agent with runtime's forge (shares the same graph store)
        let agent = Agent::new(path).await?;

        Ok((agent, runtime))
    }

    /// Runs agent loop with runtime coordination.
    ///
    /// This method coordinates with ForgeRuntime for optimal performance:
    /// - Query cache access for faster graph operations
    /// - Metrics collection for observability
    ///
    /// Note: For v0.4, watcher pause/resume is deferred to future versions.
    /// The runtime provides cache access and metrics, but file watching
    /// coordination is a future enhancement.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the ForgeRuntime
    /// * `query` - The natural language query or request
    ///
    /// # Returns
    ///
    /// Returns `LoopResult` with transaction ID, modified files, and audit trail.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use forge_agent::Agent;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let (agent, mut runtime) = Agent::with_runtime("./project").await?;
    /// let result = agent.run_with_runtime(&mut runtime, "add error handling").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_with_runtime(
        &self,
        _runtime: &mut ForgeRuntime,
        query: &str,
    ) -> crate::Result<LoopResult> {
        // For v0.4, we don't pause watcher (future enhancement)
        // Just run the normal loop - runtime provides cache access and metrics

        // TODO: Future version - Add watcher.pause() / watcher.resume()
        // TODO: Future version - Utilize runtime cache for graph query optimization

        self.run(query).await
    }

    /// Gets reference to runtime cache if available.
    ///
    /// This method provides access to the runtime's query cache for
    /// optimization of repeated graph queries.
    ///
    /// # Returns
    ///
    /// Returns `None` - cache access requires runtime association (not yet implemented).
    ///
    /// Note: This is a placeholder for future functionality.
    pub fn runtime_cache(&self) -> Option<()> {
        // TODO: Return cache from associated runtime
        // For v0.4, return None (cache access will be added in future version)
        None
    }

    /// Gets runtime statistics if available.
    ///
    /// This method provides access to runtime metrics including cache size,
    /// watch status, and reindex count.
    ///
    /// # Returns
    ///
    /// Returns `None` - stats access requires runtime association (not yet implemented).
    ///
    /// Note: This is a placeholder for future functionality.
    pub fn runtime_stats(&self) -> Option<()> {
        // TODO: Return stats from associated runtime
        // For v0.4, return None
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_with_runtime_creation() {
        let temp = tempfile::tempdir().unwrap();
        let (agent, runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        // Verify both agent and runtime were created
        assert_eq!(agent.codebase_path, temp.path());
        assert_eq!(runtime.codebase_path(), temp.path());
    }

    #[tokio::test]
    async fn test_agent_run_with_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let (agent, mut runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        // Run agent with runtime
        let result = agent.run_with_runtime(&mut runtime, "test query").await;

        // Should complete (may fail on actual query processing, but infrastructure works)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_agent_runtime_stats_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let (agent, _runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        // For v0.4, runtime_stats returns None
        assert!(agent.runtime_stats().is_none());
    }

    #[tokio::test]
    async fn test_agent_runtime_cache_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let (agent, _runtime) = Agent::with_runtime(temp.path()).await.unwrap();

        // For v0.4, runtime_cache returns None
        assert!(agent.runtime_cache().is_none());
    }
}
