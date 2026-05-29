use crate::cfg::TestCfg;
use crate::error::{ForgeError, Result};
use crate::types::BlockId;

use super::{CfgExtractor, FunctionInfo, SupportedLanguage};

impl CfgExtractor {
    /// Extract CFG from Java source code
    pub fn extract_java(source: &str) -> Result<Vec<FunctionInfo>> {
        use tree_sitter::Parser;
        use tree_sitter_java;

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to set Java language: {:?}", e))
            })?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ForgeError::DatabaseError("Failed to parse Java code".to_string()))?;

        let root = tree.root_node();
        let mut functions = Vec::new();

        Self::extract_java_functions(source, &root, &mut functions)?;

        Ok(functions)
    }

    fn extract_java_functions(
        source: &str,
        node: &tree_sitter::Node,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<()> {
        let kind = node.kind();

        // Look for method declarations
        if kind == "method_declaration" {
            if let Some(func) = Self::parse_java_function(source, node)? {
                functions.push(func);
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_java_functions(source, &child, functions)?;
        }

        Ok(())
    }

    fn parse_java_function(source: &str, node: &tree_sitter::Node) -> Result<Option<FunctionInfo>> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        // Find method name
        let mut name = "unknown".to_string();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                name = Self::node_text(source, &child);
                break;
            }
        }

        // Find method body (block)
        let mut body = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                body = Some(child);
                break;
            }
        }

        let cfg = if let Some(body) = body {
            Self::build_cfg_from_body(source, &body, SupportedLanguage::Java)?
        } else {
            // Abstract method without body
            TestCfg::new(BlockId(0))
        };

        Ok(Some(FunctionInfo {
            name,
            start_byte,
            end_byte,
            cfg,
        }))
    }
}
