# Belief Dependency Graph

**Status**: Design Phase  
**Priority**: P1 - Enables root cause analysis  
**Related**: Contradiction Detector (uses dependency chains), Hypothesis Board (beliefs support hypotheses)

---

## Problem Statement

Beliefs form chains of reasoning:
- "Dequantization is correct" → because → "Python reference matches"
- "Layer 2 weights are bad" → because → "Range is 6.7x after normalization"

When a belief is challenged, you need to know:
1. **What depends on this?** - If this is wrong, what else collapses?
2. **What supports this?** - What evidence/theory underlies this belief?
3. **What's the root cause?** - Trace back to the fundamental assumption

Without explicit dependency tracking:
- **Lone beliefs** - "I think X is true" with no memory of why
- **Cascade failures** - Wrong belief poisons dependent inferences unnoticed
- **No root cause** - Can't trace contradictions to their source

---

## Design Goals

1. **Explicit dependency tracking** - Every belief knows what it depends on
2. **Impact analysis** - When belief X changes, find all affected beliefs
3. **Root cause tracing** - Follow dependency chain to foundation
4. **Circular reasoning detection** - Belief A depends on B depends on A
5. **Confidence propagation** - Update dependent beliefs when foundation shifts

---

## Core Types

```rust
/// A node in the belief dependency graph
#[derive(Clone, Debug)]
pub struct BeliefNode {
    pub belief: Belief,
    
    /// What this belief directly depends on (edges IN)
    pub dependencies: Vec<DependencyEdge>,
    
    /// What depends on this belief (edges OUT)
    pub dependents: Vec<BeliefId>,
    
    /// Depth in dependency tree (0 = foundational)
    pub depth: usize,
    
    /// Stability score - how likely this is to change
    pub stability: StabilityScore,
}

/// Edge representing a dependency relationship
#[derive(Clone, Debug)]
pub struct DependencyEdge {
    pub to: BeliefId,  // Belief that this one depends on
    pub relationship: DependencyType,
    pub strength: f64,  // How much confidence flows (0.0 - 1.0)
    pub reason: String, // Why this dependency exists
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyType {
    /// Logical implication: if A then B
    LogicalImplication,
    
    /// Causal: A causes B
    Causal,
    
    /// Evidential: A is evidence for B
    Evidential,
    
    /// Mathematical derivation: B derived from A
    Derivation,
    
    /// Assumption: B assumes A is true
    Assumption,
    
    /// Measurement: B measured using A
    MeasurementBasis,
}

#[derive(Clone, Copy, Debug)]
pub enum StabilityScore {
    Foundation,  // Axioms, definitions (never changes)
    Stable,      // Well-tested, high confidence
    Tentative,   // New, still being verified
    Speculative, // Hypothesis, may be wrong
}

/// The belief dependency graph
pub struct BeliefGraph {
    nodes: HashMap<BeliefId, BeliefNode>,
    
    /// Index by belief kind for quick queries
    kind_index: HashMap<BeliefKind, Vec<BeliefId>>,
    
    /// Index by depth for topological sorting
    depth_index: BTreeMap<usize, Vec<BeliefId>>,
}

/// Query results
#[derive(Clone, Debug)]
pub struct DependencyChain {
    pub from: BeliefId,
    pub to: BeliefId,
    pub path: Vec<BeliefId>,
    pub total_strength: f64,  // Product of edge strengths
}

#[derive(Clone, Debug)]
pub struct ImpactAnalysis {
    pub target: BeliefId,
    /// All beliefs that would be affected if target changes
    pub affected_beliefs: Vec<AffectedBelief>,
    /// Total number of downstream beliefs
    pub total_impact: usize,
    /// Maximum depth of impact
    pub max_depth: usize,
}

#[derive(Clone, Debug)]
pub struct AffectedBelief {
    pub belief_id: BeliefId,
    pub statement: String,
    /// Path from target to this belief
    pub dependency_path: Vec<BeliefId>,
    /// How much this belief's confidence depends on target
    pub dependency_strength: f64,
}
```

---

## Belief Graph API

```rust
impl BeliefGraph {
    /// Create empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            kind_index: HashMap::new(),
            depth_index: BTreeMap::new(),
        }
    }
    
    /// Add a belief to the graph
    pub fn add_belief(&mut self, belief: Belief) -> Result<()> {
        let node = BeliefNode {
            belief: belief.clone(),
            dependencies: vec![],
            dependents: vec![],
            depth: 0,  // Will be computed
            stability: Self::infer_stability(&belief),
        };
        
        self.nodes.insert(belief.id, node);
        self.kind_index
            .entry(belief.kind)
            .or_default()
            .push(belief.id);
        
        // Recompute depths
        self.compute_depths()?;
        
        Ok(())
    }
    
    /// Add dependency edge between beliefs
    pub fn add_dependency(
        &mut self,
        from: BeliefId,      // Belief that depends
        to: BeliefId,        // Belief being depended on
        relationship: DependencyType,
        strength: f64,
        reason: &str,
    ) -> Result<()> {
        // Create edge
        let edge = DependencyEdge {
            to,
            relationship,
            strength,
            reason: reason.to_string(),
        };
        
        // Add to source's dependencies
        if let Some(node) = self.nodes.get_mut(&from) {
            node.dependencies.push(edge);
        } else {
            return Err(BeliefError::BeliefNotFound(from));
        }
        
        // Add to target's dependents
        if let Some(node) = self.nodes.get_mut(&to) {
            node.dependents.push(from);
        } else {
            return Err(BeliefError::BeliefNotFound(to));
        }
        
        // Check for cycles
        if self.has_cycle() {
            // Remove the edge we just added
            self.remove_dependency(from, to)?;
            return Err(BeliefError::CircularDependency { from, to });
        }
        
        // Recompute depths
        self.compute_depths()?;
        
        Ok(())
    }
    
    /// Find all paths from one belief to another
    pub fn find_paths(&self, from: BeliefId, to: BeliefId) -> Vec<DependencyChain> {
        let mut paths = Vec::new();
        let mut current_path = vec![from];
        
        self.dfs_paths(from, to, &mut current_path, &mut paths, 1.0);
        
        paths
    }
    
    fn dfs_paths(
        &self,
        current: BeliefId,
        target: BeliefId,
        path: &mut Vec<BeliefId>,
        paths: &mut Vec<DependencyChain>,
        strength: f64,
    ) {
        if current == target {
            paths.push(DependencyChain {
                from: path[0],
                to: target,
                path: path.clone(),
                total_strength: strength,
            });
            return;
        }
        
        if let Some(node) = self.nodes.get(&current) {
            for edge in &node.dependencies {
                if !path.contains(&edge.to) {  // Avoid cycles
                    path.push(edge.to);
                    self.dfs_paths(
                        edge.to,
                        target,
                        path,
                        paths,
                        strength * edge.strength,
                    );
                    path.pop();
                }
            }
        }
    }
    
    /// Analyze impact of a belief changing
    pub fn analyze_impact(&self, target: BeliefId) -> Result<ImpactAnalysis> {
        let mut affected = Vec::new();
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(BeliefId, Vec<BeliefId>, f64)> = VecDeque::new();
        
        // Start with direct dependents
        if let Some(node) = self.nodes.get(&target) {
            for &dependent_id in &node.dependents {
                queue.push_back((dependent_id, vec![target, dependent_id], 1.0));
            }
        }
        
        let mut max_depth = 0;
        
        while let Some((current_id, path, cumulative_strength)) = queue.pop_front() {
            if !visited.insert(current_id) {
                continue;
            }
            
            if let Some(node) = self.nodes.get(&current_id) {
                // Find the edge strength from predecessor
                let edge_strength = if path.len() >= 2 {
                    let prev_id = path[path.len() - 2];
                    if let Some(prev_node) = self.nodes.get(&prev_id) {
                        prev_node.dependencies.iter()
                            .find(|e| e.to == current_id)
                            .map(|e| e.strength)
                            .unwrap_or(1.0)
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                
                let new_strength = cumulative_strength * edge_strength;
                
                affected.push(AffectedBelief {
                    belief_id: current_id,
                    statement: node.belief.statement.clone(),
                    dependency_path: path.clone(),
                    dependency_strength: new_strength,
                });
                
                max_depth = max_depth.max(path.len() - 1);
                
                // Continue to dependents
                for &dependent_id in &node.dependents {
                    let mut new_path = path.clone();
                    new_path.push(dependent_id);
                    queue.push_back((dependent_id, new_path, new_strength));
                }
            }
        }
        
        // Sort by dependency strength (strongest first)
        affected.sort_by(|a, b| b.dependency_strength.partial_cmp(&a.dependency_strength).unwrap());
        
        Ok(ImpactAnalysis {
            target,
            affected_beliefs: affected,
            total_impact: affected.len(),
            max_depth,
        })
    }
    
    /// Get foundational beliefs (depth = 0)
    pub fn foundations(&self) -> Vec<&Belief> {
        self.depth_index
            .get(&0)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes.get(id).map(|n| &n.belief))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Get beliefs at a specific depth
    pub fn at_depth(&self, depth: usize) -> Vec<&Belief> {
        self.depth_index
            .get(&depth)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes.get(id).map(|n| &n.belief))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Topological sort of beliefs (foundations first)
    pub fn topological_sort(&self) -> Vec<BeliefId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        
        // Process by depth (ensures dependencies come first)
        for (_, ids) in &self.depth_index {
            for &id in ids {
                if visited.insert(id) {
                    result.push(id);
                }
            }
        }
        
        result
    }
    
    /// Propagate confidence updates through the graph
    pub fn propagate_confidence(&mut self, changed_belief: BeliefId) -> Result<Vec<BeliefId>> {
        let mut updated = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(changed_belief);
        
        while let Some(current_id) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current_id).cloned() {
                // Calculate new confidence based on dependencies
                let new_confidence = self.calculate_derived_confidence(&node)?;
                
                // If confidence changed significantly, update and propagate
                let old_confidence = node.belief.confidence;
                if (new_confidence - old_confidence).abs() > 0.05 {
                    if let Some(node) = self.nodes.get_mut(&current_id) {
                        node.belief.confidence = new_confidence;
                        updated.push(current_id);
                        
                        // Propagate to dependents
                        for &dependent_id in &node.dependents {
                            queue.push_back(dependent_id);
                        }
                    }
                }
            }
        }
        
        Ok(updated)
    }
    
    /// Calculate confidence for a belief based on its dependencies
    fn calculate_derived_confidence(&self, node: &BeliefNode) -> Result<f64> {
        if node.dependencies.is_empty() {
            // Foundational belief - keep its confidence
            return Ok(node.belief.confidence);
        }
        
        // Combine confidences from dependencies
        // Using a weighted geometric mean
        let mut product = 1.0f64;
        let mut total_weight = 0.0f64;
        
        for edge in &node.dependencies {
            if let Some(dep_node) = self.nodes.get(&edge.to) {
                let weight = edge.strength;
                product *= dep_node.belief.confidence.powf(weight);
                total_weight += weight;
            }
        }
        
        if total_weight > 0.0 {
            Ok(product.powf(1.0 / total_weight))
        } else {
            Ok(node.belief.confidence)
        }
    }
    
    /// Check for cycles in the graph
    fn has_cycle(&self) -> bool {
        // Use DFS-based cycle detection
        let mut white: HashSet<BeliefId> = self.nodes.keys().cloned().collect();
        let mut gray: HashSet<BeliefId> = HashSet::new();
        let mut black: HashSet<BeliefId> = HashSet::new();
        
        fn dfs(
            graph: &BeliefGraph,
            node_id: BeliefId,
            white: &mut HashSet<BeliefId>,
            gray: &mut HashSet<BeliefId>,
            black: &mut HashSet<BeliefId>,
        ) -> bool {
            white.remove(&node_id);
            gray.insert(node_id);
            
            if let Some(node) = graph.nodes.get(&node_id) {
                for edge in &node.dependencies {
                    if black.contains(&edge.to) {
                        continue;
                    }
                    if gray.contains(&edge.to) {
                        return true;  // Cycle!
                    }
                    if dfs(graph, edge.to, white, gray, black) {
                        return true;
                    }
                }
            }
            
            gray.remove(&node_id);
            black.insert(node_id);
            false
        }
        
        while let Some(&start) = white.iter().next() {
            if dfs(self, start, &mut white, &mut gray, &mut black) {
                return true;
            }
        }
        
        false
    }
    
    /// Compute depth for all nodes
    fn compute_depths(&mut self) -> Result<()> {
        // Reset depths
        for node in self.nodes.values_mut() {
            node.depth = usize::MAX;
        }
        self.depth_index.clear();
        
        // Topological sort to compute depths
        let sorted = self.topological_sort();
        
        for id in sorted {
            let depth = if let Some(node) = self.nodes.get(&id) {
                if node.dependencies.is_empty() {
                    0
                } else {
                    node.dependencies.iter()
                        .filter_map(|edge| self.nodes.get(&edge.to))
                        .map(|dep| dep.depth + 1)
                        .max()
                        .unwrap_or(0)
                }
            } else {
                0
            };
            
            if let Some(node) = self.nodes.get_mut(&id) {
                node.depth = depth;
            }
            
            self.depth_index.entry(depth).or_default().push(id);
        }
        
        Ok(())
    }
    
    fn infer_stability(belief: &Belief) -> StabilityScore {
        match belief.kind {
            BeliefKind::Observation if belief.confidence > 0.95 => StabilityScore::Stable,
            BeliefKind::Assumption => StabilityScore::Speculative,
            BeliefKind::Inference if belief.confidence > 0.8 => StabilityScore::Tentative,
            _ => StabilityScore::Speculative,
        }
    }
}
```

---

## Visualization

```rust
impl BeliefGraph {
    /// Generate Graphviz DOT format for visualization
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph BeliefGraph {\n");
        dot.push_str("  rankdir=TB;\n");
        dot.push_str("  node [shape=box, style=rounded];\n\n");
        
        // Nodes grouped by depth
        for (depth, ids) in &self.depth_index {
            dot.push_str(&format!("  subgraph depth_{} {{\n", depth));
            dot.push_str(&format!("    label=\"Depth {}\";\n", depth));
            
            for id in ids {
                if let Some(node) = self.nodes.get(id) {
                    let color = match node.stability {
                        StabilityScore::Foundation => "black",
                        StabilityScore::Stable => "green",
                        StabilityScore::Tentative => "orange",
                        StabilityScore::Speculative => "red",
                    };
                    
                    let label = format!("{}\\n({:.0}%)", 
                        Self::truncate(&node.belief.statement, 30),
                        node.belief.confidence * 100.0
                    );
                    
                    dot.push_str(&format!(
                        "    \"{}\" [label=\"{}\", color={}];\n",
                        id.0, label, color
                    ));
                }
            }
            
            dot.push_str("  }\n\n");
        }
        
        // Edges
        for (id, node) in &self.nodes {
            for edge in &node.dependencies {
                let style = match edge.relationship {
                    DependencyType::LogicalImplication => "solid",
                    DependencyType::Causal => "dashed",
                    DependencyType::Evidential => "dotted",
                    _ => "solid",
                };
                
                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\" [style={}, label=\"{:.0}%\"];\n",
                    id.0, edge.to.0, style, edge.strength * 100.0
                ));
            }
        }
        
        dot.push_str("}\n");
        dot
    }
    
    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len])
        }
    }
}
```

---

## Real-World Example (ROCmForge Debugging)

```rust
let mut graph = BeliefGraph::new();

// Foundational observations
let b1 = graph.add_belief(Belief {
    id: id1,
    statement: "Python Q4_0 dequant produces values X".to_string(),
    kind: BeliefKind::Observation,
    confidence: 0.99,
    source: BeliefSource::DirectMeasurement { tool: "python".to_string(), raw_value: "...".to_string() },
    timestamp: Utc::now(),
    dependencies: vec![],
    dependents: vec![],
})?;

let b2 = graph.add_belief(Belief {
    id: id2,
    statement: "Rust Q4_0 dequant produces values Y".to_string(),
    kind: BeliefKind::Observation,
    confidence: 0.95,
    source: BeliefSource::DirectMeasurement { tool: "rocmforge".to_string(), raw_value: "...".to_string() },
    timestamp: Utc::now(),
    dependencies: vec![],
    dependents: vec![],
})?;

// Inferences
let b3 = graph.add_belief(Belief {
    id: id3,
    statement: "Q4_0 dequantization is correct".to_string(),
    kind: BeliefKind::Inference,
    confidence: 0.0,  // Will be derived
    source: BeliefSource::LogicalDeduction { from: vec![id1, id2], rule: "X == Y".to_string() },
    timestamp: Utc::now(),
    dependencies: vec![],
    dependents: vec![],
})?;

// Add dependency: b3 depends on b1 and b2
graph.add_dependency(id3, id1, DependencyType::Evidential, 0.9, "Python is reference")?;
graph.add_dependency(id3, id2, DependencyType::Evidential, 0.9, "Rust matches Python")?;

// Propagate confidence
graph.propagate_confidence(id3)?;

// Later, if we find that b2 was wrong...
// Analyze impact
let impact = graph.analyze_impact(id2);
println!("If Rust dequant is wrong, {} beliefs are affected:", impact.total_impact);
for aff in &impact.affected_beliefs {
    println!("  - {} ({}% dependency)", aff.statement, aff.dependency_strength * 100.0);
}

// Find root cause of a failed belief
let paths = graph.find_paths(id2, id1);  // From Rust observation to Python observation
if paths.is_empty() {
    println!("No path - these are independent observations");
}

// Visualize
std::fs::write("belief_graph.dot", graph.to_dot())?;
```

---

## CLI Integration

```bash
# Show belief dependencies
forge belief deps <belief-id>

# Show what depends on a belief
forge belief impact <belief-id>

# Find path between two beliefs
forge belief path <from-id> <to-id>

# Export graph visualization
forge belief visualize --format dot > graph.dot
forge belief visualize --format png > graph.png

# Propagate confidence updates
forge belief propagate <belief-id>

# Show foundational beliefs
forge belief foundations
```

---

## Success Metrics

- [ ] Can trace any belief back to its foundational evidence
- [ ] Impact analysis completes in < 100ms for 1000 belief graphs
- [ ] Zero circular dependencies in production debugging sessions
- [ ] Confidence propagation correctly predicts belief stability
- [ ] Visualization helps identify weak points in reasoning
