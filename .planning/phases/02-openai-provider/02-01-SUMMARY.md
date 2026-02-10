---
phase: 02-openai-provider
plan: 01
subsystem: api
tags: [openai, serde, reqwest, rustls, tls, wire-format]

# Dependency graph
requires:
  - phase: 01-core-abstractions
    provides: "Message, ModelResponse, ToolCall, ToolDefinition types that wire-format types bridge to"
provides:
  - "reqwest with rustls TLS backend for HTTPS API calls"
  - "All OpenAI Chat Completions API serde types (request, response, shared, error)"
  - "Private openai module in lib.rs for compile verification"
affects: [02-openai-provider]

# Tech tracking
tech-stack:
  added: [rustls (via reqwest feature)]
  patterns: [wire-format types separate from public API types, serde tag-based enum serialization, skip_serializing_if for absent-not-null]

key-files:
  created:
    - src/openai/types.rs
    - src/openai/mod.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/lib.rs

key-decisions:
  - "reqwest 0.13 uses 'rustls' feature (not 'rustls-tls') for TLS backend"
  - "All wire-format types are pub(crate) -- not part of public API"
  - "ChatMessage uses serde tag='role' for internally tagged enum serialization"
  - "Optional request fields use skip_serializing_if for absent-not-null behavior"

patterns-established:
  - "Wire-format types: separate serde types in provider module, not on public types"
  - "Absent-not-null: all Option fields on request types use skip_serializing_if"
  - "Role tagging: ChatMessage enum uses serde(tag='role') for OpenAI's role-based format"

# Metrics
duration: 2min
completed: 2026-02-10
---

# Phase 2 Plan 1: TLS Backend & Wire-Format Types Summary

**Enabled rustls TLS for HTTPS calls and defined all 14 OpenAI Chat Completions API serde types with correct serialization attributes**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-10T21:04:30Z
- **Completed:** 2026-02-10T21:06:31Z
- **Tasks:** 1
- **Files modified:** 5

## Accomplishments
- Enabled rustls TLS backend on reqwest so the library can make HTTPS calls to api.openai.com
- Created all 14 OpenAI wire-format types covering requests, responses, shared tool call structures, and error responses
- All optional request fields serialize as absent (not null) via skip_serializing_if
- ResponseMessage.content handles null for tool-call-only responses
- Types compile as part of the crate (verified via cargo check)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix reqwest TLS backend and create OpenAI wire-format types** - `fa9e243` (feat)

**Plan metadata:** `45b895b` (docs: complete plan)

## Files Created/Modified
- `Cargo.toml` - Added rustls feature to reqwest dependency
- `Cargo.lock` - Updated with rustls and related TLS dependencies
- `src/openai/types.rs` - All 14 OpenAI wire-format serde types
- `src/openai/mod.rs` - Module declaration for types submodule
- `src/lib.rs` - Added private `mod openai` declaration

## Decisions Made
- **reqwest feature name:** reqwest 0.13 uses `rustls` (not `rustls-tls` as the plan specified). The plan's feature name was for older reqwest versions; 0.13 renamed it to just `rustls`. Applied the correct feature name per deviation Rule 3.
- **All types pub(crate):** Wire-format types are crate-internal, not public API. The provider will convert between these and the public types.
- **ChatMessage as tagged enum:** Uses `#[serde(tag = "role")]` for clean serialization matching OpenAI's `{"role": "system", "content": "..."}` format.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] reqwest 0.13 feature name is `rustls` not `rustls-tls`**
- **Found during:** Task 1 (cargo check after applying plan's feature name)
- **Issue:** Plan specified `rustls-tls` as the reqwest feature name, but reqwest 0.13.2 uses `rustls` (the feature was renamed between reqwest versions)
- **Fix:** Changed feature from `rustls-tls` to `rustls` in Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** `cargo check` passes, `cargo metadata` confirms `rustls` enables full TLS stack (hyper-rustls, tokio-rustls, rustls, aws-lc-rs provider, platform verifier)
- **Committed in:** fa9e243 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Feature name correction was necessary for compilation. Functionally identical outcome to what the plan intended. No scope creep.

## Issues Encountered
- Dead code warnings for all 14 types (expected -- types are pub(crate) and not yet used by any consumer; Plan 02-02 will use them)

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- TLS backend is ready for HTTPS calls in Plan 02-02
- All wire-format types are available for the OpenAI provider implementation
- The `mod openai` declaration in lib.rs will be changed to `pub mod openai` in Plan 02-02

## Self-Check: PASSED

- FOUND: src/openai/types.rs (5392 bytes)
- FOUND: src/openai/mod.rs (22 bytes)
- FOUND: 02-01-SUMMARY.md (4843 bytes)
- FOUND: commit fa9e243 in git log
- PASSED: cargo check compiles without errors

---
*Phase: 02-openai-provider*
*Completed: 2026-02-10*
