---
phase: 03-tool-system
plan: 01
subsystem: api
tags: [async-trait, tool-calling, registry, trait-object, dyn-dispatch]

# Dependency graph
requires:
  - phase: 01-core-abstractions
    provides: "Error enum, Result type alias, ToolDefinition struct, async_trait pattern"
provides:
  - "Tool trait for defining LLM-callable tools"
  - "ToolRegistry struct for collecting and looking up tools"
  - "DuplicateTool error variant for duplicate registration"
affects: [03-tool-system, 04-workflow-engine, 05-agent-loop]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Tool trait mirrors Model trait (async_trait + Send + Sync)", "Registry pattern with Box<dyn Trait> ownership"]

key-files:
  created: [src/tool.rs]
  modified: [src/error.rs, src/lib.rs]

key-decisions:
  - "Separate methods (name, description, parameters) rather than metadata struct for readability"
  - "Default definition() method assembles ToolDefinition from individual methods"
  - "ToolRegistry owns tools via Box<dyn Tool> for simple lifetime story"
  - "register() returns Error::DuplicateTool instead of silently overwriting"
  - "Added Default impl for ToolRegistry (Rust convention)"

patterns-established:
  - "Tool trait: async_trait + Send + Sync, matching Model trait pattern"
  - "Registry pattern: HashMap<String, Box<dyn Trait>> with error-on-duplicate"
  - "Bridge method: definitions() converts owned traits to wire-format types"

# Metrics
duration: 1min
completed: 2026-02-10
---

# Phase 3 Plan 1: Tool Trait and Registry Summary

**Tool trait with async execute via async_trait and ToolRegistry with HashMap-based lookup, duplicate rejection, and ToolDefinition export**

## Performance

- **Duration:** 1 min
- **Started:** 2026-02-10T22:28:52Z
- **Completed:** 2026-02-10T22:30:18Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- DuplicateTool(String) error variant added to Error enum (now 10 variants)
- Tool trait with 5 methods: name, description, parameters, execute (async), definition (default impl)
- ToolRegistry with new, register, get, definitions methods and Default impl
- Module declared and re-exported in lib.rs for ergonomic imports

## Task Commits

Each task was committed atomically:

1. **Task 1: Add DuplicateTool error variant** - `86bcc45` (feat)
2. **Task 2: Create Tool trait and ToolRegistry struct** - `5e9202c` (feat)

## Files Created/Modified
- `src/tool.rs` - Tool trait and ToolRegistry struct with comprehensive doc comments
- `src/error.rs` - Added DuplicateTool(String) variant in Tool errors section
- `src/lib.rs` - Added pub mod tool, re-exports for Tool and ToolRegistry, updated module docs

## Decisions Made
- Separate methods (name, description, parameters) rather than metadata struct -- reads naturally as educational code
- Default definition() method assembles ToolDefinition from the individual methods -- override only if custom behavior needed
- ToolRegistry owns tools via Box<dyn Tool> -- simple lifetime story, tools live as long as registry
- register() returns Error::DuplicateTool instead of silently overwriting -- catches misconfiguration early
- Added Default impl for ToolRegistry -- Rust convention for types with new()

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tool trait and ToolRegistry ready for Plan 03-02 (tool dispatch logic)
- ToolRegistry::get() returns &dyn Tool for dispatch to call execute()
- ToolRegistry::definitions() returns Vec<ToolDefinition> for Model::chat_with_tools()

## Self-Check: PASSED

All files exist, all commits verified, all content checks passed.

---
*Phase: 03-tool-system*
*Completed: 2026-02-10*
