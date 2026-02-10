# Domain Pitfalls

**Domain:** Rust LLM agentic framework library (educational)
**Researched:** 2026-02-10
**Overall confidence:** MEDIUM-HIGH (based on established Rust ecosystem patterns; web-based verification unavailable)

---

## Critical Pitfalls

Mistakes that cause architectural rewrites or block progress entirely.

---

### Pitfall 1: Async Trait Object Nightmares -- Designing Model Traits That Cannot Be Made Into `dyn Trait`

**What goes wrong:** You define a `Model` trait with async methods, then discover you cannot use `Box<dyn Model>` because async methods return opaque `impl Future` types, which are not object-safe. You end up unable to store different model implementations (OpenAI, Gemini) in the same workflow step without knowing the concrete type at compile time.

**Why it happens:** Rust's native `async fn` in traits (stabilized in Rust 1.75) produces `impl Future` return types, which are not object-safe. The `async-trait` crate solves this by boxing the future, but introduces `Send` bound complexity. Developers often start with native async trait syntax, build several components around it, then hit the wall when they need dynamic dispatch.

**Consequences:** You either: (a) rewrite every trait to use `async-trait` or manual `Box<Pin<dyn Future>>`, rippling changes through the entire codebase, or (b) abandon dynamic dispatch entirely and use enum dispatch, which changes the architecture fundamentally.

**Prevention:**
- **Decide upfront: enum dispatch vs. trait objects.** For an educational library with exactly two backends (OpenAI + Gemini), enum dispatch (`enum ModelBackend { OpenAI(...), Gemini(...) }`) is simpler, avoids the entire async trait object problem, and is more readable for learners.
- If trait objects are desired (for extensibility), use the `async-trait` crate from day one, or manually return `Pin<Box<dyn Future<Output = Result<...>> + Send + 'a>>`.
- Test the dispatch pattern in isolation BEFORE building the rest of the system around it.
- Native async traits work fine if you only ever use static dispatch (`impl Model` or generics), but the moment you need a `Vec<Box<dyn Model>>` or `HashMap<String, Box<dyn Model>>`, you need the boxing solution.

**Detection (warning signs):**
- "Cannot make into object" compiler errors referencing async trait methods.
- Growing use of generics/monomorphization where you really want runtime polymorphism.
- Inability to store mixed model types in workflow step configurations.

**Phase relevance:** Must be decided in Phase 1 (Model Abstraction Layer). Wrong choice here poisons everything downstream.

**Confidence:** HIGH -- this is one of the most well-documented Rust async pitfalls.

**Recommendation for this project:** Use enum dispatch. Two backends with a shared trait for the common interface, but dispatch through a concrete enum. This avoids async trait object complexity entirely, is more readable for educational purposes, and two variants is not enough to justify the abstraction cost of dynamic dispatch.

---

### Pitfall 2: Model Abstraction Layer That Doesn't Actually Abstract

**What goes wrong:** You build a "unified" model trait, but it leaks provider-specific concepts. The abstraction either: (a) becomes lowest-common-denominator (losing provider features), or (b) grows escape hatches that make every consumer provider-aware anyway.

**Why it happens:** OpenAI and Gemini have fundamentally different API shapes for tool calling:
- OpenAI uses `tools` array with `function` type, returns `tool_calls` in the assistant message with `id` fields, expects `tool` role messages with matching IDs.
- Gemini uses `tools` with `function_declarations`, returns `functionCall` parts, expects `functionResponse` parts.
- Parameter schemas differ subtly (OpenAI uses strict JSON Schema, Gemini uses a subset).
- Response structures differ (OpenAI: choices[0].message, Gemini: candidates[0].content).

Developers try to paper over these differences with a trait, but the tool calling flow is deeply coupled to the message format.

**Consequences:** Either the abstraction is so thin it adds no value (just renaming fields), or it's so thick that adding a third provider requires understanding every normalization decision. Educational value drops because readers cannot see *why* the abstraction exists.

**Prevention:**
- **Define the abstraction boundary at the semantic level, not the HTTP level.** The trait should speak in terms of: "send messages, get a response that may include tool calls" -- not in terms of JSON shapes.
- Create a crate-internal message/response model that is neither OpenAI's nor Gemini's format. Each provider adapter converts TO and FROM this internal model.
- Keep tool calling representation provider-agnostic: `ToolCall { name: String, arguments: serde_json::Value }` -- both providers can map to/from this.
- **Explicitly document what is lost** in the abstraction. If OpenAI supports `parallel_tool_calls` and Gemini doesn't, the trait should not expose it. Make this a conscious, documented design decision.

**Detection (warning signs):**
- The model trait has methods like `openai_specific_config()`.
- Consumer code checks which provider is being used before constructing requests.
- Adding a third provider (Anthropic, Ollama) would require changes outside the provider module.

**Phase relevance:** Phase 1 (Model Abstraction). Get the internal message model right before implementing either provider.

**Confidence:** HIGH -- this is a universal abstraction design challenge, well-documented in the LLM framework space.

---

### Pitfall 3: Serde Deserialization That Silently Drops Data or Panics on API Changes

**What goes wrong:** You define Rust structs mirroring the LLM provider's JSON response, then an API update adds a field, changes a type, or returns an unexpected variant. Your deserialization either: (a) panics/errors on the unknown field, (b) silently drops the new data, or (c) fails on a legitimate response because your enum doesn't cover a new case.

**Why it happens:** LLM provider APIs evolve rapidly. OpenAI adds new `finish_reason` variants, new message types, new tool calling modes. Serde's default behavior with `#[serde(deny_unknown_fields)]` will reject responses with new fields. Without it, new fields are silently ignored -- which is usually fine, but can mask breaking changes. Worse, if you use `#[serde(untagged)]` enums to handle polymorphic responses, error messages become useless ("data did not match any variant").

**Consequences:** Production breakage when a provider updates their API. Difficult debugging because serde's untagged enum errors reveal nothing about what actually went wrong. Users of your library file issues you cannot reproduce because they hit a response shape you never saw.

**Prevention:**
- **Never use `#[serde(deny_unknown_fields)]`** on API response types. Allow unknown fields to pass through.
- **Always use `#[serde(default)]` on optional fields** and make fields `Option<T>` liberally. A missing field should not crash deserialization.
- **Avoid `#[serde(untagged)]` enums for API responses.** Use tagged enums where possible, or deserialize into `serde_json::Value` first and then pattern-match manually with better error messages.
- **Include a `serde_json::Value` catch-all** in response types for forward compatibility:
  ```rust
  #[derive(Deserialize)]
  pub struct ChatResponse {
      pub id: String,
      pub choices: Vec<Choice>,
      // Captures everything we don't explicitly model
      #[serde(flatten)]
      pub extra: HashMap<String, serde_json::Value>,
  }
  ```
- **Write deserialization tests using saved JSON fixtures** from real API responses. When a provider changes, you can add the new fixture and verify your types still deserialize.

**Detection (warning signs):**
- Tests use hand-crafted JSON instead of real API response samples.
- No `#[serde(default)]` on any optional fields.
- Use of `#[serde(untagged)]` on enums with more than 2 variants.
- Deserialization errors in production that say "data did not match any variant."

**Phase relevance:** Phase 1 (Model Abstraction) and Phase 2 (Provider Implementations). Create the serde strategy before writing the first provider.

**Confidence:** HIGH -- well-known serde pattern. The `#[serde(flatten)]` forward-compatibility pattern is widely used in Rust API clients.

---

### Pitfall 4: DAG Executor That Deadlocks or Starves

**What goes wrong:** The workflow DAG executor spawns async tasks for each step but creates a deadlock scenario: Step A waits for Step B's output, Step B waits for Step C, and the executor's concurrency limit means Step C never gets scheduled. Or: all executor slots are consumed by steps waiting for their dependencies, and no dependency-free steps can make progress.

**Why it happens:** DAG execution requires careful scheduling. A naive "spawn all ready steps, wait for completions, spawn newly-ready steps" loop works but is fragile. Common mistakes:
- Using a fixed-size `Semaphore` or thread pool and spawning steps that hold a slot while awaiting dependencies.
- Not properly tracking which steps are "ready" (all dependencies satisfied) vs. "pending" (has unsatisfied dependencies).
- Spawning too eagerly -- starting steps whose dependencies haven't completed, then blocking inside the task.

**Consequences:** Deadlock (the workflow hangs forever), starvation (steps that could run are delayed unnecessarily), or panic (attempts to read outputs that don't exist yet).

**Prevention:**
- **Separate scheduling from execution.** The scheduler identifies which steps are ready (all dependencies met); the executor runs only ready steps. A step is NEVER spawned until all its inputs are available.
- **Use a simple topological-sort-based level scheduler for v1:**
  1. Topological sort the DAG.
  2. Group steps into "levels" where each level's steps are independent of each other.
  3. Execute each level fully before starting the next.
  4. Within a level, run all steps concurrently with `futures::future::join_all` or `tokio::JoinSet`.
  This is correct by construction and easy to understand. It sacrifices some parallelism (a step in level 3 that only depends on level 1 must wait for all of level 2), but for an educational library this tradeoff is excellent.
- **For maximum parallelism (advanced):** Use a channel-based approach where each completed step signals dependents, and dependents check if all their inputs are ready. Only add to the ready queue when all dependencies are satisfied. But this is significantly more complex.
- **Always validate the DAG for cycles** before execution (topological sort will detect this).

**Detection (warning signs):**
- Workflow tests pass for linear chains but hang for diamond-shaped DAGs (A -> B, A -> C, B -> D, C -> D).
- Steps occasionally execute before their inputs are ready.
- Tests are flaky or timing-dependent.

**Phase relevance:** Phase 3 (DAG Execution Engine). Must be designed carefully, but the level-based approach largely avoids these issues.

**Confidence:** HIGH -- DAG scheduling is well-understood computer science, and the level-based approach is a proven simple solution.

---

### Pitfall 5: Lifetime Infection in Async Contexts -- References That Prevent `Send` Futures

**What goes wrong:** You pass borrowed data (`&str`, `&Config`) into async functions, and the resulting future captures the lifetime. Because the future must be `Send` (to work with `tokio::spawn`), and the borrowed data lives on a specific thread's stack, the compiler rejects the code. You then face a cascade: either clone everything (performance cost, cluttered API), restructure to use `Arc<T>`, or fight with explicit lifetime bounds that infect every function signature upward.

**Why it happens:** `tokio::spawn` requires `'static + Send` futures. Any reference with a non-`'static` lifetime in an async context prevents the future from being `'static`. This is correct safety behavior, but it means that the "zero-cost borrowing" paradigm of synchronous Rust code does not translate directly to async code. Beginners (and even experienced Rustaceans) often design APIs with borrows first, then discover they cannot spawn tasks with those APIs.

**Consequences:** Either massive API churn (changing `&self` methods to take `Arc<Self>`, changing `&str` parameters to `String`), or avoiding `spawn` entirely and using only `.await` (which prevents parallelism in the DAG executor).

**Prevention:**
- **Use owned types at async boundaries from the start.** Workflow steps, model configs, and tool definitions should own their data (`String`, not `&str`; `Config`, not `&Config`).
- **Use `Arc<T>` for shared immutable state** (like the model client configuration that multiple steps need). This is idiomatic in async Rust and costs almost nothing.
- **Reserve references for synchronous, non-spawning code paths** (builder pattern validation, for instance).
- **Rule of thumb:** If a function might be `.await`ed inside a spawned task, its parameters should be owned or `Arc`-wrapped. Design the API with this assumption from day one.

**Detection (warning signs):**
- Compiler errors about "borrowed value does not live long enough" in async contexts.
- Errors about futures not being `Send` because of borrowed data.
- Growing number of `.clone()` calls added reactively to fix compilation errors.

**Phase relevance:** Phase 1 (Model Abstraction) and Phase 3 (DAG Execution). The model trait's method signatures must be designed with spawn-compatibility in mind.

**Confidence:** HIGH -- this is the most common Rust async learning curve issue, extensively documented.

---

## Moderate Pitfalls

Mistakes that cause delays, technical debt, or poor educational value.

---

### Pitfall 6: Tool Calling Type System That Is Either Too Rigid or Too Dynamic

**What goes wrong:** Two failure modes:
- **Too rigid:** Tool parameter types are fully modeled as Rust types with generics. Defining a new tool requires implementing complex trait bounds. The type machinery is so heavy that readers cannot understand the tool system without understanding advanced Rust generics first.
- **Too dynamic:** Everything is `serde_json::Value`. No type safety, no compile-time validation of tool definitions, easy to pass wrong parameters. The code reads like Python with extra syntax.

**Why it happens:** LLM tool calling is inherently dynamic -- the LLM chooses which tool to call and provides arguments as JSON. But Rust developers instinctively reach for static typing. The tension between "the LLM sends arbitrary JSON" and "Rust wants types" creates a design challenge.

**Prevention:**
- **Use typed definitions, dynamic dispatch.** Tool definitions should be static (name, description, parameter JSON schema) but tool *execution* should accept `serde_json::Value` arguments and internally deserialize to typed parameters.
- **The sweet spot pattern:**
  ```rust
  // Tool definition is static and declarative
  pub struct ToolDef {
      pub name: String,
      pub description: String,
      pub parameters: serde_json::Value, // JSON Schema
  }

  // Tool execution receives dynamic args, deserializes internally
  pub trait Tool: Send + Sync {
      fn definition(&self) -> ToolDef;
      fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolError>;
  }

  // Concrete tool implementations deserialize to their typed args
  impl Tool for WeatherTool {
      fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolError> {
          let params: WeatherParams = serde_json::from_value(args)?;
          // ... typed code from here on
      }
  }
  ```
- This pattern is readable (good for education), type-safe at the implementation level, and compatible with LLM-provided dynamic arguments.
- **Do NOT use `inventory` or `linkme` crates** for auto-registration of tools via proc macros. This is magical and anti-educational.

**Detection (warning signs):**
- Tool trait has more than 2 generic parameters.
- Users need to understand `PhantomData` or higher-kinded types to define a tool.
- All tool parameters are untyped `Value` with no validation.

**Phase relevance:** Phase 2 (Tool System). Design the `Tool` trait before implementing tool dispatch.

**Confidence:** HIGH -- this pattern is common across Rust LLM libraries (rig, llm-chain, etc.).

---

### Pitfall 7: Error Handling That Swallows Context or Blocks Recovery

**What goes wrong:** You define a single `Error` enum for the entire crate, and every error variant is either: (a) a flat string message with no structured data, or (b) an opaque wrapper around `Box<dyn std::error::Error>`. When a workflow step fails, the user cannot programmatically determine whether the failure was a network timeout (retryable), a malformed API response (bug), or an invalid tool call (LLM's fault).

**Why it happens:** Rust's error handling ecosystem offers many choices (`anyhow`, `thiserror`, custom enums). For libraries, `anyhow` is inappropriate (it's for applications -- it erases error types). Custom error enums are right but tedious. Developers either over-use `anyhow` for convenience or create flat error enums with no context chain.

**Prevention:**
- **Use `thiserror` for the public error types.** It generates `Display` and `Error` implementations and supports `#[from]` for conversion.
- **Layer errors by component:**
  ```rust
  // Model layer errors
  #[derive(Debug, thiserror::Error)]
  pub enum ModelError {
      #[error("HTTP request failed: {0}")]
      Http(#[from] reqwest::Error),
      #[error("Failed to deserialize response: {0}")]
      Deserialization(#[from] serde_json::Error),
      #[error("API returned error: {status} {message}")]
      ApiError { status: u16, message: String },
  }

  // Workflow layer errors (wraps model errors with context)
  #[derive(Debug, thiserror::Error)]
  pub enum WorkflowError {
      #[error("Step '{step_name}' failed: {source}")]
      StepFailed {
          step_name: String,
          #[source]
          source: ModelError,
      },
      #[error("Cycle detected in workflow DAG")]
      CycleDetected,
      // ...
  }
  ```
- **Each error layer adds context** (which step, which model, which tool) without losing the underlying cause.
- **Educational value:** Show readers how error layering works in Rust. This IS the lesson.

**Detection (warning signs):**
- `anyhow::Result` appears in the public API.
- Error messages say things like "something went wrong" without identifying where.
- Users cannot `match` on error variants to handle specific cases.

**Phase relevance:** Phase 1 (define error types alongside the model trait). Each subsequent phase adds its own error layer.

**Confidence:** HIGH -- `thiserror` for libraries, `anyhow` for applications is a well-established Rust convention.

---

### Pitfall 8: Builder Pattern That Compiles Invalid Workflows

**What goes wrong:** The builder pattern allows constructing a workflow that is invalid: steps reference non-existent dependencies, a step's output type doesn't match its dependent's expected input type, or the DAG has cycles. These errors only surface at runtime (during `execute()`), not at build time.

**Why it happens:** Rust's builder pattern typically returns `Self` from each method and validates in `build()`. For a DAG workflow builder, this means you accumulate steps and edges as data, then validate the graph structure at `build()` time. But developers often defer validation too long, or forget edge cases.

**Prevention:**
- **Validate in `build()`, not in `execute()`.** The `build()` method should return `Result<Workflow, BuildError>` and check:
  - All edges reference existing steps.
  - The DAG has no cycles (topological sort attempt).
  - There is at least one step with no dependencies (an entry point).
  - No orphan steps (steps with no path from any entry point).
- **Use typed step IDs (newtypes), not raw strings.** This prevents typos in edge definitions:
  ```rust
  let step_a = builder.add_step("summarize", ...); // Returns StepId
  let step_b = builder.add_step("translate", ...); // Returns StepId
  builder.add_edge(step_a, step_b); // Type-safe, not string-based
  ```
- **Consider compile-time validation** for simple cases (typestate builder), but don't over-engineer it. Runtime validation in `build()` is sufficient for an educational library and far more readable.
- **Produce clear error messages** from `build()` that explain what's wrong and how to fix it. This is especially important for an educational library.

**Detection (warning signs):**
- `build()` returns `Workflow` (not `Result<Workflow, _>`).
- Edges use `&str` step names with no validation.
- DAG cycle detection only happens during execution.

**Phase relevance:** Phase 3 (Workflow Builder + DAG Execution).

**Confidence:** HIGH -- standard builder pattern concerns.

---

### Pitfall 9: Data Flow Between Steps Using `Any` Downcasting

**What goes wrong:** You need steps to pass data to dependent steps, but different steps produce different types. You use `Box<dyn Any>` to erase the type, then downcast in the consuming step. This compiles but: (a) type mismatches become runtime panics, not compile errors; (b) readers cannot understand the data flow without tracing through the downcast calls; (c) adding a new step requires knowing the exact type the upstream step produces.

**Why it happens:** DAG-based data flow is inherently heterogeneous -- a "summarize" step produces text, a "classify" step produces an enum, a "fetch" step produces structured data. Rust doesn't have great options for heterogeneous typed maps.

**Prevention:**
- **For an educational LLM framework, use `serde_json::Value` as the universal data type between steps.** This is honest: LLM outputs are text/JSON anyway. Steps always receive and produce `serde_json::Value` at the boundary.
- **Let individual steps deserialize to their typed internal representation** from the `serde_json::Value` input, similar to the tool pattern.
- **This is actually more educational** than a complex type-erased system, because it mirrors how real LLM pipelines work: JSON in, JSON out, with typed processing inside each step.
- **Store step outputs in a `HashMap<StepId, serde_json::Value>`** that the executor manages. Each step receives only its declared inputs (selected from this map by the executor).
- **Avoid `Box<dyn Any>`** entirely. It adds complexity without educational value and creates a false sense of type safety.

**Detection (warning signs):**
- `downcast_ref::<ConcreteType>()` calls scattered through step implementations.
- Runtime panics with "downcast failed" messages.
- Step implementations need to know the concrete return type of other steps.

**Phase relevance:** Phase 3 (Data Flow between steps).

**Confidence:** HIGH -- `serde_json::Value` as inter-step format is the standard approach in workflow engines.

---

### Pitfall 10: Over-Abstraction That Obscures Educational Value

**What goes wrong:** You build clean, DRY abstractions with traits, generics, and indirection -- and the result is a codebase where readers cannot follow the actual execution path from "user calls workflow.execute()" to "HTTP request hits OpenAI." The educational purpose is defeated by the engineering quality.

**Why it happens:** Experienced Rust developers instinctively reach for traits, generics, and zero-cost abstractions. These are good for production libraries but actively harmful for educational code where the goal is comprehension, not extensibility.

**Consequences:** The target audience (Rust developers new to LLM patterns) cannot learn from the code because they're spending all their cognitive budget on understanding the abstraction machinery rather than the agentic AI concepts.

**Prevention:**
- **Prefer concrete types to generic ones.** `OpenAiClient` is more educational than `T: ModelProvider<Config = C, Response = R>`.
- **Prefer duplication to wrong abstraction.** If OpenAI and Gemini handlers share 70% of their code, it's better to have two readable implementations than one generic implementation with 30% conditional logic.
- **One level of indirection maximum.** A step calls a model function, which calls HTTP. Not: a step calls a dispatcher, which resolves a provider, which creates a request builder, which configures middleware, which calls HTTP.
- **Comments explain WHY, code shows HOW.** Don't abstract away the HOW.
- **Use the "explain to a colleague" test:** If you can't explain a module's execution path in under 2 minutes, it's too abstract.

**Detection (warning signs):**
- More than 2 trait bounds on a function.
- `where` clauses longer than the function body.
- Readers need to open 4+ files to trace a single API call.
- Generic parameters that are only ever instantiated with one concrete type.

**Phase relevance:** ALL PHASES. This is a project-wide discipline, not a one-time decision.

**Confidence:** HIGH -- this is a fundamental educational software design principle, and the PROJECT.md explicitly calls this out as a priority.

---

## Minor Pitfalls

Mistakes that cause annoyance, confusion, or minor rework.

---

### Pitfall 11: Forgetting `Send + Sync` Bounds on Trait Objects in Async Contexts

**What goes wrong:** You define `Box<dyn Tool>` without `Send + Sync`, then find you cannot share the tool registry across tokio tasks. Adding `Send + Sync` retroactively is a breaking change to the trait's object safety requirements if any implementors are not `Send + Sync`.

**Prevention:**
- Always define shared async traits as: `trait Tool: Send + Sync { ... }`
- For trait objects: `Box<dyn Tool + Send + Sync>`
- Add these bounds from day one. Removing them later is easy; adding them is a breaking change.

**Phase relevance:** Phase 2 (Tool System), Phase 1 (Model trait).

**Confidence:** HIGH.

---

### Pitfall 12: reqwest Client Instantiation Per Request

**What goes wrong:** Each API call creates a new `reqwest::Client`. This is expensive: it creates a new connection pool, new TLS sessions, and disables HTTP keep-alive benefits. Latency increases significantly.

**Prevention:**
- Create one `reqwest::Client` at initialization and reuse it (via `Arc<Client>` or by storing it in the model client struct). `reqwest::Client` is designed to be cloned cheaply (it uses `Arc` internally).
- This is a one-line fix but important to get right from the start.

**Phase relevance:** Phase 1 (HTTP client setup for model providers).

**Confidence:** HIGH -- documented in reqwest's own README.

---

### Pitfall 13: Blocking Operations Inside Async Context

**What goes wrong:** You call a synchronous blocking function (e.g., `std::fs::read`, a compute-heavy JSON schema validation, or a synchronous library function) inside an async context. This blocks the tokio runtime thread, starving other tasks. Symptoms: other tasks mysteriously stall, timeouts in unrelated code.

**Prevention:**
- Use `tokio::fs` instead of `std::fs` if you need file I/O.
- Use `tokio::task::spawn_blocking()` for CPU-intensive work.
- This is less likely to be an issue in an LLM framework (most work is I/O-bound HTTP calls), but be aware when implementing tool execution -- user-defined tools might do heavy computation.
- Document for library users: "If your tool does heavy computation, wrap it in `spawn_blocking`."

**Phase relevance:** Phase 2 (Tool Execution), Phase 3 (DAG Executor).

**Confidence:** HIGH -- standard tokio guidance.

---

### Pitfall 14: Hardcoded API Base URLs

**What goes wrong:** The OpenAI client has `https://api.openai.com/v1` hardcoded. Users who need to use Azure OpenAI, a local proxy, or an OpenAI-compatible API (Ollama, LiteLLM) cannot use the library without modifying the source.

**Prevention:**
- Make the base URL configurable in the client builder. Default to the standard URL, but allow override.
- This is trivial to implement but easy to forget, and it significantly impacts usability.

**Phase relevance:** Phase 1 (Model Client construction).

**Confidence:** HIGH.

---

### Pitfall 15: Leaking API Keys in Debug Output or Error Messages

**What goes wrong:** You derive `Debug` on your config struct, which contains the API key. A debug log, error message, or panic dumps the key to stdout/stderr/logs. In an educational context, learners running examples may unknowingly expose their keys.

**Prevention:**
- **Never derive `Debug` on structs containing API keys.** Implement `Debug` manually to redact the key:
  ```rust
  impl fmt::Debug for ModelConfig {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
          f.debug_struct("ModelConfig")
              .field("base_url", &self.base_url)
              .field("api_key", &"[REDACTED]")
              .finish()
      }
  }
  ```
- Use a newtype wrapper (`ApiKey(String)`) that redacts in `Debug` and `Display`.
- This is especially important for an educational project where users are learning and may not be security-conscious.

**Phase relevance:** Phase 1 (Config types).

**Confidence:** HIGH.

---

### Pitfall 16: Example Programs That Require Paid API Keys to Run

**What goes wrong:** Every example program requires a real OpenAI or Gemini API key and makes real API calls. New users who clone the repo cannot run anything without signing up for a paid service. CI cannot test examples. Development requires spending money on API calls.

**Prevention:**
- Provide at least one example that works with a mock/stub model backend.
- Consider a `MockModel` implementation that returns canned responses -- this also serves as a testing utility.
- For API-dependent examples, document the expected cost per run and provide sample output in the README so readers can understand the example without running it.
- Use environment variables for API keys with clear error messages when missing.

**Phase relevance:** Phase 4 (Examples and Documentation).

**Confidence:** HIGH.

---

### Pitfall 17: JSON Schema Generation That Doesn't Match Provider Expectations

**What goes wrong:** You generate JSON Schema for tool parameters (from Rust types via `schemars` or manually), but the generated schema doesn't match what the LLM provider expects. OpenAI expects a specific JSON Schema dialect. Gemini expects a slightly different format (e.g., it doesn't support all JSON Schema features like `$ref`). The LLM receives a schema it can't interpret correctly and generates malformed tool call arguments.

**Prevention:**
- **Test generated schemas against actual API calls.** Don't just verify they're valid JSON Schema -- verify the LLM can use them correctly.
- **Keep schemas simple for v1:** String, number, boolean, object with named fields, arrays. Avoid `oneOf`, `$ref`, `allOf` -- these are not reliably handled by all LLMs.
- **Generate schemas per-provider if needed.** The internal tool definition has a canonical schema; each provider adapter may need to adjust it for the provider's expectations.
- If using `schemars` crate for auto-generation, verify its output format against each provider's documentation. `schemars` produces JSON Schema draft-07 which may need adjustments.

**Phase relevance:** Phase 2 (Tool System).

**Confidence:** MEDIUM -- schema compatibility details may have changed since training data. Verify against current provider docs when implementing.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Model Abstraction Layer | Async trait object design (#1) | Decide enum vs. trait dispatch on day 1 |
| Model Abstraction Layer | Lifetime infection in async (#5) | Use owned types at API boundaries |
| Model Abstraction Layer | API key leakage (#15) | Use redacting newtype for API keys |
| Provider Implementations | Serde deserialization fragility (#3) | Test with real API response fixtures |
| Provider Implementations | Abstraction that doesn't abstract (#2) | Define internal message model first |
| Tool System | Type system too rigid or too dynamic (#6) | Typed definitions, `Value` dispatch pattern |
| Tool System | JSON Schema compatibility (#17) | Test schemas against actual provider calls |
| DAG Execution Engine | Deadlock/starvation (#4) | Use level-based scheduling for v1 |
| DAG Execution Engine | `Any` downcasting for data flow (#9) | Use `serde_json::Value` as inter-step format |
| Workflow Builder | Builder compiles invalid workflows (#8) | Validate in `build()`, use typed StepIds |
| Error Handling | Swallowed context (#7) | Layer errors per component with `thiserror` |
| All Phases | Over-abstraction (#10) | One-level indirection max, concrete types preferred |
| Examples/Docs | Paid API requirement (#16) | Include mock model backend |

---

## Rust-Specific Quick Reference

These are Rust-specific patterns that recur across multiple pitfalls above:

| Pattern | Do This | Not This |
|---------|---------|----------|
| Async dispatch | Enum dispatch for small variant sets | `async-trait` + `Box<dyn T>` when only 2 implementations exist |
| Data across async boundaries | `Arc<T>`, `String`, owned types | `&T` references in spawned tasks |
| Error handling in libraries | `thiserror` with layered error enums | `anyhow` (for applications only) |
| Trait object bounds | `trait Foo: Send + Sync` from day one | Adding `Send + Sync` retroactively |
| API response deserialization | `#[serde(default)]`, `Option<T>`, `#[serde(flatten)]` | `#[serde(deny_unknown_fields)]`, `#[serde(untagged)]` |
| HTTP client | One `reqwest::Client`, reused via `Arc` or `Clone` | New `Client` per request |
| Inter-step data | `serde_json::Value` at boundaries, typed internally | `Box<dyn Any>` with downcast |
| Secrets in structs | Manual `Debug` impl that redacts | `#[derive(Debug)]` on key-containing structs |

---

## Sources

- Confidence assessments are based on established Rust ecosystem patterns and well-known async Rust design guidance. WebSearch and WebFetch were unavailable during this research session.
- Pitfalls #1, #5, #11, #13: Standard async Rust guidance from tokio documentation and Rust async book.
- Pitfall #3: Serde best practices documentation.
- Pitfall #7: `thiserror` vs `anyhow` convention is widely documented in Rust error handling guides.
- Pitfall #12: reqwest documentation explicitly recommends client reuse.
- Pitfalls #2, #6, #9, #10: Derived from analysis of Rust LLM framework design patterns in crates like `rig`, `async-openai`, `llm-chain`.
- **Confidence note:** All pitfalls are rated HIGH confidence because they are based on well-established Rust patterns and common LLM framework design challenges. The one exception is Pitfall #17 (JSON Schema compatibility) which is MEDIUM -- provider-specific schema requirements change and should be verified against current API documentation during implementation.
