//! Module dependency analysis
//!
//! Analyzes imports and dependencies between modules.

use crate::error::{ForgeError, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Module dependency analyzer
pub struct ModuleAnalyzer<'a> {
    db_path: &'a Path,
}

impl<'a> ModuleAnalyzer<'a> {
    /// Create a new module analyzer
    pub fn new(db_path: &'a Path) -> Self {
        Self { db_path }
    }

    /// Analyze module dependencies from the graph database
    pub fn analyze_dependencies(&self) -> Result<ModuleDependencyGraph> {
        use sqlitegraph::{open_graph, GraphConfig, snapshot::SnapshotId};
        
        let config = GraphConfig::sqlite();
        let backend = open_graph(self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;
        
        let snapshot = SnapshotId::current();
        let mut modules: HashMap<String, ModuleInfo> = HashMap::new();
        
        let entity_ids = backend.entity_ids()
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to list entities: {}", e)))?;
        
        // Collect modules
        for id in entity_ids.clone() {
            if let Ok(node) = backend.get_node(snapshot, id) {
                if node.kind == "module" {
                    modules.insert(node.name.clone(), ModuleInfo {
                        id,
                        name: node.name,
                        file_path: node.file_path.clone().unwrap_or_default(),
                        symbols: Vec::new(),
                        imports: HashSet::new(),
                        exports: HashSet::new(),
                    });
                }
            }
        }
        
        // Build cross-file dependencies
        let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();
        
        for id in entity_ids {
            if let Ok(node) = backend.get_node(snapshot, id) {
                let from_file = node.file_path.clone().unwrap_or_default();
                
                if let Ok(outgoing) = backend.fetch_outgoing(id) {
                    for target_id in outgoing {
                        if let Ok(target) = backend.get_node(snapshot, target_id) {
                            let to_file = target.file_path.unwrap_or_default();
                            if from_file != to_file && !from_file.is_empty() && !to_file.is_empty() {
                                dependencies.entry(from_file.clone())
                                    .or_default()
                                    .insert(to_file);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(ModuleDependencyGraph {
            modules,
            dependencies,
        })
    }

    /// Find circular dependencies
    pub fn find_cycles(&self) -> Result<Vec<Vec<String>>> {
        let graph = self.analyze_dependencies()?;
        
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut path_set = HashSet::new();
        
        fn dfs(
            node: &str,
            dependencies: &HashMap<String, HashSet<String>>,
            visited: &mut HashSet<String>,
            path: &mut Vec<String>,
            path_set: &mut HashSet<String>,
            cycles: &mut Vec<Vec<String>>,
        ) {
            if path_set.contains(node) {
                if let Some(start) = path.iter().position(|x| x == node) {
                    let cycle: Vec<String> = path[start..].to_vec();
                    cycles.push(cycle);
                }
                return;
            }
            
            if visited.contains(node) {
                return;
            }
            
            visited.insert(node.to_string());
            path.push(node.to_string());
            path_set.insert(node.to_string());
            
            if let Some(deps) = dependencies.get(node) {
                for dep in deps {
                    dfs(dep, dependencies, visited, path, path_set, cycles);
                }
            }
            
            path.pop();
            path_set.remove(node);
        }
        
        for file in graph.dependencies.keys() {
            if !visited.contains(file) {
                dfs(file, &graph.dependencies, &mut visited, &mut path, &mut path_set, &mut cycles);
            }
        }
        
        Ok(cycles)
    }
}

/// Information about a module
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: i64,
    pub name: String,
    pub file_path: String,
    pub symbols: Vec<String>,
    pub imports: HashSet<String>,
    pub exports: HashSet<String>,
}

/// Module dependency graph
#[derive(Debug)]
pub struct ModuleDependencyGraph {
    pub modules: HashMap<String, ModuleInfo>,
    pub dependencies: HashMap<String, HashSet<String>>,
}

impl ModuleDependencyGraph {
    /// Get all modules that depend on the given module
    pub fn dependents(&self, module_file: &str) -> Vec<&str> {
        self.dependencies
            .iter()
            .filter(|(_, deps)| deps.contains(module_file))
            .map(|(file, _)| file.as_str())
            .collect()
    }
    
    /// Get the dependency depth
    pub fn dependency_depth(&self) -> usize {
        let mut max_depth = 0;
        
        for start in self.dependencies.keys() {
            let mut visited = HashSet::new();
            let mut queue: Vec<(String, usize)> = vec![(start.clone(), 0)];
            
            while let Some((current, depth)) = queue.pop() {
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current.clone());
                max_depth = max_depth.max(depth);
                
                if let Some(deps) = self.dependencies.get(&current) {
                    for dep in deps {
                        queue.push((dep.clone(), depth + 1));
                    }
                }
            }
        }
        
        max_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_analyzer_creation() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        
        let analyzer = ModuleAnalyzer::new(&db_path);
        // Verify creation
        assert!(analyzer.db_path.exists() == false);
    }
}
