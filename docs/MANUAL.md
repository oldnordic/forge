# ForgeKit User Manual

Complete guide to using ForgeKit for code intelligence operations.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Working Examples](#working-examples)
3. [Backends](#backends)
4. [Graph Operations](#graph-operations)
5. [Analysis Operations](#analysis-operations)
6. [Search Operations](#search-operations)
7. [CFG Analysis](#cfg-analysis)
8. [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

Add ForgeKit to your `Cargo.toml`:

```toml
[dependencies]
forge-core = "0.2"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

### Opening a Codebase

```rust
use forge_core::{Forge, BackendKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Option 1: Default backend (SQLite)
    let forge = Forge::open("./my-project").await?;
    
    // Option 2: Specify backend
    let forge = Forge::open_with_backend(
        "./project", 
        BackendKind::NativeV3
    ).await?;
    
    Ok(())
}
```

### Database Location

ForgeKit stores databases in `.forge/` directory:

```
my-project/
├── src/
├── .forge/
│   ├── graph.db      # SQLite backend
│   └── graph.v3      # Native V3 backend
└── Cargo.toml
```

## Working Examples

### Example 1: Find Dead Code

Detect unused functions in your codebase:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // Find all dead code
    let dead_code = forge.analysis().find_dead_code().await?;
    
    if dead_code.is_empty() {
        println!("No dead code found!");
    } else {
        println!("Found {} unused symbols:", dead_code.len());
        for symbol in dead_code {
            println!("  - {} ({:?}) in {:?}", 
                symbol.name,
                symbol.kind,
                symbol.location.file_path
            );
        }
    }
    
    Ok(())
}
```

### Example 2: Impact Analysis

Find what would break if you change a function:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // Find all symbols impacted by changing "process_request"
    let impacted = forge.graph()
        .impact_analysis("process_request", Some(2))  // 2 hops
        .await?;
    
    println!("Changing 'process_request' would affect {} symbols:", 
        impacted.len());
    
    for symbol in impacted {
        println!("  [{} hops] {} ({}) in {}",
            symbol.hop_distance,
            symbol.name,
            symbol.kind,
            symbol.file_path
        );
    }
    
    Ok(())
}
```

### Example 3: Find Callers

Find all functions that call a specific function:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // Find all callers of "database_query"
    let callers = forge.graph().callers_of("database_query").await?;
    
    println!("'database_query' is called by {} functions:", callers.len());
    for caller in callers {
        println!("  - {:?} at line {}", 
            caller.from,
            caller.location.line_number
        );
    }
    
    Ok(())
}
```

### Example 4: Complexity Analysis

Calculate cyclomatic complexity of source code:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    let analysis = forge.analysis();
    
    let source = r#"
        fn calculate(x: i32, y: i32) -> i32 {
            if x > 0 {
                if y > 0 {
                    x + y
                } else {
                    x - y
                }
            } else {
                0
            }
        }
    "#;
    
    let metrics = analysis.analyze_source_complexity(source);
    
    println!("Cyclomatic Complexity: {}", metrics.cyclomatic_complexity);
    println!("Decision Points: {}", metrics.decision_points);
    println!("Max Nesting Depth: {}", metrics.max_nesting_depth);
    println!("Lines of Code: {}", metrics.lines_of_code);
    println!("Risk Level: {:?}", metrics.risk_level());
    
    // Risk levels:
    // - Low (1-10): Simple, easy to test
    // - Medium (11-20): Moderate complexity
    // - High (21-50): Complex, needs refactoring
    // - VeryHigh (>50): Very complex, high risk
    
    Ok(())
}
```

### Example 5: Pattern Search

Search for code patterns using regex:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // Find all test functions
    let tests = forge.search().pattern(r"fn test_.*\(").await?;
    println!("Found {} test functions", tests.len());
    
    // Find all async functions
    let async_fns = forge.search().pattern(r"async fn ").await?;
    println!("Found {} async functions", async_fns.len());
    
    // Find all structs
    let structs = forge.search().symbols_by_kind(
        forge_core::types::SymbolKind::Struct
    ).await?;
    println!("Found {} structs", structs.len());
    
    Ok(())
}
```

### Example 6: Module Dependencies

Analyze dependencies between modules:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // Get all module dependencies
    let deps = forge.analysis().module_dependencies().await?;
    
    println!("Module Dependencies:");
    for dep in deps {
        println!("  {} -> {}", dep.from, dep.to);
    }
    
    // Find circular dependencies
    let cycles = forge.analysis().find_dependency_cycles().await?;
    
    if !cycles.is_empty() {
        println!("\nWarning: Found {} circular dependencies!", cycles.len());
        for cycle in cycles {
            println!("  Cycle: {}", cycle.join(" -> "));
        }
    }
    
    Ok(())
}
```

### Example 7: CFG Analysis

Analyze control flow of functions:

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./project").await?;
    
    // First find a symbol
    let symbols = forge.graph().find_symbol("process_data").await?;
    if let Some(symbol) = symbols.first() {
        let symbol_id = symbol.id;
        
        // Compute dominators
        let doms = forge.cfg().dominators(symbol_id).await?;
        println!("Function has {} dominator relationships", doms.len());
        
        // Find loops
        let loops = forge.cfg().loops(symbol_id).await?;
        println!("Found {} loops", loops.len());
        
        for loop_info in loops {
            println!("  Loop at block {:?} with {} blocks",
                loop_info.header,
                loop_info.len()
            );
        }
    }
    
    Ok(())
}
```

## Backends

### SQLite Backend

Stable, mature backend with full SQL access:

```rust
let forge = Forge::open_with_backend(
    "./project",
    BackendKind::SQLite
).await?;
```

**Pros:**
- Full ACID transactions
- Battle-tested stability
- External tool compatibility

**Cons:**
- Slower than V3
- Requires libsqlite3

### Native V3 Backend

High-performance pure Rust backend:

```rust
let forge = Forge::open_with_backend(
    "./project",
    BackendKind::NativeV3
).await?;
```

**Pros:**
- 10-20x faster traversals
- Pure Rust (no C dependencies)
- Better for large codebases

**Cons:**
- No raw SQL access
- Newer (less battle-tested)

### Backend Selection Guide

| Use Case | Recommended Backend |
|----------|---------------------|
| New projects | Native V3 |
| Large codebases (>1M LOC) | Native V3 |
| Need SQL access | SQLite |
| Maximum stability | SQLite |
| CI/CD pipelines | Native V3 |

## Graph Operations

### Finding Symbols

```rust
// By name (fuzzy search)
let symbols = forge.graph().find_symbol("main").await?;

// By ID
let symbol = forge.graph().find_symbol_by_id(SymbolId(42)).await?;
```

### Finding References

```rust
// All references
let refs = forge.graph().references("MyStruct").await?;

// Only callers (functions that call this)
let callers = forge.graph().callers_of("my_function").await?;
```

### Reachability

```rust
// Find all symbols reachable from a starting point
let reachable = forge.graph().reachable_from(SymbolId(1)).await?;
```

## Analysis Operations

### Impact Analysis

```rust
// Find what would be affected by a change
let impact = forge.analysis().analyze_impact("target_function").await?;
println!("{} call sites affected", impact.call_sites);
```

### Cross References

```rust
// Get both callers and callees
let xrefs = forge.analysis().cross_references("my_function").await?;
println!("{} callers, {} callees", 
    xrefs.callers.len(), 
    xrefs.callees.len()
);
```

## Search Operations

### Pattern Search

```rust
// Regex search
let results = forge.search().pattern(r"fn.*test").await?;
```

### Semantic Search

```rust
// Search by meaning (requires indexing)
let results = forge.search().semantic("error handling").await?;
```

### By Kind

```rust
// Find all functions
let functions = forge.search()
    .symbols_by_kind(SymbolKind::Function)
    .await?;
```

## CFG Analysis

### Building Test CFGs

For unit testing, you can construct CFGs programmatically:

```rust
use forge_core::cfg::TestCfg;
use forge_core::types::BlockId;

// Create a simple chain
let cfg = TestCfg::chain(0, 5);  // 0 -> 1 -> 2 -> 3 -> 4

// Create if-else structure
let cfg = TestCfg::if_else();

// Create a loop
let cfg = TestCfg::simple_loop();

// Analyze
let dominators = cfg.compute_dominators();
let loops = cfg.detect_loops();
let paths = cfg.enumerate_paths();
```

## Troubleshooting

### Database Not Found

```
Error: DatabaseError("Failed to open graph: ...")
```

**Solution:** The `.forge/` directory doesn't exist. Create it or let ForgeKit create it automatically.

### Symbol Not Found

```
Error: SymbolNotFound(SymbolId(42))
```

**Solution:** The symbol ID doesn't exist in the graph. Use `find_symbol()` to search by name first.

### Empty Results

If queries return empty results, the graph may not be indexed:

```rust
// Index the codebase first (requires magellan feature)
forge.graph().index().await?;
```

### Backend Mismatch

```
Error: DatabaseError("Database format mismatch")
```

**Solution:** You're trying to open a V3 database with SQLite backend or vice versa. Use the correct backend for your database file.

---

*For API details, see the [API Reference](API.md).*
