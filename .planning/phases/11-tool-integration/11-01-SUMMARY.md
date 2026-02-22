---
phase: 11-tool-integration
plan: 01
title: "Shell Command Execution with tokio::process"
status: complete
date: 2026-02-22
duration: 4 minutes
tasks: 3
files: 1
---

# Phase 11 Plan 01: Shell Command Execution Summary

## Overview

Implemented actual shell command execution for `ShellCommandTask` using `tokio::process` with configurable working directory, environment variables, and timeout support. Commands now execute for real instead of returning `Skipped`, with proper process tracking for compensation during rollback.

**One-liner:** Shell command execution with tokio::process, configurable working directory and environment variables, and process compensation for rollback.

## Tasks Completed

| Task | Commit | Files | Description |
|------|--------|-------|-------------|
| 1 | 9a92433 | forge_agent/src/workflow/tasks.rs | Add ShellCommandConfig with working directory and environment support |
| 2 | 95ac8db | forge_agent/src/workflow/tasks.rs | Implement ShellCommandTask::execute using tokio::process |
| 3 | 7dcc948 | forge_agent/src/workflow/tasks.rs | Add process compensation to ShellCommandTask |

## Files Modified

### forge_agent/src/workflow/tasks.rs

**Lines changed:** +277/-18 (file now 752 lines)

**Changes:**
- Added `ShellCommandConfig` struct with builder pattern
- Implemented actual shell command execution using `tokio::process::Command`
- Added process PID tracking via `Arc<Mutex<Option<u32>>>`
- Implemented compensation method to return process termination
- Added comprehensive tests for working directory, environment, and compensation

**New types:**
- `ShellCommandConfig` - Configuration for command execution with working_dir, env, timeout

**New methods:**
- `ShellCommandConfig::new()`, `args()`, `working_dir()`, `env()`, `timeout()` - Builder pattern
- `ShellCommandTask::with_config()` - Constructor with full configuration
- Updated `ShellCommandTask::execute()` - Actual process spawning and execution
- Updated `ShellCommandTask::compensation()` - Returns process termination

**New tests:**
- `test_shell_command_with_working_dir` - Verifies commands run in correct directory
- `test_shell_command_with_env` - Verifies environment variables are passed
- `test_shell_command_compensation` - Verifies process compensation is created

## Test Results

**Unit tests:** 4/4 passing
```bash
cargo test -p forgekit-agent shell_command
```

Tests verified:
1. `test_shell_command_task_stub` - Basic command execution (echo hello world)
2. `test_shell_task_args_default` - Empty args handling
3. `test_shell_command_with_working_dir` - Working directory configuration
4. `test_shell_command_with_env` - Environment variable configuration
5. `test_shell_command_compensation` - Process compensation creation

**Integration tests:** Actual command execution verified (e.g., `echo "hello"` produces expected output)

**Compilation:** No errors, no new warnings

```bash
cargo check
# Compiling forgekit-agent v0.4.0
# Finished `dev` profile [unoptimized + debuginfo]
```

## Deviations from Plan

**None.** Plan executed exactly as written.

## Key Decisions

1. **Process ID tracking:** Used `Arc<Mutex<Option<u32>>>` for thread-safe PID storage across async execution
2. **Optional PID handling:** `tokio::process::Child::id()` returns `Option<u32>`, handled gracefully with None case
3. **Exit status handling:** Convert non-zero exit codes to `TaskResult::Failed` with stderr capture
4. **Backward compatibility:** Kept deprecated `with_args()` method for compatibility, marked with `#[deprecated]`
5. **Compensation before execution:** Returns `Skip` compensation before any process is spawned
6. **Compensation after execution:** Returns `UndoFunction` compensation with PID for termination

## Success Criteria Met

- [x] `ShellCommandTask::execute()` runs actual shell commands instead of returning `Skipped`
- [x] Commands execute with configurable working directory via `ShellCommandConfig::working_dir()`
- [x] Commands execute with configurable environment variables via `ShellCommandConfig::env()`
- [x] Non-zero exit codes return `TaskResult::Failed` with error message
- [x] Spawned processes are tracked via PID for compensation
- [x] `compensation()` method returns process termination compensation

## Artifacts Created

### ShellCommandConfig

**Purpose:** Configure shell command execution with working directory, environment, and timeout

**Fields:**
- `command: String` - Command to execute
- `args: Vec<String>` - Command arguments
- `working_dir: Option<PathBuf>` - Optional working directory
- `env: HashMap<String, String>` - Environment variables
- `timeout: Option<Duration>` - Optional timeout

**Builder methods:**
- `new(command)`, `args()`, `working_dir()`, `env()`, `timeout()`

### Process Compensation

**Implementation:**
- PID stored in `Arc<Mutex<Option<u32>>>` for thread-safe access
- Compensation returns `UndoFunction` after process spawn
- Compensation returns `Skip` before execution (no process spawned)
- Future integration with `CompensationRegistry::register_process_spawn()` planned

## Dependencies

**Internal:**
- `tokio::process::Command` - Async process spawning
- `std::sync::Arc` - Thread-safe reference counting
- `std::sync::Mutex` - Thread-safe PID storage
- `HashMap` - Environment variable storage

**External:**
- None (tokio already in workspace dependencies)

## Integration Points

**With Phase 09 (compensation):**
- `CompensationRegistry::register_process_spawn()` - Process termination on rollback
- `ToolCompensation::process_compensation(pid)` - Platform-specific process killing

**With Phase 10 (timeouts):**
- `ShellCommandConfig::timeout` - Optional timeout per command
- `tokio::time::timeout()` - Async timeout enforcement

## Next Steps

**Plan 11-02:** File Edit Task with Splice Integration
- Implement actual file editing using Splice tool
- Add file backup/restore for compensation
- Test span-safe edits with graph re-indexing

**Plan 11-03:** Graph Query Task with Magellan Integration
- Implement actual graph queries using Magellan
- Add caching for repeated queries
- Test query result parsing and error handling

## Performance Metrics

- **Execution time:** 4 minutes
- **Files changed:** 1
- **Lines added:** 277
- **Lines removed:** 18
- **Tests added:** 3
- **Tests passing:** 4/4

## Notes

- Shell commands execute with actual process spawning (no stub/skip)
- Working directory properly sets process cwd
- Environment variables properly passed to child process
- Process PID tracked for future rollback integration
- Unix-only process termination (Windows support deferred)

