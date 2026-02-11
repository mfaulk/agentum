# Requirements: Agentic Framework

**Defined:** 2026-02-10
**Core Value:** Clearly demonstrate how agentic AI patterns (workflows, tool calling, model abstraction) are built, so Rust developers can read the code and understand every layer.

## v1 Requirements

### Model Layer

- [x] **MOD-01**: Library defines a `Model` async trait with a method for chat completion that accepts messages and optional tool definitions
- [x] **MOD-02**: OpenAI provider implements the `Model` trait using raw HTTP (reqwest) against the chat completions API
- [x] **MOD-03**: Model trait supports returning tool call requests from the LLM response
- [x] **MOD-04**: OpenAI provider handles API key configuration and endpoint construction

### Tool System

- [x] **TOOL-01**: Library defines a `Tool` trait with name, description, JSON schema for parameters, and an execute method
- [x] **TOOL-02**: LLM step can include tool definitions that are sent to the model as function declarations
- [x] **TOOL-03**: When the model returns a tool call, the framework executes the matching tool and returns the result to the caller
- [x] **TOOL-04**: Library includes example tools (e.g. calculator, mock weather) that demonstrate the tool definition pattern

### Workflow Engine

- [x] **WF-01**: Workflows are directed acyclic graphs of steps, validated for cycles and missing dependencies
- [x] **WF-02**: Workflow executor runs steps in topological order, respecting dependency edges
- [x] **WF-03**: Steps can be LLM calls (with optional tools) or data transformation functions
- [x] **WF-04**: Output of a step is passed as input to its dependent steps (data flow)
- [x] **WF-05**: Workflows are constructed using a builder pattern API (`Workflow::builder()`)
- [x] **WF-06**: Builder validates the workflow at build time -- cycles and missing dependencies produce compile-time or construction-time errors

### Quality

- [x] **QLT-01**: Library uses structured error types (thiserror) with clear context for each failure mode
- [x] **QLT-02**: Errors distinguish between framework errors (bad workflow definition) and runtime errors (API failure, tool error)
- [x] **QLT-03**: Working example programs demonstrate: single LLM call, LLM with tools, multi-step workflow, workflow with data flow

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Model Layer

- **MOD-05**: Gemini provider implements the Model trait
- **MOD-06**: Transparent request/response logging for debugging API interactions

### Workflow Engine

- **WF-07**: Independent steps in the same DAG level execute concurrently (parallel waves)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Autonomous agent loops | Deliberate omission -- workflows are structured, not open-ended |
| Streaming responses | Adds complexity without educational value for v1 |
| RAG / vector store integration | Separate concern, not core to workflow patterns |
| Memory / conversation persistence | Adds state management complexity beyond v1 scope |
| Derive macros for tool definition | Magic that hides the pattern -- against educational goal |
| CLI tool or web UI | This is a library with examples |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| MOD-01 | Phase 1 | Complete |
| MOD-02 | Phase 2 | Complete |
| MOD-03 | Phase 1 | Complete |
| MOD-04 | Phase 2 | Complete |
| TOOL-01 | Phase 3 | Complete |
| TOOL-02 | Phase 3 | Complete |
| TOOL-03 | Phase 3 | Complete |
| TOOL-04 | Phase 6 | Complete |
| WF-01 | Phase 4 | Complete |
| WF-02 | Phase 4 | Complete |
| WF-03 | Phase 4 | Complete |
| WF-04 | Phase 4 | Complete |
| WF-05 | Phase 5 | Complete |
| WF-06 | Phase 5 | Complete |
| QLT-01 | Phase 1 | Complete |
| QLT-02 | Phase 1 | Complete |
| QLT-03 | Phase 6 | Complete |

**Coverage:**
- v1 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-02-10*
*Last updated: 2026-02-10 after roadmap creation*
