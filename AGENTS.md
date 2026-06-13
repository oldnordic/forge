# Agent Instructions

This file is intentionally small. The canonical workflow is:

`/home/feanor/Projects/AGENTS.md`

Follow that shared standard before making code changes: query with Magellan / llmgrep / Mirage first, edit surgically, then run the standard verification gates and refresh the graph.

Project: `forge`
Scope: the repository root (workspace with 4 crates)

Database paths (magellan registry managed):
- forge-core: `~/.magellan/forge/forge-core.db`
- forge-agent: `~/.magellan/forge/forge-agent.db`
- forge-reasoning: `~/.magellan/forge/forge-reasoning.db`
- forge-runtime: `~/.magellan/forge/forge-runtime.db`

Local notes:

- Preserve existing dirty worktree changes; assume they belong to the user or another active agent.
- Prefer repo-local `.claude/scripts/quality-gate.sh` when present.
- Each crate is registered separately in magellan. Query the correct DB for the crate you're working on.
- `forge_core::storage::default_db_path()` reads `~/.config/magellan/registry.toml` to resolve DB paths.
- Keep `AGENTS.md` / `CLAUDE.md` out of public packages unless the user explicitly asks otherwise.
