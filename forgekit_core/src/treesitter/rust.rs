use crate::cfg::TestCfg;
use crate::error::{ForgeError, Result};
use crate::types::BlockId;

use super::{CfgExtractor, FunctionInfo, SupportedLanguage};

impl CfgExtractor {
    /// Extract CFG from Rust source code
    pub fn extract_rust(source: &str) -> Result<Vec<FunctionInfo>> {
        use tree_sitter::Parser;
        use tree_sitter_rust;

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| {
                ForgeError::DatabaseError(format!("Failed to set Rust language: {:?}", e))
            })?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ForgeError::DatabaseError("Failed to parse Rust code".to_string()))?;

        let root = tree.root_node();
        let mut functions = Vec::new();

        Self::extract_rust_functions(source, &root, &mut functions)?;

        Ok(functions)
    }

    fn extract_rust_functions(
        source: &str,
        node: &tree_sitter::Node,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<()> {
        let kind = node.kind();

        // Look for function and method definitions
        if kind == "function_item" || kind == "method_declaration" {
            if let Some(func) = Self::parse_rust_function(source, node)? {
                functions.push(func);
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_rust_functions(source, &child, functions)?;
        }

        Ok(())
    }

    fn parse_rust_function(source: &str, node: &tree_sitter::Node) -> Result<Option<FunctionInfo>> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        // Find function name - look for identifier after fn keyword
        let mut name = "unknown".to_string();
        let mut found_fn = false;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "fn" {
                found_fn = true;
                continue;
            }
            if found_fn && child.kind() == "identifier" {
                name = Self::node_text(source, &child);
                break;
            }
        }

        // Find function body (block)
        let mut body = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                body = Some(child);
                break;
            }
        }

        let cfg = if let Some(body) = body {
            Self::build_cfg_from_body(source, &body, SupportedLanguage::Rust)?
        } else {
            // Function without body (trait method)
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
