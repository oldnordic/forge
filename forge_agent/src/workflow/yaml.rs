//! YAML workflow definition and parsing.
//!
//! Provides YAML-based workflow definition for simple workflows.
//! Complex workflows with custom task types should use the Rust API.

use crate::workflow::{
    task::TaskId,
    tasks::{GraphQueryTask, GraphQueryType, AgentLoopTask, ShellCommandTask},
    dag::{Workflow, WorkflowError},
    builder::WorkflowBuilder,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Workflow definition from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlWorkflow {
    /// Workflow name
    pub name: String,
    /// Optional version for future compatibility
    #[serde(default)]
    pub version: Option<String>,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Workflow tasks
    pub tasks: Vec<YamlTask>,
}

/// Task definition from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlTask {
    /// Unique task identifier
    pub id: String,
    /// Human-readable task name
    pub name: String,
    /// Task type
    #[serde(rename = "type")]
    pub task_type: YamlTaskType,
    /// Task dependencies (task IDs)
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Task-specific parameters
    #[serde(default)]
    pub params: YamlTaskParams,
}

/// Task type enumeration for YAML workflows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum YamlTaskType {
    /// Graph query task (find symbols, references, impact)
    GraphQuery,
    /// Agent loop task (AI-driven operations)
    AgentLoop,
    /// Shell command task (stub for Phase 11)
    Shell,
}

/// Task parameters as flexible JSON values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YamlTaskParams {
    /// Parameter map
    #[serde(flatten)]
    pub params: HashMap<String, Value>,
}

/// Errors that can occur during YAML workflow parsing.
#[derive(Error, Debug)]
pub enum YamlWorkflowError {
    /// Invalid YAML schema
    #[error("Invalid workflow schema: {0}")]
    InvalidSchema(String),

    /// Invalid task type
    #[error("Invalid task type: {0}")]
    InvalidTaskType(String),

    /// Missing required parameter
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// Error during workflow conversion
    #[error("Conversion error: {0}")]
    ConversionError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
}

impl TryFrom<YamlWorkflow> for Workflow {
    type Error = YamlWorkflowError;

    fn try_from(yaml_workflow: YamlWorkflow) -> Result<Self, Self::Error> {
        let mut builder = WorkflowBuilder::new();

        // Add all tasks first
        for yaml_task in &yaml_workflow.tasks {
            let task_id = TaskId::new(yaml_task.id.clone());

            match yaml_task.task_type {
                YamlTaskType::GraphQuery => {
                    // Extract required parameters
                    let query_type_str = yaml_task.params.params.get("query_type")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| YamlWorkflowError::MissingParameter("query_type".to_string()))?;

                    let target = yaml_task.params.params.get("target")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| YamlWorkflowError::MissingParameter("target".to_string()))?;

                    // Convert query_type string to enum
                    let query_type = match query_type_str {
                        "find_symbol" => GraphQueryType::FindSymbol,
                        "references" => GraphQueryType::References,
                        "impact" | "impact_analysis" => GraphQueryType::ImpactAnalysis,
                        _ => return Err(YamlWorkflowError::InvalidSchema(format!("Unknown query_type: {}", query_type_str))),
                    };

                    let task = GraphQueryTask::with_id(
                        task_id.clone(),
                        query_type,
                        target,
                    );

                    builder = builder.add_task(Box::new(task));
                }
                YamlTaskType::AgentLoop => {
                    // Extract query parameter
                    let query = yaml_task.params.params.get("query")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| YamlWorkflowError::MissingParameter("query".to_string()))?;

                    let task = AgentLoopTask::new(
                        task_id.clone(),
                        yaml_task.name.clone(),
                        query,
                    );

                    builder = builder.add_task(Box::new(task));
                }
                YamlTaskType::Shell => {
                    // Extract command parameter
                    let command = yaml_task.params.params.get("command")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| YamlWorkflowError::MissingParameter("command".to_string()))?;

                    // Extract args (optional)
                    let args: Vec<String> = yaml_task.params.params.get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    let task = ShellCommandTask::new(
                        task_id.clone(),
                        yaml_task.name.clone(),
                        command,
                    ).with_args(args);

                    builder = builder.add_task(Box::new(task));
                }
            }
        }

        // Add dependencies after all tasks are added
        for yaml_task in &yaml_workflow.tasks {
            let task_id = TaskId::new(yaml_task.id.clone());
            for dep_id in &yaml_task.depends_on {
                builder = builder.dependency(
                    TaskId::new(dep_id.clone()),
                    task_id.clone(),
                );
            }
        }

        // Build and validate workflow
        let workflow = builder.build()
            .map_err(|e| match e {
                WorkflowError::EmptyWorkflow => YamlWorkflowError::InvalidSchema("Workflow has no tasks".to_string()),
                WorkflowError::CycleDetected(msg) => YamlWorkflowError::ConversionError(format!("Cycle detected: {:?}", msg)),
                WorkflowError::TaskNotFound(id) => YamlWorkflowError::ConversionError(format!("Task not found: {}", id)),
                WorkflowError::MissingDependency(id) => YamlWorkflowError::ConversionError(format!("Missing dependency: {}", id)),
                WorkflowError::CheckpointCorrupted(msg) => YamlWorkflowError::ConversionError(format!("Checkpoint corrupted: {}", msg)),
                WorkflowError::CheckpointNotFound(msg) => YamlWorkflowError::ConversionError(format!("Checkpoint not found: {}", msg)),
                WorkflowError::WorkflowChanged(msg) => YamlWorkflowError::ConversionError(format!("Workflow changed: {}", msg)),
            })?;

        Ok(workflow)
    }
}

/// Loads a workflow from a YAML file.
///
/// # Arguments
///
/// * `path` - Path to the YAML file
///
/// # Returns
///
/// - `Ok(Workflow)` - If workflow loaded and converted successfully
/// - `Err(YamlWorkflowError)` - If file cannot be read or YAML is invalid
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::yaml::load_workflow_from_file;
///
/// let workflow = load_workflow_from_file(Path::new("workflow.yaml")).await?;
/// ```
pub async fn load_workflow_from_file(path: &Path) -> Result<Workflow, YamlWorkflowError> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(load_workflow_from_string(&content)?)
}

/// Loads a workflow from a YAML string.
///
/// # Arguments
///
/// * `yaml` - YAML string containing workflow definition
///
/// # Returns
///
/// - `Ok(Workflow)` - If workflow parsed and converted successfully
/// - `Err(YamlWorkflowError)` - If YAML is invalid
///
/// # Example
///
/// ```ignore
/// use forge_agent::workflow::yaml::load_workflow_from_string;
///
/// let yaml = r#"
/// name: "My Workflow"
/// tasks:
///   - id: "task1"
///     name: "Task 1"
///     type: GRAPH_QUERY
///     params:
///       query_type: "find_symbol"
///       target: "my_function"
/// "#;
///
/// let workflow = load_workflow_from_string(yaml)?;
/// ```
pub fn load_workflow_from_string(yaml: &str) -> Result<Workflow, YamlWorkflowError> {
    let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml)?;
    yaml_workflow.try_into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parse_basic() {
        let yaml = r#"
name: "Test Workflow"
tasks:
  - id: "task1"
    name: "First Task"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "my_function"
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.name, "Test Workflow");
        assert_eq!(workflow.tasks.len(), 1);
        assert_eq!(workflow.tasks[0].id, "task1");
        assert_eq!(workflow.tasks[0].task_type, YamlTaskType::GraphQuery);
    }

    #[test]
    fn test_yaml_parse_with_dependencies() {
        let yaml = r#"
name: "Dependent Workflow"
tasks:
  - id: "find"
    name: "Find Symbol"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "process_data"
  - id: "analyze"
    name: "Analyze Impact"
    type: GRAPH_QUERY
    depends_on: ["find"]
    params:
      query_type: "impact"
      target: "process_data"
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.tasks.len(), 2);
        assert_eq!(workflow.tasks[1].depends_on, vec!["find"]);
    }

    #[test]
    fn test_yaml_parse_with_optional_fields() {
        let yaml = r#"
name: "Simple Workflow"
version: "1.0"
description: "A test workflow"
tasks:
  - id: "task1"
    name: "Task 1"
    type: AGENT_LOOP
    params:
      query: "Find all functions"
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.version, Some("1.0".to_string()));
        assert_eq!(workflow.description, Some("A test workflow".to_string()));
    }

    #[test]
    fn test_yaml_parse_empty_depends_on() {
        let yaml = r#"
name: "Simple Workflow"
tasks:
  - id: "task1"
    name: "Task 1"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "test"
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        assert!(workflow.tasks[0].depends_on.is_empty());
    }

    #[test]
    fn test_yaml_parse_invalid_schema() {
        // Missing required 'name' field
        let yaml = r#"
tasks:
  - id: "task1"
    type: GRAPH_QUERY
"#;

        let result: Result<YamlWorkflow, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_graph_query_conversion() {
        let yaml = r#"
name: "Graph Query Test"
tasks:
  - id: "find"
    name: "Find Symbol"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "process_data"
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let workflow: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[test]
    fn test_yaml_to_workflow() {
        let yaml = r#"
name: "Test Workflow"
tasks:
  - id: "find"
    name: "Find Symbol"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "my_function"
  - id: "analyze"
    name: "Analyze Impact"
    type: GRAPH_QUERY
    depends_on: ["find"]
    params:
      query_type: "impact"
      target: "my_function"
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let workflow: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.task_count(), 2);

        // Verify execution order respects dependencies
        let execution_order = workflow.execution_order().unwrap();
        assert_eq!(execution_order[0], TaskId::new("find"));
        assert_eq!(execution_order[1], TaskId::new("analyze"));
    }

    #[test]
    fn test_missing_parameter_error() {
        let yaml = r#"
name: "Missing Parameter Test"
tasks:
  - id: "task1"
    name: "Task 1"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      # Missing 'target' parameter
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let result: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(result.is_err());
        assert!(matches!(result, Err(YamlWorkflowError::MissingParameter(_))));
    }

    #[test]
    fn test_agent_loop_conversion() {
        let yaml = r#"
name: "Agent Loop Test"
tasks:
  - id: "observe"
    name: "Gather Context"
    type: AGENT_LOOP
    params:
      query: "Find all functions that call process_data"
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let workflow: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[test]
    fn test_agent_loop_missing_query() {
        let yaml = r#"
name: "Agent Loop Missing Query"
tasks:
  - id: "task1"
    name: "Task 1"
    type: AGENT_LOOP
    params:
      # Missing 'query' parameter
      other: "value"
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let result: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(result.is_err());
        assert!(matches!(result, Err(YamlWorkflowError::MissingParameter(_))));
    }

    #[test]
    fn test_shell_task_stub() {
        let yaml = r#"
name: "Shell Task Test"
tasks:
  - id: "run"
    name: "Run Command"
    type: SHELL
    params:
      command: "echo"
      args: ["hello", "world"]
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let workflow: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[test]
    fn test_shell_task_args_default() {
        let yaml = r#"
name: "Shell Task No Args"
tasks:
  - id: "run"
    name: "Run Command"
    type: SHELL
    params:
      command: "ls"
"#;

        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();
        let workflow: Result<Workflow, _> = yaml_workflow.try_into();

        assert!(workflow.is_ok());
        let workflow = workflow.unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[tokio::test]
    async fn test_load_from_string() {
        let yaml = r#"
name: "Test Workflow"
tasks:
  - id: "task1"
    name: "Task 1"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "test_function"
"#;

        let workflow = load_workflow_from_string(yaml).unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[tokio::test]
    async fn test_load_from_file() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let yaml = r#"
name: "File Workflow"
tasks:
  - id: "task1"
    name: "Task 1"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "my_function"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", yaml).unwrap();

        let workflow = load_workflow_from_file(temp_file.path()).await.unwrap();
        assert_eq!(workflow.task_count(), 1);
    }

    #[tokio::test]
    async fn test_yaml_round_trip() {
        let yaml = r#"
name: "Round Trip Test"
version: "1.0"
description: "Test serialization round trip"
tasks:
  - id: "task1"
    name: "Task 1"
    type: GRAPH_QUERY
    params:
      query_type: "find_symbol"
      target: "test"
"#;

        // Parse YAML to YamlWorkflow
        let yaml_workflow: YamlWorkflow = serde_yaml::from_str(yaml).unwrap();

        // Serialize back to YAML
        let yaml_out = serde_yaml::to_string(&yaml_workflow).unwrap();

        // Parse again
        let yaml_workflow2: YamlWorkflow = serde_yaml::from_str(&yaml_out).unwrap();

        // Should be identical
        assert_eq!(yaml_workflow.name, yaml_workflow2.name);
        assert_eq!(yaml_workflow.tasks.len(), yaml_workflow2.tasks.len());
    }

    #[tokio::test]
    async fn test_load_simple_graph_query_example() {
        let yaml = include_str!("examples/simple_graph_query.yaml");

        let workflow = load_workflow_from_string(yaml).unwrap();
        assert_eq!(workflow.task_count(), 2);

        let execution_order = workflow.execution_order().unwrap();
        assert_eq!(execution_order[0], TaskId::new("find"));
        assert_eq!(execution_order[1], TaskId::new("analyze"));
    }

    #[tokio::test]
    async fn test_load_agent_assisted_example() {
        let yaml = include_str!("examples/agent_assisted.yaml");

        let workflow = load_workflow_from_string(yaml).unwrap();
        assert_eq!(workflow.task_count(), 2);

        let execution_order = workflow.execution_order().unwrap();
        assert_eq!(execution_order[0], TaskId::new("observe"));
        assert_eq!(execution_order[1], TaskId::new("plan"));
    }

    #[tokio::test]
    async fn test_load_complex_dependencies_example() {
        let yaml = include_str!("examples/complex_dependencies.yaml");

        let workflow = load_workflow_from_string(yaml).unwrap();
        assert_eq!(workflow.task_count(), 4);

        let execution_order = workflow.execution_order().unwrap();
        // Verify topological sort respects diamond pattern
        assert_eq!(execution_order[0], TaskId::new("a"));
        // B and C can be in either order after A
        let b_index = execution_order.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let c_index = execution_order.iter().position(|id| id == &TaskId::new("c")).unwrap();
        assert!(b_index > 0 && c_index > 0);
        // D must be after both B and C
        let d_index = execution_order.iter().position(|id| id == &TaskId::new("d")).unwrap();
        assert!(d_index > b_index && d_index > c_index);
    }
}
