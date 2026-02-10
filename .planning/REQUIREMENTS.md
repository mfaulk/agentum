# Requirements: Agentic Framework

**Defined:** 2026-02-10
**Core Value:** Clearly demonstrate how agentic AI patterns (workflows, tool calling, model abstraction) are built, so Rust developers can read the code and understand every layer.

## v1 Requirements

### Model Layer

- [ ] **MOD-01**: Library defines a `Model` async trait with a method for chat completion that accepts messages and optional tool definitions
- [ ] **MOD-02**: OpenAI provider implements the `Model` trait using raw HTTP (reqwest) against the chat completions API
- [ ] **MOD-03**: Model trait supports returning tool call requests from the LLM response
- [ ] **MOD-04**: OpenAI provider handles API key configuration and endpoint construction

### Tool System

- [ ] **TOOL-01**: Library defines a `Tool` trait with name, description, JSON schema for parameters, and an execute method
- [ ] **TOOL-02**: LLM step can include tool definitions that are sent to the model as function declarations
- [ ] **TOOL-03**: When the model returns a tool call, the framework executes the matching tool and returns the result to the caller
- [ ] **TOOL-04**: Library includes example tools (e.g. calculator, mock weather) that demonstrate the tool definition pattern

### Workflow Engine

- [ ] **WF-01**: Workflows are directed acyclic graphs of steps, validated for cycles and missing dependencies
- [ ] **WF-02**: Workflow executor runs steps in topological order, respecting dependency edges
- [ ] **WF-03**: Steps can be LLM calls (with optional tools) or data transformation functions
- [ ] **WF-04**: Output of a step is passed as input to its dependent steps (data flow)
- [ ] **WF-05**: Workflows are constructed using a builder pattern API (`Workflow::builder()`)
- [ ] **WF-06**: Builder validates the workflow at build time -- cycles and missing dependencies produce compile-time or construction-time errors

### Quality

- [ ] **QLT-01**: Library uses structured error types (thiserror) with clear context for each failure mode
- [ ] **QLT-02**: Errors distinguish between framework errors (bad workflow definition) and runtime errors (API failure, tool error)
- [ ] **QLT-03**: Working example programs demonstrate: single LLM call, LLM with tools, multi-step workflow, workflow with data flow

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
| MOD-01 | Phase 1 | Pending |
| MOD-02 | Phase 2 | Pending |
| MOD-03 | Phase 1 | Pending |
| MOD-04 | Phase 2 | Pending |
| TOOL-01 | Phase 3 | Pending |
| TOOL-02 | Phase 3 | Pending |
| TOOL-03 | Phase 3 | Pending |
| TOOL-04 | Phase 6 | Pending |
| WF-01 | Phase 4 | Pending |
| WF-02 | Phase 4 | Pending |
| WF-03 | Phase 4 | Pending |
| WF-04 | Phase 4 | Pending |
| WF-05 | Phase 5 | Pending |
| WF-06 | Phase 5 | Pending |
| QLT-01 | Phase 1 | Pending |
| QLT-02 | Phase 1 | Pending |
| QLT-03 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-02-10*
*Last updated: 2026-02-10 after roadmap creation*
