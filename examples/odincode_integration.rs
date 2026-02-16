//! Example: OdinCode-style integration with ForgeKit
//!
//! This example demonstrates how OdinCode would use ForgeKit
//! as a unified SDK instead of individual tool crates.

use forge_core::{Forge, ForgeBuilder};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open a codebase with ForgeKit
    let forge = Forge::open("./my-project").await?;
    
    // Or use the builder for custom configuration
    let forge = ForgeBuilder::new()
        .path("./my-project")
        .cache_ttl(std::time::Duration::from_secs(300))
        .build()
        .await?;

    println!("=== Graph Operations ===");
    
    // Find symbols by name
    let symbols = forge.graph().find_symbol("main").await?;
    println!("Found {} symbols named 'main'", symbols.len());
    
    for symbol in &symbols {
        println!("  - {} ({:?}) at {:?}", 
            symbol.name, 
            symbol.kind,
            symbol.location
        );
    }
    
    // Get symbol details
    if let Some(first) = symbols.first() {
        let details = forge.graph().find_symbol_by_id(first.id.clone()).await?;
        println!("\nDetails: {:?}", details);
        
        // Find references
        let references = forge.graph().find_references(&first.id).await?;
        println!("References: {}", references.len());
    }

    println!("\n=== Search Operations ===");
    
    // Pattern search (regex)
    let results = forge.search().pattern_search(r"async fn.*").await?;
    println!("Found {} async functions", results.len());
    
    // Semantic search (if llmgrep available)
    match forge.search().semantic_search("error handling").await {
        Ok(results) => println!("Semantic search: {} results", results.len()),
        Err(e) => println!("Semantic search not available: {}", e),
    }

    println!("\n=== Edit Operations ===");
    
    // Span-safe patching via splice integration
    let patch_result = forge.edit().patch_symbol(
        "old_function_name",
        "pub fn new_function_name() {}"
    ).await;
    
    match patch_result {
        Ok(result) => println!("Patched successfully: {:?}", result),
        Err(e) => println!("Patch failed (splice not available?): {}", e),
    }
    
    // Multi-step refactoring plan
    let plan = forge.edit().create_plan()
        .rename_symbol("old_name", "new_name")
        .update_references()
        .validate_with_analyzer()
        .build();
    
    match forge.edit().apply_plan(&plan).await {
        Ok(result) => println!("Plan applied: {} files changed", result.changed_files.len()),
        Err(e) => println!("Plan failed: {}", e),
    }

    println!("\n=== CFG Analysis ===");
    
    // Control flow analysis via mirage
    match forge.cfg().analyze_function("process_data").await {
        Ok(cfg) => {
            println!("CFG nodes: {}", cfg.nodes.len());
            println!("CFG edges: {}", cfg.edges.len());
        }
        Err(e) => println!("CFG not available: {}", e),
    }

    println!("\nDone!");
    Ok(())
}

// Example: How OdinCode would structure its tool wrappers
mod odincode_style {
    //! This shows how OdinCode would re-export ForgeKit functionality
    
    use forge_core::{Forge, ForgeError};
    use std::path::Path;
    
    /// OdinCode's unified tool interface
    pub struct OdinTools {
        forge: Forge,
    }
    
    impl OdinTools {
        pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
            let forge = Forge::open(path).await?;
            Ok(Self { forge })
        }
        
        /// File read operation
        pub async fn file_read(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
            // OdinCode might add logging, validation, etc.
            tokio::fs::read_to_string(path).await
                .map_err(|e| anyhow::anyhow!("file read failed: {}", e))
        }
        
        /// Semantic search (using ForgeKit)
        pub async fn semantic_search(&self, query: &str) -> anyhow::Result<Vec<SearchResult>> {
            let symbols = self.forge.search().semantic_search(query).await
                .map_err(|e| anyhow::anyhow!("search failed: {}", e))?;
            
            Ok(symbols.into_iter().map(|s| SearchResult {
                name: s.name,
                file: s.location.file,
                line: s.location.line,
            }).collect())
        }
        
        /// Symbol patching (using ForgeKit -> Splice)
        pub async fn patch_symbol(
            &self,
            symbol: &str,
            replacement: &str
        ) -> anyhow::Result<PatchResult> {
            let result = self.forge.edit().patch_symbol(symbol, replacement).await
                .map_err(|e| anyhow::anyhow!("patch failed: {}", e))?;
            
            Ok(PatchResult {
                success: result.success,
                changed_files: result.changed_files,
            })
        }
        
        /// Graph query (using ForgeKit -> Magellan)
        pub async fn find_symbol(&self, name: &str) -> anyhow::Result<Vec<SymbolInfo>> {
            let symbols = self.forge.graph().find_symbol(name).await
                .map_err(|e| anyhow::anyhow!("graph query failed: {}", e))?;
            
            Ok(symbols.into_iter().map(|s| SymbolInfo {
                name: s.name,
                kind: format!("{:?}", s.kind),
                location: s.location,
            }).collect())
        }
    }
    
    pub struct SearchResult {
        pub name: String,
        pub file: PathBuf,
        pub line: usize,
    }
    
    pub struct PatchResult {
        pub success: bool,
        pub changed_files: Vec<PathBuf>,
    }
    
    pub struct SymbolInfo {
        pub name: String,
        pub kind: String,
        pub location: forge_core::types::Location,
    }
}
