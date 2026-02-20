//! Cyclomatic complexity and other code metrics calculation
//!
//! Analyzes function CFG to calculate complexity metrics.

// Note: This module provides complexity analysis without needing Result types
use crate::cfg::TestCfg;
use crate::types::BlockId;

/// Complexity metrics for a function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComplexityMetrics {
    /// Cyclomatic complexity (McCabe metric)
    /// CC = E - N + 2P where E=edges, N=nodes, P=connected components
    pub cyclomatic_complexity: usize,
    /// Number of decision points (if, while, for, match, etc.)
    pub decision_points: usize,
    /// Maximum nesting depth
    pub max_nesting_depth: usize,
    /// Number of lines of code (approximate)
    pub lines_of_code: usize,
}

impl ComplexityMetrics {
    /// Calculate complexity from a CFG
    pub fn from_cfg(cfg: &TestCfg, lines_of_code: usize) -> Self {
        let edges = count_edges(cfg);
        let nodes = count_nodes(cfg);
        let connected_components = 1; // Assuming single entry point
        
        // Cyclomatic complexity: CC = E - N + 2P
        // For a connected graph with a single entry/exit, this gives the number
        // of linearly independent paths through the code
        let cc = if nodes == 0 {
            1
        } else {
            // Use isize to handle negative values properly
            let e = edges as isize;
            let n = nodes as isize;
            let p = connected_components as isize;
            ((e - n + 2 * p).max(1)) as usize
        };
        
        let decision_points = count_decision_points(cfg);
        let max_depth = calculate_max_depth(cfg);
        
        Self {
            cyclomatic_complexity: cc,
            decision_points,
            max_nesting_depth: max_depth,
            lines_of_code,
        }
    }
    
    /// Returns a risk assessment based on complexity
    pub fn risk_level(&self) -> RiskLevel {
        match self.cyclomatic_complexity {
            1..=10 => RiskLevel::Low,
            11..=20 => RiskLevel::Medium,
            21..=50 => RiskLevel::High,
            _ => RiskLevel::VeryHigh,
        }
    }
    
    /// Check if complexity exceeds threshold
    pub fn is_complex(&self, threshold: usize) -> bool {
        self.cyclomatic_complexity > threshold
    }
}

/// Risk level assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::VeryHigh => "very_high",
        }
    }
}

/// Count total edges in CFG
fn count_edges(cfg: &TestCfg) -> usize {
    cfg.successors.values().map(|v| v.len()).sum()
}

/// Count total nodes (blocks) in CFG
fn count_nodes(cfg: &TestCfg) -> usize {
    let mut nodes: std::collections::HashSet<BlockId> = std::collections::HashSet::new();
    nodes.insert(cfg.entry);
    
    for (from, tos) in &cfg.successors {
        nodes.insert(*from);
        for to in tos {
            nodes.insert(*to);
        }
    }
    
    nodes.len()
}

/// Count decision points (branches) in CFG
/// Each node with more than one outgoing edge is a decision
fn count_decision_points(cfg: &TestCfg) -> usize {
    cfg.successors
        .values()
        .filter(|succs| succs.len() > 1)
        .count()
}

/// Calculate maximum nesting depth using DFS
fn calculate_max_depth(cfg: &TestCfg) -> usize {
    let mut max_depth = 0;
    let mut visited = std::collections::HashSet::new();
    
    fn dfs(
        cfg: &TestCfg,
        node: BlockId,
        depth: usize,
        visited: &mut std::collections::HashSet<BlockId>,
        max_depth: &mut usize,
    ) {
        if visited.contains(&node) {
            return;
        }
        visited.insert(node);
        
        *max_depth = (*max_depth).max(depth);
        
        if let Some(succs) = cfg.successors.get(&node) {
            for succ in succs {
                dfs(cfg, *succ, depth + 1, visited, max_depth);
            }
        }
    }
    
    dfs(cfg, cfg.entry, 1, &mut visited, &mut max_depth);
    max_depth
}

/// Analyze source code to estimate complexity without full CFG
pub fn analyze_source_complexity(source: &str) -> ComplexityMetrics {
    let lines_of_code = source.lines().count();
    
    // Count decision keywords
    let decision_keywords = [
        "if ", "if(", "else if", "match ", "match{",
        "for ", "for(", "while ", "while(", "loop ", "loop{",
        "&&", "||", "?", "unwrap_or", "ok_or",
    ];
    
    let mut decision_points = 0;
    for keyword in &decision_keywords {
        decision_points += source.matches(keyword).count();
    }
    
    // Estimate nesting depth by counting indentation levels
    let max_depth = source
        .lines()
        .filter_map(|line| {
            let indent = line.len() - line.trim_start().len();
            if indent > 0 { Some(indent / 4) } else { Some(0) }
        })
        .max()
        .unwrap_or(0);
    
    // Estimate cyclomatic complexity: decisions + 1
    let cc = decision_points + 1;
    
    ComplexityMetrics {
        cyclomatic_complexity: cc,
        decision_points,
        max_nesting_depth: max_depth,
        lines_of_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_function_complexity() {
        let cfg = TestCfg::new(BlockId(0));
        let metrics = ComplexityMetrics::from_cfg(&cfg, 10);
        
        // For a simple CFG with 1 node and 0 edges:
        // CC = E - N + 2P = 0 - 1 + 2*1 = 1
        assert_eq!(metrics.cyclomatic_complexity, 1);
        assert_eq!(metrics.decision_points, 0);
    }
    
    #[test]
    fn test_risk_levels() {
        let low = ComplexityMetrics {
            cyclomatic_complexity: 5,
            decision_points: 4,
            max_nesting_depth: 2,
            lines_of_code: 20,
        };
        assert_eq!(low.risk_level(), RiskLevel::Low);
        
        let medium = ComplexityMetrics {
            cyclomatic_complexity: 15,
            decision_points: 14,
            max_nesting_depth: 3,
            lines_of_code: 50,
        };
        assert_eq!(medium.risk_level(), RiskLevel::Medium);
        
        let high = ComplexityMetrics {
            cyclomatic_complexity: 30,
            decision_points: 29,
            max_nesting_depth: 5,
            lines_of_code: 100,
        };
        assert_eq!(high.risk_level(), RiskLevel::High);
    }
    
    #[test]
    fn test_analyze_source_complexity() {
        let source = r#"
            fn example(x: i32) -> i32 {
                if x > 0 {
                    if x > 10 {
                        return x * 2;
                    }
                } else if x < 0 {
                    return -x;
                }
                x
            }
        "#;
        
        let metrics = analyze_source_complexity(source);
        assert!(metrics.cyclomatic_complexity >= 3); // if + else if
        assert!(metrics.lines_of_code > 0);
    }
}
