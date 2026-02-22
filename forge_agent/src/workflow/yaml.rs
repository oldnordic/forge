//! YAML workflow definition and parsing.
//!
//! Provides YAML-based workflow definition for simple workflows.
//! Complex workflows with custom task types should use the Rust API.

use crate::workflow::{
    task::{TaskId, TaskResult, TaskError, TaskContext},
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
}
