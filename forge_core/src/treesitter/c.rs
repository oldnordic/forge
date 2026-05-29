use crate::cfg::TestCfg;
use crate::error::{ForgeError, Result};
use crate::types::BlockId;

use super::{CfgExtractor, FunctionInfo, SupportedLanguage};

impl CfgExtractor {
    /// Extract CFG from C source code
    pub fn extract_c(source: &str) -> Result<Vec<FunctionInfo>> {
        use tree_sitter::Parser;
        use tree_sitter_c;

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to set C language: {:?}", e)))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ForgeError::DatabaseError("Failed to parse C code".to_string()))?;

        let root = tree.root_node();
        let mut functions = Vec::new();

        Self::extract_c_functions(source, &root, &mut functions)?;

        Ok(functions)
    }

    fn extract_c_functions(
        source: &str,
        node: &tree_sitter::Node,
        functions: &mut Vec<FunctionInfo>,
    ) -> Result<()> {
        let kind = node.kind();

        // Look for function definitions
        if kind == "function_definition" {
            if let Some(func) = Self::parse_c_function(source, node)? {
                functions.push(func);
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_c_functions(source, &child, functions)?;
        }

        Ok(())
    }

    fn parse_c_function(source: &str, node: &tree_sitter::Node) -> Result<Option<FunctionInfo>> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        // Find function name - look for identifier within function_declarator
        let mut name = "unknown".to_string();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Direct identifier (for simple cases)
            if child.kind() == "identifier" {
                name = Self::node_text(source, &child);
                break;
            }
            // For function declarator, look inside for the identifier
            if child.kind() == "function_declarator" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "identifier" {
                        name = Self::node_text(source, &inner);
                        break;
                    }
                    // Handle pointer declarator
                    if inner.kind() == "pointer_declarator" || inner.kind() == "function_declarator"
                    {
                        let mut ptr_cursor = inner.walk();
                        for ptr_child in inner.children(&mut ptr_cursor) {
                            if ptr_child.kind() == "identifier" {
                                name = Self::node_text(source, &ptr_child);
                                break;
                            }
                        }
                    }
                }
                break;
            }
            // For pointer functions at top level
            if child.kind() == "pointer_declarator" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "function_declarator" {
                        let mut fn_cursor = inner.walk();
                        for fn_child in inner.children(&mut fn_cursor) {
                            if fn_child.kind() == "identifier" {
                                name = Self::node_text(source, &fn_child);
                                break;
                            }
                        }
                    }
                }
                break;
            }
        }

        // Find compound_statement (function body)
        let mut body = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" {
                body = Some(child);
                break;
            }
        }

        let cfg = if let Some(body) = body {
            Self::build_cfg_from_body(source, &body, SupportedLanguage::C)?
        } else {
            // Function declaration without body
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
