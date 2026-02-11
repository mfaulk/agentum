# Agentic Framework

## What This Is

A minimal Rust library for building LLM-powered applications through DAG-based workflows. Designed as an educational resource for Rust developers who want to understand how agentic AI patterns work under the hood — defining agents, tools, and workflows from scratch rather than relying on existing frameworks.

## Core Value

Clearly demonstrate how agentic AI patterns (workflows, tool calling, model abstraction) are built, so Rust developers can read the code and understand every layer.

## Requirements

### Validated

- [x] Model async trait with chat completion accepting messages and optional tool definitions (MOD-01)
- [x] OpenAI provider implements Model trait using raw HTTP (MOD-02)
- [x] Model trait supports returning tool call requests from LLM response (MOD-03)
- [x] OpenAI provider handles API key configuration and endpoint construction (MOD-04)
- [x] Tool trait with name, description, JSON schema, and execute method (TOOL-01)
- [x] LLM step includes tool definitions sent as function declarations (TOOL-02)
- [x] Framework dispatches tool calls and returns results to caller (TOOL-03)
- [x] Example tools demonstrate the tool definition pattern (TOOL-04)
- [x] DAG workflows validated for cycles and missing dependencies (WF-01)
- [x] Workflow executor runs steps in topological order (WF-02)
- [x] LLM and Transform step types (WF-03)
- [x] Data flows between steps via StepInput (WF-04)
- [x] Fluent builder pattern API (WF-05)
- [x] Build-time validation for cycles and undefined steps (WF-06)
- [x] Structured error types with thiserror (QLT-01)
- [x] Errors distinguish framework vs runtime (QLT-02)
- [x] Working example programs for all major concepts (QLT-03)

### Active

(None — next milestone will define new requirements)

### Out of Scope

- Autonomous agent loops (deliberate omission — workflows are structured, not open-ended)
- Existing framework dependencies like Rig (defeats the educational purpose)
- Production hardening (retry policies, rate limiting, circuit breakers)
- Streaming responses (adds complexity without educational value for v1)
- Persistent state or checkpointing
- Web UI or CLI tool — this is a library with examples

## Context

- Educational project: code clarity and readability are first-class concerns
- Target audience knows Rust but may be new to LLM application patterns
- "From scratch" means building the core abstractions (model trait, tool dispatch, workflow executor) directly, not wrapping another framework
- v1 ships with OpenAI as the sole provider; Gemini deferred to v2
- DAG execution uses petgraph for topological sorting and cycle detection
- async-trait chosen for dyn-safe Model trait (Box<dyn Model> compiles)
- All types use owned types (String, Vec) for async boundary safety

## Constraints

- **Language**: Rust — the entire point of the project
- **Dependencies**: Minimize external crates; use them for HTTP/JSON/async but not for agentic abstractions
- **API Style**: Builder pattern for workflow construction
- **No looping**: LLM steps make a single call (optionally with tools), no retry/loop behavior

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| DAG over sequential pipelines | Parallel branches show more interesting patterns | v1.0: Implemented with petgraph, topological ordering verified |
| Single-shot tool calling (no loops) | Keeps scope minimal, focuses on the mechanism | v1.0: dispatch/dispatch_all with fail-fast, works as designed |
| Builder pattern API | Idiomatic Rust, readable in examples | v1.0: Consuming-self builder with comprehensive validation |
| OpenAI as sole v1 backend | Ship faster, Gemini deferred to v2 | v1.0: Full HTTP provider with structured errors |
| async-trait for Model trait | dyn-safe dispatch, Box<dyn Model> at runtime | v1.0: Works, Send+Sync supertraits for task sharing |
| Owned types at async boundaries | String/Vec not &str for safe async Send | v1.0: Clean async story, no lifetime issues |

---
*Last updated: 2026-02-11 after v1.0 milestone completion*
