# Agentic Framework

## What This Is

A minimal Rust library for building LLM-powered applications through DAG-based workflows. Designed as an educational resource for Rust developers who want to understand how agentic AI patterns work under the hood — defining agents, tools, and workflows from scratch rather than relying on existing frameworks.

## Core Value

Clearly demonstrate how agentic AI patterns (workflows, tool calling, model abstraction) are built, so Rust developers can read the code and understand every layer.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Multi-model support with OpenAI and Gemini backends
- [ ] DAG-based workflow engine that executes steps in dependency order with parallel branches
- [ ] Builder pattern API for defining workflows (steps, edges, data flow)
- [ ] LLM steps that can invoke tools in a single pass (no looping)
- [ ] Tool definition system — developers can define tools the LLM can call
- [ ] Data flow between steps — output of one step feeds as input to dependents
- [ ] Working example programs that demonstrate all major concepts
- [ ] Clear, readable code that prioritizes understanding over abstraction

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
- OpenAI and Gemini have different API shapes for tool calling — the model abstraction layer needs to normalize this
- DAG execution requires topological sorting and tracking step readiness

## Constraints

- **Language**: Rust — the entire point of the project
- **Dependencies**: Minimize external crates; use them for HTTP/JSON/async but not for agentic abstractions
- **API Style**: Builder pattern for workflow construction
- **No looping**: LLM steps make a single call (optionally with tools), no retry/loop behavior

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| DAG over sequential pipelines | Parallel branches show more interesting patterns | — Pending |
| Single-shot tool calling (no loops) | Keeps scope minimal, focuses on the mechanism | — Pending |
| Builder pattern API | Idiomatic Rust, readable in examples | — Pending |
| OpenAI + Gemini as initial backends | Two different API shapes forces a clean abstraction | — Pending |

---
*Last updated: 2026-02-10 after initialization*
