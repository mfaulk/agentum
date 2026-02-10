# Stack Research

**Domain:** Rust LLM framework library (educational, agentic AI patterns)
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH (core Rust ecosystem crates are stable and well-known; exact latest versions unverified due to tool limitations)

## Verification Note

WebSearch, WebFetch, and Bash were unavailable during this research session. All version numbers and recommendations are based on training data (cutoff ~May 2025) and deep familiarity with the Rust ecosystem. The core crates recommended here (tokio, reqwest, serde, thiserror) have been the standard Rust stack for years and are unlikely to have been displaced. However, **exact version numbers should be verified** by running `cargo search <crate>` before writing Cargo.toml. Confidence levels reflect this limitation.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **tokio** | ~1.43+ | Async runtime | The de facto standard async runtime in Rust. reqwest requires it. Every Rust HTTP library and LLM SDK assumes tokio. No reason to use anything else. | HIGH |
| **reqwest** | ~0.12+ | HTTP client | Built on hyper+tokio. Supports async, TLS, JSON, streaming, timeouts. Used by Rig, used by virtually every Rust project hitting APIs. | HIGH |
| **serde** | ~1.0 | Serialization framework | The Rust serialization standard. Every API client, every config system uses it. Non-negotiable. | HIGH |
| **serde_json** | ~1.0 | JSON serialization | serde's JSON implementation. Needed for OpenAI and Gemini API request/response bodies. | HIGH |
| **thiserror** | ~2.0+ | Error type derivation | For defining library error types with `#[derive(Error)]`. The standard choice for library code (vs anyhow for applications). | HIGH |
| **tracing** | ~0.1 | Structured logging/diagnostics | The standard Rust observability crate. Educational value: shows how to instrument async workflows. | HIGH |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| **tokio** (features: `rt-multi-thread`, `macros`) | ~1.43+ | Full async runtime with `#[tokio::main]` | Always -- needed for examples and integration tests | HIGH |
| **serde** (feature: `derive`) | ~1.0 | `#[derive(Serialize, Deserialize)]` | Always -- used on every API struct | HIGH |
| **reqwest** (features: `json`, `rustls-tls`) | ~0.12+ | JSON body helpers, TLS without OpenSSL | Always -- `rustls-tls` avoids system OpenSSL dependency headaches | HIGH |
| **secrecy** | ~0.10+ | Wrapping API keys so they don't leak in logs | For API key handling in model clients | MEDIUM |
| **dotenvy** | ~0.15+ | Loading .env files | For examples only (loading API keys from env) | MEDIUM |
| **tokio-test** | ~0.4+ | Test utilities for async code | For unit/integration tests | MEDIUM |
| **wiremock** | ~0.6+ | HTTP mocking for tests | For testing API clients without hitting real endpoints | MEDIUM |
| **petgraph** | ~0.7+ | Graph data structures and algorithms | For DAG representation, topological sorting | MEDIUM |
| **futures** | ~0.3 | Future combinators (`join_all`, `select`, etc.) | For parallel execution of DAG branches | HIGH |
| **async-trait** | ~0.1 | Async functions in traits | For the `Model` trait and `Tool` trait -- though native async traits are stabilizing, async-trait is still more practical for dyn dispatch | MEDIUM |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **cargo clippy** | Linting | Run with `-- -W clippy::all` for educational-quality code |
| **cargo fmt** | Formatting | Use default rustfmt config for consistency |
| **cargo doc** | Documentation generation | Educational project should have thorough doc comments |
| **cargo test** | Testing | Async tests need `#[tokio::test]` attribute |
| **cargo-nextest** | Faster test runner | Optional but nice for larger test suites |

---

## Key Decision: Raw HTTP vs SDK Crates for LLM Providers

### Recommendation: Raw HTTP with reqwest

**Use reqwest directly to call OpenAI and Gemini APIs. Do NOT use `async-openai`, `openai-api-rs`, or similar SDK crates.**

**Rationale:**

1. **Educational purpose demands it.** The entire point of this library is showing how LLM integrations work. Wrapping an SDK crate hides the most interesting part -- the API request/response cycle, tool call serialization, and provider-specific quirks.

2. **Provider SDKs are volatile.** OpenAI and Google change their APIs frequently. SDK crates often lag behind, break, or add unwanted abstractions. Raw HTTP with serde structs puts you in control.

3. **Two providers forces clean abstraction.** When you build both OpenAI and Gemini from raw HTTP, you naturally discover what the `Model` trait should look like. This is the educational payoff.

4. **Less dependency surface.** SDK crates pull in their own HTTP clients, error types, and async runtimes. Raw reqwest keeps the dependency tree minimal and comprehensible.

**What this means in practice:**
- Define Rust structs for OpenAI's chat completion API (request/response) with serde
- Define Rust structs for Gemini's generateContent API (request/response) with serde
- Write thin client modules that use reqwest to POST to these APIs
- Both implement a shared `Model` trait with a `complete()` method

### SDK Crates Explicitly Rejected

| Crate | Why Not |
|-------|---------|
| `async-openai` | Hides the HTTP layer. Over-abstracted for educational use. Pulls its own error types. |
| `openai-api-rs` | Same issues. Also less maintained. |
| `rig-core` | Using Rig defeats the project's "from scratch" purpose entirely. |
| `llm-chain` | Abandoned/unmaintained. Was never production-quality. |
| `langchain-rust` | Port of Python patterns. Not idiomatic Rust. Wraps too much. |
| `genai` | Adds another abstraction layer we're trying to teach people to build. |

---

## Key Decision: petgraph vs Hand-Rolled DAG

### Recommendation: petgraph

**Use petgraph for DAG representation and topological sorting.**

**Rationale:**

1. **Correct graph algorithms are hard.** Topological sort, cycle detection, and parallel-ready execution ordering are non-trivial to get right. petgraph has been battle-tested for years.

2. **Still educational.** Using petgraph for the graph structure while building the execution engine yourself is the right level of abstraction. The interesting educational content is the executor (how to run DAG steps in parallel with data flow), not the graph data structure itself.

3. **Small, focused dependency.** petgraph does one thing (graph algorithms) and does it well. No runtime overhead, no async opinion.

**Alternative considered:** Hand-rolling a simple DAG with `Vec<Node>` and adjacency lists. This is viable for a simple DAG but you'd need to implement topological sort and cycle detection yourself. For an educational project, this is not where you want readers to spend their attention.

---

## Key Decision: async-trait vs Native Async Traits

### Recommendation: Use native async traits where possible, async-trait for dyn dispatch

**Rationale:**

As of Rust 1.75+ (stable since late 2023), `async fn` in traits is supported natively. However, there is a critical limitation: **async trait methods cannot be used with `dyn Trait`** without boxing the future yourself. Since the `Model` trait will likely be used as `Box<dyn Model>` or `&dyn Model` in the workflow engine:

- **Option A:** Use `async-trait` crate which handles the boxing automatically. Simpler, well-understood, and the code reads cleanly.
- **Option B:** Use native async traits and manually return `Pin<Box<dyn Future>>`. More explicit but noisier.

For an educational project, `async-trait` is the better choice because it keeps the trait definitions readable. The `#[async_trait]` attribute is well-known in the Rust community and readers will recognize it. Add a code comment explaining what it does under the hood.

**Update note (MEDIUM confidence):** Rust may have stabilized `dyn` compatibility for async traits by now (trait_variant or similar). Verify before implementation. If native support is available, prefer it.

---

## Key Decision: Error Handling Strategy

### Recommendation: thiserror for the library, anyhow for examples

**Rationale:**

| Context | Crate | Why |
|---------|-------|-----|
| Library code (`src/`) | `thiserror` | Libraries should expose typed errors. `thiserror` derives `std::error::Error` with zero runtime cost. Callers can pattern-match on error variants. |
| Example programs (`examples/`) | `anyhow` | Examples should focus on the happy path. `anyhow::Result` erases error types for simplicity. `?` propagation just works. |

**Error type structure:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API error from {provider}: {message}")]
    Api { provider: String, message: String },

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Tool execution failed: {0}")]
    Tool(String),
}
```

This is a standard Rust library error pattern. Educational value: shows how to compose errors from dependency errors using `#[from]`.

---

## Alternatives Considered

| Category | Recommended | Alternative | When to Use Alternative |
|----------|-------------|-------------|-------------------------|
| Async runtime | tokio | async-std | Never for this project. async-std has less ecosystem support, reqwest doesn't support it. |
| Async runtime | tokio | smol | Never. Minimalist but lacks ecosystem support. |
| HTTP client | reqwest | hyper (directly) | Never for this project. hyper is low-level; reqwest wraps it with ergonomic API. |
| HTTP client | reqwest | ureq | Only if you wanted blocking HTTP. We need async for parallel DAG execution. |
| JSON | serde_json | simd-json | Not needed. We're not parsing massive JSON payloads. serde_json is standard. |
| Error handling | thiserror | derive_more | thiserror is more focused on Error derivation. derive_more does too many things. |
| Graphs | petgraph | daggy | daggy is built on petgraph but adds DAG-specific enforcement. Could use, but petgraph directly is better known and documented. |
| Logging | tracing | log | tracing is the modern standard. Structured, async-aware, spans for workflows. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `async-openai` | Hides HTTP layer, defeats educational purpose | Raw reqwest + serde structs |
| `rig-core` | Defeats "from scratch" purpose entirely | Build your own Model trait |
| `llm-chain` | Unmaintained, last meaningful commits were 2023 | Build your own chain abstractions |
| `langchain-rust` | Port of Python patterns, not idiomatic Rust | Build your own workflow engine |
| `async-std` | Incompatible with reqwest, smaller ecosystem | tokio |
| `openssl` (feature) | System dependency pain, cross-compilation issues | reqwest with `rustls-tls` feature |
| `log` crate | Older, unstructured, not async-aware | tracing |
| `failure` crate | Deprecated since 2019 | thiserror |
| `snafu` | Less popular than thiserror, more ceremony | thiserror |

---

## Cargo.toml Structure

### Library Cargo.toml (`Cargo.toml`)

```toml
[package]
name = "agentic-framework"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"  # Minimum for async fn in traits

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
futures = "0.3"
petgraph = "0.7"
async-trait = "0.1"

[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
anyhow = "1"
dotenvy = "0.15"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Notes:**
- `reqwest` with `default-features = false` disables the default `native-tls` and uses `rustls-tls` instead. Verify this is correct for your reqwest version.
- `tokio` features: `rt-multi-thread` for the runtime, `macros` for `#[tokio::main]` and `#[tokio::test]`, `sync` for channels/mutexes used in DAG execution.
- `rust-version = "1.75"` ensures async fn in traits is available.

### Version Confidence

| Crate | Listed Version | Confidence | Notes |
|-------|---------------|------------|-------|
| tokio | 1 | HIGH | tokio 1.x has been stable since 2021. Semver-compatible updates only. |
| reqwest | 0.12 | MEDIUM | Was 0.12 as of mid-2025. May have bumped to 0.13. Verify with `cargo search`. |
| serde | 1 | HIGH | serde 1.x has been stable for 7+ years. |
| serde_json | 1 | HIGH | Same stability as serde. |
| thiserror | 2 | MEDIUM | thiserror 2.0 was released in late 2024. Verify it's the current major. |
| tracing | 0.1 | HIGH | tracing 0.1.x has been the stable line for years. |
| futures | 0.3 | HIGH | futures 0.3.x stable for years. |
| petgraph | 0.7 | MEDIUM | Was 0.6.x for a long time, 0.7 may or may not be released. Verify. |
| async-trait | 0.1 | HIGH | Stable and unchanged for years. |
| wiremock | 0.6 | LOW | Version uncertain. Verify with `cargo search wiremock`. |

---

## What Existing Rust LLM Libraries Use

### Rig (rig-core)

Rig is the most mature Rust LLM framework as of early 2025. Understanding its stack choices informs ours.

**Rig's stack (HIGH confidence, from training data):**
- **tokio** for async runtime
- **reqwest** for HTTP
- **serde/serde_json** for serialization
- **thiserror** for error types
- **tracing** for logging
- Provider-specific modules for OpenAI, Anthropic, Cohere, etc.
- Builder pattern for agents and completion requests

**What Rig does well (and we should learn from):**
- Clean `CompletionModel` trait abstraction
- Separate provider crates (rig-core, rig-openai, etc.)
- Builder pattern for constructing agent configurations

**What Rig does that we explicitly skip:**
- RAG/embedding support (out of scope for educational v1)
- Vector store integrations
- Complex agent loops
- Many provider integrations

### llm-chain

**Status:** Largely unmaintained as of 2024. Last significant commits in mid-2023.
**Stack:** Used tokio, reqwest, serde. Tried to replicate LangChain patterns in Rust.
**Lesson:** Don't over-abstract. LangChain's Python patterns (chains, agents, memory) don't translate well to Rust's type system. Build Rust-idiomatic abstractions.

### Other Notable Crates

| Crate | Status | Relevance |
|-------|--------|-----------|
| `genai` | Active (small) | Simple multi-provider client. Good reference for API struct definitions. |
| `ollama-rs` | Active | Ollama-specific. Not relevant for OpenAI/Gemini but shows good Rust API client patterns. |
| `mistralrs` | Active | Inference engine, not a framework. Different domain. |

---

## Stack Patterns by Project Phase

**Phase 1 (Foundation):**
- tokio, serde, serde_json, thiserror, reqwest
- Just the Model trait + one provider (OpenAI)
- Focus: API structs, HTTP client, basic completion

**Phase 2 (Multi-model):**
- Add second provider (Gemini)
- async-trait (if needed for dyn Model dispatch)
- Focus: Trait abstraction, normalizing different API shapes

**Phase 3 (Tools):**
- serde_json (for tool argument schema generation)
- Focus: Tool trait, JSON Schema for function definitions, tool call parsing

**Phase 4 (DAG Workflows):**
- petgraph, futures (`join_all` for parallel branches)
- tokio sync primitives (channels, oneshot)
- Focus: DAG construction, topological execution, data flow

**Phase 5 (Polish):**
- tracing, tracing-subscriber
- More examples, documentation
- Focus: Observability, developer experience

---

## OpenAI API Shape (for struct definition reference)

**Endpoint:** `POST https://api.openai.com/v1/chat/completions`

Key request fields:
- `model`: string (e.g., "gpt-4o")
- `messages`: array of `{role, content}` objects
- `tools`: optional array of tool definitions (JSON Schema)
- `tool_choice`: optional ("auto", "none", or specific)

Key response fields:
- `choices[0].message.content`: the text response
- `choices[0].message.tool_calls`: array of `{id, type, function: {name, arguments}}` if tools were called
- `usage`: token counts

## Gemini API Shape (for struct definition reference)

**Endpoint:** `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`

Key request fields:
- `contents`: array of `{role, parts}` objects
- `tools`: optional array with `function_declarations`
- `tool_config`: optional tool selection config

Key response fields:
- `candidates[0].content.parts`: array of parts (text or function_call)
- `candidates[0].content.parts[n].functionCall`: `{name, args}` if tool was called
- `usageMetadata`: token counts

**Key difference:** OpenAI uses `tool_calls` in the response message; Gemini uses `functionCall` parts in the content. The Model trait must normalize this.

---

## Sources

- Training data knowledge of Rust ecosystem (HIGH confidence for core crates, they haven't changed significantly)
- Training data knowledge of Rig framework architecture (MEDIUM confidence -- verified it exists and uses these patterns, but exact current version unverified)
- Training data knowledge of OpenAI/Gemini API shapes (MEDIUM confidence -- APIs may have evolved since May 2025)
- **No live verification was possible** -- WebSearch, WebFetch, and Bash tools were unavailable
- All version numbers should be verified with `cargo search <crate>` before writing actual Cargo.toml

---

## Action Items Before Implementation

1. **Verify crate versions:** Run `cargo search tokio reqwest serde thiserror petgraph async-trait tracing` to confirm latest versions
2. **Check OpenAI API docs:** Confirm current chat completions endpoint and tool calling format at https://platform.openai.com/docs/api-reference
3. **Check Gemini API docs:** Confirm current generateContent endpoint at https://ai.google.dev/api/rest
4. **Check async trait status:** Verify whether `dyn`-compatible async traits have been stabilized in recent Rust versions (1.82+), which could eliminate the async-trait crate dependency
5. **Check petgraph version:** Confirm whether 0.7.x is released or if 0.6.x is still current

---
*Stack research for: Rust LLM agentic framework library*
*Researched: 2026-02-10*
*Tool limitations: WebSearch, WebFetch, Bash unavailable -- recommendations based on training data with confidence levels noted*
