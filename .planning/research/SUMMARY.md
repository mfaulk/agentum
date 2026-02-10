# Project Research Summary

**Project:** Agentic Framework (Educational Rust LLM Library)
**Domain:** LLM Framework Library (DAG-based workflows, tool calling)
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH

## Executive Summary

This project aims to build an educational Rust LLM framework centered on DAG-based workflow orchestration with tool calling. Research reveals a clear technical path: use raw HTTP with tokio + reqwest (no SDK wrappers) to show the mechanism, implement a trait-based Model abstraction covering OpenAI and Gemini, and build a topological-sort-based DAG executor that demonstrates async Rust concurrency patterns. The project fills a genuine gap — existing Rust LLM libraries either hide mechanisms (Rig's derive macros) or lack DAG support (llm-chain's sequential chains only).

The recommended approach prioritizes transparency over abstraction. Build the Model trait first with one provider (OpenAI), prove the abstraction with a second provider (Gemini), add single-shot tool calling (no autonomous loops), then construct the DAG engine with parallel execution. Use a builder pattern for workflow construction with validation at build time. Key differentiators: visible HTTP request/response logging, compile-time workflow validation, explicit async concurrency, and zero-magic implementation (no proc macros obscuring behavior).

Critical risks center on async Rust patterns: async trait objects (use enum dispatch to avoid complexity), lifetime infection (own data at async boundaries), and DAG executor deadlocks (use wave-based scheduling). Additional risks include fragile serde deserialization (test with real API fixtures), and over-abstraction that defeats educational goals (prefer concrete types to generic ones). Research confidence is medium-high — the Rust ecosystem recommendations are solid, but exact crate versions need verification and no web-based validation was possible.

## Key Findings

### Recommended Stack

The core stack is mature and stable: tokio for async runtime (de facto standard, reqwest requires it), reqwest for HTTP (built on hyper+tokio, supports async/JSON/streaming), serde ecosystem for serialization (non-negotiable standard), thiserror for library error types, and tracing for structured logging. For DAG execution: petgraph for topological sort and cycle detection (battle-tested graph algorithms), futures crate for `join_all` and parallel execution combinators, and async-trait for dyn-compatible async traits (native async traits don't support trait objects yet).

**Core technologies:**
- **tokio + reqwest + serde**: Async runtime, HTTP, JSON — the standard Rust API client stack
- **Raw HTTP (no SDK crates)**: Educational purpose demands showing API request/response cycle, not hiding behind abstractions
- **petgraph**: Topological sort and DAG validation — correct graph algorithms are hard, reuse the proven implementation
- **async-trait**: Enables `Box<dyn Model>` trait objects — native async fns aren't object-safe yet
- **thiserror**: Library error types with structured context — anyhow is for applications only

**Key decision: Raw HTTP vs SDK wrappers.** Use reqwest directly for OpenAI and Gemini APIs. SDK crates (async-openai, genai) hide the mechanism we're teaching. The entire educational value is in showing how request bodies are constructed, how tool schemas are sent, how responses are parsed, and how provider quirks differ. Two providers force discovery of the correct Model trait abstraction.

### Expected Features

Research identified 9 table stakes features (must have), 6 differentiators (competitive), and 10 anti-features (deliberately exclude).

**Must have (table stakes):**
- **Model abstraction trait** (TS-1): Normalize OpenAI and Gemini behind shared trait. Include tool calling, not streaming (defer).
- **Tool definition system** (TS-3): Tool trait with name/description/schema/execute. Manual implementation, no derive macros (show the mechanism).
- **Single-shot tool execution** (TS-4): One pass — LLM calls tool, tool executes, result returns. No autonomous loops (anti-feature AF-1).
- **DAG workflow engine** (TS-5): Topological sort, parallel branches, data flow. This IS the differentiator.
- **Builder pattern API** (TS-6): Fluent workflow construction with validation at build time.
- **Data flow between steps** (TS-7): Step outputs become inputs to dependent steps. Use `serde_json::Value` at boundaries, typed inside steps.
- **Working examples** (TS-8): Simple chain, parallel branches, tool calling, multi-model. Critical for educational mission.
- **Error handling** (TS-9): Layered errors (model errors, workflow errors) with thiserror. Show Rust error patterns.

**Should have (competitive):**
- **Transparent API logging** (D-1): Debug mode shows raw HTTP JSON. Demystifies LLM APIs completely.
- **Compile-time workflow validation** (D-2): Builder.build() catches cycles, unknown steps, type errors. Rust advantage over Python.
- **Explicit async concurrency** (D-3): DAG executor demonstrates tokio patterns (spawn, join, Arc/Mutex). Rust makes concurrency visible.
- **Provider API comparison docs** (D-4): Document how OpenAI and Gemini formats differ, how trait normalizes them.
- **Zero-magic implementation** (D-5): No proc macros, no codegen. Every abstraction is visible.

**Defer (v2+ or anti-features):**
- **Autonomous agent loops** (AF-1): ReAct pattern adds unbounded execution complexity without teaching new mechanisms. Single-shot is enough.
- **RAG pipeline** (AF-2): Document loading, chunking, embeddings, vector stores — doubles project scope, orthogonal to workflows.
- **Streaming responses** (AF-5): SSE parsing complexity without architectural value. Batch-only for v1.
- **Retry/resilience** (AF-6): Production concerns (backoff, rate limits) obscure core patterns.
- **Derive macros for tools** (AF-4): Rig uses `#[derive(Tool)]` — we show what that generates, manually.

### Architecture Approach

Single-crate library with feature flags (not a workspace — simpler for learners). Module structure: `model.rs` and `tool.rs` as leaf modules (no internal deps), `step.rs` builds on them, `workflow/` contains builder and DAG utilities, `executor.rs` orchestrates async execution, `providers/` implements OpenAI and Gemini behind the Model trait. Execution uses wave-based scheduling (topological sort into levels, execute each level in parallel with `join_all`).

**Major components:**
1. **Model trait + Providers** — CompletionRequest/Response types (provider-agnostic), OpenAiModel and GeminiModel translate to/from wire formats. Trait methods: `model_id()` and `async fn complete()`. Use enum dispatch or async-trait for object safety.
2. **Tool trait + Registry** — Tool definitions (name, schema, execute), ToolRegistry dispatches calls by name. Single-shot flow: LLM returns tool calls, executor dispatches them, sends results back, LLM synthesizes final response.
3. **DAG Executor** — Topological sort via Kahn's algorithm (detects cycles, produces execution waves), wave-based parallel execution (all steps in wave run concurrently via `join_all`), step outputs stored in `HashMap<StepId, serde_json::Value>`.
4. **WorkflowBuilder** — Fluent API with consuming self, validation in `build()` (checks cycles, unknown edges, disconnected nodes), produces immutable Workflow.
5. **Step types** — Enum: `LlmStep` (model + prompt builder + optional tools), `FnStep` (pure function transform). Each receives `StepInputs` (outputs of dependencies) and produces `StepOutput`.

**Key pattern: Provider translation layer.** OpenAI and Gemini have different tool calling JSON shapes. Each provider module has a translation layer: `CompletionRequest` → provider JSON body → HTTP call → provider JSON response → `CompletionResponse`. The Model trait sees only the normalized types. This is the core educational payoff — showing what abstraction layers do.

### Critical Pitfalls

Research identified 17 pitfalls across critical (5), moderate (5), and minor (7) severity.

1. **Async trait object nightmares** — Async fns in traits produce non-object-safe `impl Future` types. Cannot use `Box<dyn Model>` without async-trait crate or manual boxing. **Mitigation:** Decide enum dispatch vs trait objects on day 1. For two providers, enum dispatch is simpler and more educational.

2. **Model abstraction that doesn't abstract** — OpenAI and Gemini have fundamentally different tool calling formats. Abstraction either loses features (lowest-common-denominator) or leaks provider details (escape hatches). **Mitigation:** Define semantic-level abstraction (messages, tool calls, responses), not HTTP-level. Create internal message model, providers translate to/from it.

3. **Serde deserialization fragility** — API responses evolve. Unknown fields, new enum variants, type changes break deserialization. **Mitigation:** Use `#[serde(default)]` liberally, make fields `Option<T>`, never use `#[serde(deny_unknown_fields)]`, avoid `#[serde(untagged)]` enums. Include `#[serde(flatten)]` catch-all for forward compatibility.

4. **DAG executor deadlocks** — Spawning steps that await dependencies without proper scheduling creates deadlock or starvation. **Mitigation:** Use wave-based scheduling (topological sort into levels, execute each level fully before next). Never spawn a step until all its inputs are ready.

5. **Lifetime infection in async** — Borrowed data (`&str`, `&Config`) in async functions prevents `tokio::spawn` (requires `'static + Send`). **Mitigation:** Use owned types at async boundaries from day one (String, not &str; Arc<T> for shared data). Reserve references for synchronous code.

6. **Tool system type rigidity** — Either too generic (PhantomData, higher-kinded types) or too dynamic (all `Value`, no validation). **Mitigation:** Typed definitions, dynamic dispatch. Tool trait defines static schema, `execute()` takes `serde_json::Value` and deserializes internally.

7. **Error handling that swallows context** — Flat error strings make debugging impossible. Opaque `Box<dyn Error>` loses structured data. **Mitigation:** Use thiserror with layered errors (ModelError, WorkflowError). Each layer adds context (which step, which tool) without losing source cause.

8. **Over-abstraction** — Clean DRY code with traits/generics obscures execution paths. Defeats educational purpose. **Mitigation:** Prefer concrete types to generics. One level of indirection maximum. Duplication is acceptable if it improves clarity.

## Implications for Roadmap

Based on research, the dependency chain for MVP is:

```
HTTP Client → Model Trait → Tool System → Tool Execution → Data Flow → DAG Engine → Builder → Examples
```

This is largely sequential, with parallelism opportunities during: error handling (alongside model), tool system and data flow design (overlap), builder and executor co-development, and differentiators (logging, docs) added during relevant phases.

### Phase 1: Foundation (Model + Tools)
**Rationale:** Build the leaf abstractions first. Everything depends on Model and Tool traits. Can be fully developed and tested independently before workflow orchestration.

**Delivers:**
- Model trait with CompletionRequest/Response types
- OpenAI provider implementation (raw HTTP + serde)
- Tool trait with JSON schema generation
- Single-shot tool execution (one LLM call, tool dispatch, final response)
- Error types with thiserror
- Debug logging for HTTP requests/responses (D-1)

**Addresses:** TS-1 (Model abstraction), TS-2 (HTTP client), TS-3 (Tool definition), TS-4 (Single-shot tools), TS-9 (Error handling), D-1 (Transparent logging)

**Avoids:** Pitfall #1 (async trait decision upfront), #2 (define internal message model), #3 (serde strategy), #5 (owned types at boundaries)

**Research needs:** Standard patterns, skip research-phase. OpenAI API docs are well-documented.

### Phase 2: Multi-Model (Abstraction Proof)
**Rationale:** Adding Gemini forces discovery of correct abstraction boundaries. If the trait accommodates both providers cleanly, the design is validated.

**Delivers:**
- Gemini provider implementation
- Provider API comparison documentation (D-4)
- Example showing same workflow with different models

**Addresses:** TS-1 (continued), D-4 (API comparison)

**Avoids:** Pitfall #2 (abstraction validation)

**Research needs:** Minimal. Gemini API docs sufficient.

### Phase 3: Workflow Engine (DAG Orchestration)
**Rationale:** Dependencies complete (Model, Tool). DAG engine is the architectural centerpiece and most complex component.

**Delivers:**
- Step enum (LlmStep, FnStep) with data flow
- Workflow struct (steps + edges)
- DAG utilities (topological sort, cycle detection via petgraph)
- Executor with wave-based parallel execution
- Async concurrency patterns visible (D-3)

**Addresses:** TS-5 (DAG engine), TS-7 (Data flow), D-3 (Async concurrency), D-6 (Step variety)

**Avoids:** Pitfall #4 (deadlock via wave-based scheduling), #9 (use Value for inter-step data)

**Research needs:** Standard patterns (petgraph, tokio). Skip research-phase. Topological sort is well-documented CS.

### Phase 4: Builder API + Polish
**Rationale:** Everything built, now add ergonomic construction layer and comprehensive examples.

**Delivers:**
- WorkflowBuilder with fluent API
- Build-time validation (cycles, unknown steps) (D-2)
- 4+ comprehensive examples covering all features (TS-8)
- Example with MockModel (no API key required)
- Code clarity review for educational quality (D-5)

**Addresses:** TS-6 (Builder), TS-8 (Examples), D-2 (Compile-time validation), D-5 (Zero-magic)

**Avoids:** Pitfall #8 (builder validation), #10 (over-abstraction review)

**Research needs:** None. Implementation-only phase.

### Phase Ordering Rationale

- **Sequential by necessity:** Model trait must exist before steps can use it. Steps must exist before DAG can orchestrate them. Builder constructs what executor runs.
- **Validation points:** Phase 2 validates Phase 1's abstractions. Phase 4 validates entire architecture through examples.
- **Defer complex orchestration:** DAG executor (Phase 3) comes after proving individual components work. Reduces debug surface.
- **Educational progression:** Simple → complex. Single LLM call → multi-provider → parallel workflows → complete builder API.

### Research Flags

**Phases with standard patterns (skip research-phase):**
- **Phase 1:** HTTP clients, trait design, serde patterns — extensively documented in Rust ecosystem
- **Phase 2:** Second provider implementation — same patterns as Phase 1
- **Phase 3:** Topological sort, async concurrency — well-known CS algorithms and tokio patterns
- **Phase 4:** Builder pattern, examples — implementation work, no research needed

**Verification needed before implementation:**
- Exact crate versions (cargo search tokio reqwest serde petgraph async-trait)
- OpenAI API current format (may have changed since training data)
- Gemini API current format (verify tool calling structure)
- Rust async trait dyn compatibility status (may have improved since training cutoff)

**No phases need `/gsd:research-phase` during planning.** Research is comprehensive and covers the full project scope.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core crates (tokio, reqwest, serde, thiserror) stable for years; exact versions need cargo search verification |
| Features | HIGH | Feature categorization based on established LLM framework patterns (LangChain, Rig, llm-chain); table stakes vs differentiators clear |
| Architecture | MEDIUM-HIGH | Architectural patterns (DAG execution, trait abstraction) well-established; specific Rig/llm-chain details from training data unverified |
| Pitfalls | HIGH | Async Rust pitfalls extensively documented; LLM framework pitfalls derived from established libraries |

**Overall confidence:** MEDIUM-HIGH

The Rust ecosystem recommendations are solid (tokio, reqwest, serde are effectively mandatory). Architectural patterns are proven (DAG scheduling, trait abstractions). LLM framework feature expectations are stable (model abstraction, tool calling, workflows). The primary uncertainty is exact crate versions and current LLM API formats, both easily verified during implementation.

### Gaps to Address

**Verification gaps (handle during Phase 1 setup):**
- Confirm latest stable versions: Run `cargo search <crate>` for tokio, reqwest, serde, serde_json, thiserror, tracing, futures, petgraph, async-trait
- Check async trait object status: Verify whether Rust 1.82+ has native dyn-compatible async traits (may eliminate async-trait dependency)
- Validate OpenAI API shape: Check https://platform.openai.com/docs/api-reference/chat for current tool calling format
- Validate Gemini API shape: Check https://ai.google.dev/api/rest for current function calling structure

**Design gaps (resolve during implementation):**
- Enum dispatch vs async-trait for Model: Test both approaches in prototype, choose based on simplicity for educational audience
- Wave-based vs eager DAG execution: Start with wave-based (simpler), document eager as extension
- StepId type safety: Decide newtype vs raw String based on builder API ergonomics
- Mock model strategy: Determine canned response format for testing without API keys

**Documentation gaps (address in Phase 4):**
- Cost estimates for examples: Document expected API costs per example run
- Extension points: Document where to add agent loops, RAG, streaming (the anti-features)
- Provider differences: Create comparison table of OpenAI vs Gemini JSON formats with examples

**None of these gaps block progress.** All can be resolved during normal implementation with quick verification steps.

## Sources

### Primary (HIGH confidence)
- Rust async ecosystem (tokio, async-trait, futures) — established patterns, extensively documented
- Rust serialization (serde, serde_json) — stable standard, 7+ years unchanged
- Rust error handling (thiserror vs anyhow) — well-documented library vs application convention
- Graph algorithms (topological sort, Kahn's algorithm) — computer science fundamentals
- Training data on Rust LLM frameworks (Rig, llm-chain) — architecture patterns, not version-specific details

### Secondary (MEDIUM confidence)
- Rig framework current API — training data from 2024-2025, structure likely unchanged but versions unverified
- llm-chain status (abandoned) — last activity 2023 in training data, current state unverified via web
- OpenAI API tool calling format — training data current to May 2025, format stable but should verify
- Gemini API function calling format — training data current to May 2025, format stable but should verify

### Tertiary (LOW confidence, verify before use)
- Specific crate version numbers (reqwest 0.12, thiserror 2.0, petgraph 0.7) — based on training data, run cargo search to confirm
- Rust 1.82+ async trait object improvements — may have stabilized since training cutoff, check release notes

### Tools unavailable during research
- WebSearch: Could not verify current documentation or crate versions
- WebFetch: Could not retrieve API specs or library docs
- Bash: Could not run cargo search for version verification

All version-specific claims and API format details should be verified with live sources during Phase 1 setup.

---
*Research completed: 2026-02-10*
*Ready for roadmap: yes*
