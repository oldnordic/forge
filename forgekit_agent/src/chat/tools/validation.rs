use crate::chat::tools::types::ToolDef;
use serde_json::Value;

pub fn validate_tool_arguments(def: &ToolDef, args: &Value) -> Result<(), String> {
    if args.is_null() {
        return Err(format!(
            "arguments for '{}' must be a JSON object, got null",
            def.name
        ));
    }

    if let Some(required) = def.parameters.get("required").and_then(|r| r.as_array()) {
        if let Some(props) = args.as_object() {
            for req in required {
                if let Some(key) = req.as_str() {
                    if !props.contains_key(key) {
                        return Err(format!(
                            "missing required argument '{}' for tool '{}'",
                            key, def.name
                        ));
                    }
                }
            }
        } else {
            return Err(format!(
                "arguments for '{}' must be a JSON object",
                def.name
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_def() -> ToolDef {
        ToolDef::new(
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["path"]
            }),
        )
    }

    #[test]
    fn valid_args_pass() {
        let def = test_def();
        let args = serde_json::json!({"path": "/tmp/test", "count": 5});
        assert!(validate_tool_arguments(&def, &args).is_ok());
    }

    #[test]
    fn missing_required_fails() {
        let def = test_def();
        let args = serde_json::json!({"count": 5});
        let err = validate_tool_arguments(&def, &args).unwrap_err();
        assert!(err.contains("missing required argument 'path'"));
    }

    #[test]
    fn null_args_fails() {
        let def = test_def();
        let args = Value::Null;
        let err = validate_tool_arguments(&def, &args).unwrap_err();
        assert!(err.contains("must be a JSON object"));
    }

    #[test]
    fn no_required_accepts_empty() {
        let def = ToolDef::empty("noop", "Does nothing");
        let args = serde_json::json!({});
        assert!(validate_tool_arguments(&def, &args).is_ok());
    }

    #[test]
    fn extra_args_pass() {
        let def = test_def();
        let args = serde_json::json!({"path": "/tmp", "extra": true});
        assert!(validate_tool_arguments(&def, &args).is_ok());
    }
}
