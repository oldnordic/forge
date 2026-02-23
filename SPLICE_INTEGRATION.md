# Splice Integration Summary

## What's New

Splice tools are now **native kimi-cli tools** - no MCP, no external integrations. They work like `file_read` and `file_edit` - always available, attention-persistent.

## New Tools Added

| Tool | Purpose | Key Feature |
|------|---------|-------------|
| `SplicePatch` | Replace symbol body | LSP-verified, span-safe |
| `SpliceDelete` | Delete symbol | Removes entire definition safely |
| `SpliceRename` | Rename symbol | Updates all references |
| `SpliceUndo` | Rollback operation | Uses backup manifest |
| `SpliceApplyFiles` | Bulk replacement | Pattern matching across files |

## Complete Workflow

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  IndexCodebase  │────▶│   FindSymbols    │────▶│ FindReferences  │
│  (create db)    │     │ (find target)    │     │ (check impact)  │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                          │
┌─────────────────┐     ┌──────────────────┐              │
│   SpliceUndo    │◀────│  SplicePatch     │◀─────────────┘
│  (if needed)    │     │  dry_run=True    │
└─────────────────┘     └──────────────────┘
                                │
                                ▼
                        ┌──────────────────┐
                        │  SplicePatch     │
                        │ dry_run=False    │
                        └──────────────────┘
                                │
                                ▼
                        ┌──────────────────┐
                        │    Checkpoint    │
                        │ (save reasoning) │
                        └──────────────────┘
```

## Usage Examples

### 1. Basic Patch Workflow

```python
# Step 1: Index the codebase
IndexCodebase(project_path="/home/feanor/Projects/myapp", db_name="myapp")

# Step 2: Find the symbol to edit
FindSymbols(query="process_request", db_name="myapp")

# Step 3: Preview the change (ALWAYS do this first!)
SplicePatch(
    file_path="src/handlers.rs",
    symbol_name="process_request",
    new_content="pub fn process_request(req: Request) -> Response { ... }",
    db_path="myapp.db",
    dry_run=True  # <-- Preview only
)

# Step 4: Apply if preview looks good
SplicePatch(
    ...,
    dry_run=False  # <-- Actually apply
)
```

### 2. Safe Rename

```python
# Rename a function across all files
SpliceRename(
    old_name="old_function_name",
    new_name="better_name",
    file_path="src/lib.rs",
    db_path="myapp.db",
    dry_run=True  # Preview first
)
```

### 3. Delete with Safety

```python
# Check references first!
FindReferences(symbol_name="unused_helper", db_name="myapp")

# Safe to delete - no references
SpliceDelete(
    file_path="src/utils.rs",
    symbol_name="unused_helper",
    dry_run=True
)
```

### 4. Undo Mistakes

```python
# Oops, that broke something!
SpliceUndo(manifest_path=".splice/backups/backup_2024_...json")
```

## Key Principles

1. **Always dry_run first** - Preview before applying
2. **Check references** - Use FindReferences before Delete/Rename
3. **Use Checkpoints** - Save reasoning state before big changes
4. **Span-safe** - Byte-accurate edits, no line number drift
5. **LSP-verified** - Validates compilation before applying

## What Makes This Different

| Traditional Editing | Splice |
|---------------------|--------|
| Line-based | Byte-span based (no drift) |
| Apply then verify | Verify then apply |
| Manual find-replace | Precise symbol references |
| Hard to undo | Backup manifests for rollback |
| Hope it compiles | LSP validation before apply |

## Integration Files Modified

- `/home/feanor/Projects/kimi-cli/src/kimi_cli/tools/forge/splice_tools.py` - New tools
- `/home/feanor/Projects/kimi-cli/src/kimi_cli/tools/forge/__init__.py` - Exports
- `/home/feanor/Projects/kimi-cli/src/kimi_cli/agents/default/agent.yaml` - Tool registration

## Restart Required

The tools are registered but kimi-cli needs a restart to load them. After restart, they're native tools - no cognitive overhead, always available.
