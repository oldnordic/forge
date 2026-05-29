use crate::cfg::TestCfg;
use crate::error::Result;
use crate::types::BlockId;

use super::{CfgExtractor, SupportedLanguage};

impl CfgExtractor {
    pub(super) fn build_cfg_from_body(
        source: &str,
        body_node: &tree_sitter::Node,
        lang: SupportedLanguage,
    ) -> Result<TestCfg> {
        let mut cfg = TestCfg::new(BlockId(0));
        let mut block_counter = 1i64;
        let mut block_stack: Vec<BlockId> = vec![BlockId(0)];
        let mut loop_stack: Vec<BlockId> = Vec::new();

        Self::process_cfg_node(
            source,
            body_node,
            &mut cfg,
            &mut block_counter,
            &mut block_stack,
            &mut loop_stack,
            lang,
        )?;

        // Mark last block as exit
        if let Some(last) = block_stack.last() {
            cfg.add_exit(*last);
        }

        Ok(cfg)
    }

    fn process_cfg_node(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        let kind = node.kind();

        match kind {
            // If statement (C, Java, Rust)
            "if_statement" | "if_expression" | "if_let_expression" => {
                Self::process_if_statement(
                    source,
                    node,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
            }

            // Loops (C, Java style)
            "for_statement" | "while_statement" | "do_statement" => {
                Self::process_loop(source, node, cfg, counter, block_stack, loop_stack, lang)?;
            }

            // Rust loops
            "loop_expression" => {
                // Rust infinite loop: loop { ... }
                Self::process_rust_loop(source, node, cfg, counter, block_stack, loop_stack, lang)?;
            }

            "while_expression" | "while_let_expression" => {
                // Rust while and while let
                Self::process_rust_while(
                    source,
                    node,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
            }

            "for_expression" => {
                // Rust for loop: for x in iter { ... }
                Self::process_rust_for(source, node, cfg, counter, block_stack, loop_stack, lang)?;
            }

            // Match expression (Rust)
            "match_expression" | "match_block" => {
                Self::process_rust_match(
                    source,
                    node,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
            }

            // Switch (C)
            "switch_statement" => {
                Self::process_switch(source, node, cfg, counter, block_stack, loop_stack, lang)?;
            }

            // Return statements (all languages)
            "return_statement" | "return_expression" => {
                if let Some(current) = block_stack.last() {
                    cfg.add_exit(*current);
                }
            }

            // Break statement - jump to loop exit
            "break_statement" | "break_expression" => {
                if let Some(loop_header) = loop_stack.last() {
                    if let Some(current) = block_stack.last() {
                        cfg.add_edge(*current, *loop_header);
                    }
                }
            }

            // Continue statement - jump back to loop header
            "continue_statement" => {
                if let Some(loop_header) = loop_stack.last() {
                    if let Some(current) = block_stack.last() {
                        cfg.add_edge(*current, *loop_header);
                    }
                }
            }

            // Compound statements / blocks - process children
            "compound_statement" | "block" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::process_cfg_node(
                        source,
                        &child,
                        cfg,
                        counter,
                        block_stack,
                        loop_stack,
                        lang,
                    )?;
                }
            }

            // Sequential flow - no control flow change
            "expression_statement"
            | "declaration"
            | "local_variable_declaration"
            | "let_declaration"
            | "call_expression" => {
                // These are sequential, no control flow change
            }

            _ => {
                // For other nodes, recurse into children
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::process_cfg_node(
                        source,
                        &child,
                        cfg,
                        counter,
                        block_stack,
                        loop_stack,
                        lang,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn process_if_statement(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        let cond_block = block_stack.last().copied().unwrap_or(BlockId(0));

        // Create then block
        let then_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(cond_block, then_block);

        // Create else block and merge block
        let else_block = BlockId(*counter);
        *counter += 1;
        let merge_block = BlockId(*counter);
        *counter += 1;

        cfg.add_edge(cond_block, else_block);

        // Find then and else branches
        let mut then_body = None;
        let mut else_body = None;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "compound_statement" | "block" | "expression_statement" => {
                    if then_body.is_none() {
                        then_body = Some(child);
                    } else {
                        else_body = Some(child);
                    }
                }
                "if_statement" => {
                    // else-if
                    else_body = Some(child);
                }
                _ => {}
            }
        }

        // Process then branch
        block_stack.push(then_block);
        if let Some(then) = then_body {
            Self::process_cfg_node(source, &then, cfg, counter, block_stack, loop_stack, lang)?;
        }
        if let Some(current) = block_stack.pop() {
            cfg.add_edge(current, merge_block);
        }

        // Process else branch
        block_stack.push(else_block);
        if let Some(else_) = else_body {
            Self::process_cfg_node(source, &else_, cfg, counter, block_stack, loop_stack, lang)?;
        }
        if let Some(current) = block_stack.pop() {
            cfg.add_edge(current, merge_block);
        }

        // Continue with merge block
        block_stack.push(merge_block);

        Ok(())
    }

    fn process_loop(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        let pre_block = block_stack.last().copied().unwrap_or(BlockId(0));

        // Create header block (condition check)
        let header_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(pre_block, header_block);

        // Create body block
        let body_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, body_block);

        // Create exit block
        let exit_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, exit_block);

        // Push loop context
        loop_stack.push(header_block);

        // Find and process body
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" || child.kind() == "block" {
                block_stack.push(body_block);
                Self::process_cfg_node(
                    source,
                    &child,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
                if let Some(current) = block_stack.pop() {
                    // Back edge to header
                    cfg.add_edge(current, header_block);
                }
                break;
            }
        }

        loop_stack.pop();

        // Continue with exit block
        block_stack.push(exit_block);

        Ok(())
    }

    fn process_switch(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        let switch_block = block_stack.last().copied().unwrap_or(BlockId(0));
        let merge_block = BlockId(*counter);
        *counter += 1;

        // Find switch body
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" {
                // Process case statements
                let mut case_cursor = child.walk();
                for case in child.children(&mut case_cursor) {
                    if case.kind() == "case_statement" || case.kind() == "labeled_statement" {
                        let case_block = BlockId(*counter);
                        *counter += 1;
                        cfg.add_edge(switch_block, case_block);

                        block_stack.push(case_block);
                        Self::process_cfg_node(
                            source,
                            &case,
                            cfg,
                            counter,
                            block_stack,
                            loop_stack,
                            lang,
                        )?;
                        if let Some(current) = block_stack.pop() {
                            cfg.add_edge(current, merge_block);
                        }
                    }
                }
            }
        }

        block_stack.push(merge_block);
        Ok(())
    }

    fn process_rust_loop(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        // Rust infinite loop: loop { ... }
        let pre_block = block_stack.last().copied().unwrap_or(BlockId(0));

        // Create header block (loop entry)
        let header_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(pre_block, header_block);

        // Create body block
        let body_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, body_block);

        // Create exit block (for break)
        let exit_block = BlockId(*counter);
        *counter += 1;

        // Push loop context (header is also exit target for break)
        loop_stack.push(header_block);

        // Find and process body (block)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                block_stack.push(body_block);
                Self::process_cfg_node(
                    source,
                    &child,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
                if let Some(current) = block_stack.pop() {
                    // Back edge to header (infinite loop)
                    cfg.add_edge(current, header_block);
                }
                break;
            }
        }

        loop_stack.pop();

        // Continue with exit block
        block_stack.push(exit_block);
        cfg.add_edge(header_block, exit_block);

        Ok(())
    }

    fn process_rust_while(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        // Rust while and while let
        let pre_block = block_stack.last().copied().unwrap_or(BlockId(0));

        // Create header block (condition check)
        let header_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(pre_block, header_block);

        // Create body block
        let body_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, body_block);

        // Create exit block
        let exit_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, exit_block);

        // Push loop context
        loop_stack.push(header_block);

        // Find and process body
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                block_stack.push(body_block);
                Self::process_cfg_node(
                    source,
                    &child,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
                if let Some(current) = block_stack.pop() {
                    // Back edge to header
                    cfg.add_edge(current, header_block);
                }
                break;
            }
        }

        loop_stack.pop();

        // Continue with exit block
        block_stack.push(exit_block);

        Ok(())
    }

    fn process_rust_for(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        // Rust for loop: for x in iter { ... }
        let pre_block = block_stack.last().copied().unwrap_or(BlockId(0));

        // Create header block
        let header_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(pre_block, header_block);

        // Create body block
        let body_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, body_block);

        // Create exit block
        let exit_block = BlockId(*counter);
        *counter += 1;
        cfg.add_edge(header_block, exit_block);

        // Push loop context
        loop_stack.push(header_block);

        // Find and process body
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                block_stack.push(body_block);
                Self::process_cfg_node(
                    source,
                    &child,
                    cfg,
                    counter,
                    block_stack,
                    loop_stack,
                    lang,
                )?;
                if let Some(current) = block_stack.pop() {
                    // Back edge to header
                    cfg.add_edge(current, header_block);
                }
                break;
            }
        }

        loop_stack.pop();

        // Continue with exit block
        block_stack.push(exit_block);

        Ok(())
    }

    fn process_rust_match(
        source: &str,
        node: &tree_sitter::Node,
        cfg: &mut TestCfg,
        counter: &mut i64,
        block_stack: &mut Vec<BlockId>,
        loop_stack: &mut Vec<BlockId>,
        lang: SupportedLanguage,
    ) -> Result<()> {
        // Rust match expression
        let match_block = block_stack.last().copied().unwrap_or(BlockId(0));
        let merge_block = BlockId(*counter);
        *counter += 1;

        // Find match body (block)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                // Process match arms
                let mut arm_cursor = child.walk();
                for arm in child.children(&mut arm_cursor) {
                    if arm.kind() == "match_arm" {
                        let arm_block = BlockId(*counter);
                        *counter += 1;
                        cfg.add_edge(match_block, arm_block);

                        block_stack.push(arm_block);
                        Self::process_cfg_node(
                            source,
                            &arm,
                            cfg,
                            counter,
                            block_stack,
                            loop_stack,
                            lang,
                        )?;
                        if let Some(current) = block_stack.pop() {
                            cfg.add_edge(current, merge_block);
                        }
                    }
                }
            }
        }

        block_stack.push(merge_block);
        Ok(())
    }
}
