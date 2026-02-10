# Roadmap: Agentic Framework

## Overview

Build an educational Rust library for LLM-powered DAG workflows from the ground up. Phases follow dependency order: core abstractions and error types first, then the OpenAI provider as the first working model, then the tool system layered on top, then the DAG workflow engine, then the builder API for ergonomic construction, and finally working examples that demonstrate every concept end-to-end. Each phase delivers a complete, testable capability.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Core Abstractions** - Define the Model trait, message types, and structured error types
- [x] **Phase 2: OpenAI Provider** - Implement the first working LLM backend with raw HTTP
- [x] **Phase 3: Tool System** - Define the Tool trait, integrate tool declarations with LLM calls, and dispatch tool execution
- [ ] **Phase 4: Workflow Engine** - Build the DAG executor with topological ordering, step types, and data flow
- [ ] **Phase 5: Builder API** - Add fluent workflow construction with build-time validation
- [ ] **Phase 6: Examples** - Create working example programs that demonstrate all major concepts

## Phase Details

### Phase 1: Core Abstractions
**Goal**: Developers can import the library and work with well-defined types for LLM interactions and structured errors
**Depends on**: Nothing (first phase)
**Requirements**: MOD-01, MOD-03, QLT-01, QLT-02
**Success Criteria** (what must be TRUE):
  1. Library crate compiles with the Model trait defined, accepting messages and optional tool definitions
  2. Model trait's response type can represent both plain text completions and tool call requests
  3. Error types distinguish between framework errors (bad workflow definition) and runtime errors (API failure, tool error) with clear context
  4. A developer reading the trait definition understands the full LLM interaction contract without looking at any provider
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md — Project scaffold, message types, and shared value types (ToolCall, ToolDefinition, ModelOptions, ModelResponse)
- [x] 01-02-PLAN.md — Error hierarchy, Model trait, and lib.rs public API re-exports

### Phase 2: OpenAI Provider
**Goal**: Developers can make real LLM calls through the library using the OpenAI API
**Depends on**: Phase 1
**Requirements**: MOD-02, MOD-04
**Success Criteria** (what must be TRUE):
  1. OpenAI provider sends a chat completion request via raw HTTP (reqwest) and returns a parsed response through the Model trait
  2. Provider reads the API key from configuration and constructs correct endpoint URLs
  3. API errors (auth failure, rate limit, malformed response) surface as structured error types, not panics or opaque strings
**Plans**: 2 plans

Plans:
- [x] 02-01-PLAN.md — Fix reqwest TLS backend and define OpenAI wire-format serde types
- [x] 02-02-PLAN.md — OpenAiProvider struct with Model trait impl, conversion logic, and module registration

### Phase 3: Tool System
**Goal**: Developers can define tools the LLM can call, and the framework handles the full tool-calling round trip
**Depends on**: Phase 2
**Requirements**: TOOL-01, TOOL-02, TOOL-03
**Success Criteria** (what must be TRUE):
  1. Developer can implement the Tool trait to define a tool with name, description, JSON parameter schema, and an execute method
  2. An LLM step can include tool definitions that are sent to the model as function declarations in the API request
  3. When the model returns a tool call, the framework locates the matching tool, executes it, and returns the result to the caller
  4. Tool execution errors (unknown tool, bad arguments, execute failure) are captured as structured errors, not panics
**Plans**: 2 plans

Plans:
- [x] 03-01-PLAN.md — Tool trait, ToolRegistry struct, DuplicateTool error variant, and module registration
- [x] 03-02-PLAN.md — Dispatch and dispatch_all methods with unit tests for the full tool-calling round trip

### Phase 4: Workflow Engine
**Goal**: Developers can define multi-step workflows as DAGs that execute in dependency order with data flowing between steps
**Depends on**: Phase 3
**Requirements**: WF-01, WF-02, WF-03, WF-04
**Success Criteria** (what must be TRUE):
  1. A workflow defined as a DAG of steps is validated for cycles and missing dependencies before execution
  2. Executor runs steps in topological order, never executing a step before all its dependencies complete
  3. Steps can be either LLM calls (with optional tools) or pure data transformation functions
  4. Output of a completed step is available as input to all of its dependent steps
  5. A three-step linear workflow (transform -> LLM call -> transform) executes correctly end-to-end
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD
- [ ] 04-03: TBD

### Phase 5: Builder API
**Goal**: Developers construct workflows using a fluent builder pattern that catches structural errors at build time
**Depends on**: Phase 4
**Requirements**: WF-05, WF-06
**Success Criteria** (what must be TRUE):
  1. Workflows are constructed via `Workflow::builder()` with chainable methods for adding steps and edges
  2. `build()` rejects workflows with cycles, producing a clear error rather than silently accepting invalid graphs
  3. `build()` rejects workflows referencing undefined steps, producing a clear error
  4. Builder API reads naturally in example code -- a Rust developer can understand the workflow structure from the builder calls alone
**Plans**: TBD

Plans:
- [ ] 05-01: TBD
- [ ] 05-02: TBD

### Phase 6: Examples
**Goal**: Working example programs demonstrate every major concept so developers can learn by reading and running them
**Depends on**: Phase 5
**Requirements**: QLT-03, TOOL-04
**Success Criteria** (what must be TRUE):
  1. Example program exists for a single LLM call (simplest possible usage)
  2. Example program exists for LLM with tool calling (demonstrates tool definition and dispatch)
  3. Example program exists for a multi-step workflow with data flow between steps
  4. Example tools (e.g. calculator, mock weather) exist and demonstrate the tool definition pattern clearly
  5. Each example compiles and runs successfully (API-dependent examples document required environment variables)
**Plans**: TBD

Plans:
- [ ] 06-01: TBD
- [ ] 06-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Abstractions | 2/2 | Complete | 2026-02-10 |
| 2. OpenAI Provider | 2/2 | Complete | 2026-02-10 |
| 3. Tool System | 0/2 | Not started | - |
| 4. Workflow Engine | 0/TBD | Not started | - |
| 5. Builder API | 0/TBD | Not started | - |
| 6. Examples | 0/TBD | Not started | - |
