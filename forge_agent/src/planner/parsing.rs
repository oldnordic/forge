use crate::Result;

use super::types::{PlanIntent, PlanOperation, PlanStep};

pub(crate) fn parse_llm_steps(response: &str) -> Result<Vec<PlanStep>> {
    let trimmed = response.trim();

    let json_str = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .strip_suffix("```")
        .unwrap_or(trimmed)
        .trim();

    let items: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|_| {
        crate::AgentError::PlanningFailed("Failed to parse LLM response as JSON array".to_string())
    })?;

    let mut steps = Vec::new();
    for item in &items {
        match json_value_to_step(item) {
            Some(step) => steps.push(step),
            None => {
                tracing::warn!(
                    "LLM plan: skipping unparseable step: {}",
                    item.to_string().chars().take(200).collect::<String>()
                );
            }
        }
    }

    let skipped = items.len() - steps.len();
    if skipped > 0 {
        tracing::warn!(
            "LLM plan: {skipped} of {} steps failed to parse",
            items.len()
        );
    }

    Ok(steps)
}

pub(crate) fn json_value_to_step(val: &serde_json::Value) -> Option<PlanStep> {
    let obj = val.as_object()?;
    let op = obj.get("operation")?.as_str()?;

    let operation = match op {
        "inspect" => {
            let name = obj.get("symbol_name")?.as_str()?.to_string();
            let id = obj.get("symbol_id").and_then(|v| v.as_u64())?;
            PlanOperation::Inspect {
                symbol_id: forge_core::types::SymbolId(id as i64),
                symbol_name: name,
            }
        }
        "rename" => PlanOperation::Rename {
            old: obj.get("old")?.as_str()?.to_string(),
            new: obj.get("new")?.as_str()?.to_string(),
            file: obj.get("file").and_then(|v| v.as_str()).map(String::from),
        },
        "delete" => PlanOperation::Delete {
            name: obj.get("name")?.as_str()?.to_string(),
            file: obj.get("file").and_then(|v| v.as_str()).map(String::from),
        },
        "create" => PlanOperation::Create {
            path: obj.get("path")?.as_str()?.to_string(),
            content: obj.get("content")?.as_str()?.to_string(),
        },
        "modify" => PlanOperation::Modify {
            file: obj.get("file")?.as_str()?.to_string(),
            start: obj.get("start")?.as_u64()? as usize,
            end: obj.get("end")?.as_u64()? as usize,
            replacement: obj.get("replacement")?.as_str()?.to_string(),
        },
        _ => return None,
    };

    let description = describe_operation(&operation);
    Some(PlanStep {
        description,
        operation,
    })
}

pub(crate) fn describe_operation(op: &PlanOperation) -> String {
    match op {
        PlanOperation::Rename { old, new, .. } => format!("Rename {old} to {new}"),
        PlanOperation::Delete { name, .. } => format!("Delete {name}"),
        PlanOperation::Create { path, .. } => format!("Create {path}"),
        PlanOperation::Inspect { symbol_name, .. } => format!("Inspect {symbol_name}"),
        PlanOperation::Modify {
            file, start, end, ..
        } => {
            format!("Modify {file}:{start}-{end}")
        }
    }
}

pub(crate) fn detect_intent(query: &str) -> PlanIntent {
    if let Some(rest) = query.strip_prefix("rename ") {
        if let Some((_, new)) = rest.split_once(" to ") {
            return PlanIntent::Rename {
                new_name: new.trim().to_string(),
            };
        }
        if let Some((_, new)) = rest.split_once(" -> ") {
            return PlanIntent::Rename {
                new_name: new.trim().to_string(),
            };
        }
    }

    if query.contains("delete ") || query.contains("remove ") {
        return PlanIntent::Delete;
    }

    if query.contains("create ") || query.contains("add ") {
        return PlanIntent::Create {
            content: String::new(),
        };
    }

    PlanIntent::Inspect
}

pub(crate) fn should_precede(a: &PlanOperation, b: &PlanOperation) -> bool {
    match (a, b) {
        (PlanOperation::Inspect { symbol_name, .. }, PlanOperation::Rename { old, .. }) => {
            symbol_name == old
        }
        (PlanOperation::Inspect { symbol_name, .. }, PlanOperation::Delete { name, .. }) => {
            symbol_name == name
        }
        (PlanOperation::Rename { old, .. }, PlanOperation::Delete { name, .. }) => old == name,
        (PlanOperation::Create { path, .. }, PlanOperation::Modify { file, .. }) => path == file,
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileRegion {
    pub file: String,
    pub start: usize,
    pub end: usize,
}
