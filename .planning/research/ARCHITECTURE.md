# Architecture Patterns

**Domain:** Rust LLM framework library (educational, DAG-based workflows)
**Researched:** 2026-02-10
**Confidence:** MEDIUM (web tools unavailable; architecture patterns from training data, verified against Rust idioms and known framework structures)

## How Existing Rust LLM Frameworks Are Structured

### Rig (0xPlaygrounds/rig)

**Confidence:** MEDIUM (training data; could not verify current API via docs.rs)

Rig organizes around a core crate (`rig-core`) with provider crates (`rig-openai`, `rig-anthropic`, etc.) as separate crates in a workspace. Key architectural choices:

- **`CompletionModel` trait** as the central abstraction. Models implement `completion()` which takes a `CompletionRequest` and returns a `CompletionResponse`.
- **Agent** is a higher-level struct wrapping a model + system prompt + tools. It implements `Completion` trait.
- **Tool trait** with `definition()` (returns JSON schema) and `call()` (executes the tool) methods.
- **Extractors** for structured output via serde deserialization.
- **Provider pattern:** Each LLM provider is a separate crate that implements the core traits, with provider-specific HTTP client logic.

**What to learn from Rig:** The trait hierarchy is clean -- a small `CompletionModel` trait, then higher-level `Agent`/`Pipeline` types built on top. Provider crates depend on core, not the reverse.

**What NOT to copy:** Rig's pipeline system is chain-based (linear), not DAG-based. For this project, we need a different execution model.

### llm-chain

**Confidence:** MEDIUM (training data)

llm-chain uses a different approach:

- **Step** as the core unit -- a prompt template + model invocation.
- **Chain** composes steps sequentially, with output from one step feeding the next.
- **Executor** trait abstracts over different LLM backends.
- **Parameters** map for passing data between steps.
- Separate crates for each backend (`llm-chain-openai`, etc.).

**What to learn:** The step/chain metaphor is intuitive. Parameters as a typed map for inter-step communication is a pattern worth considering.

**What NOT to copy:** Sequential-only execution does not support the DAG requirement.

### Pattern Synthesis: What LLM Frameworks All Share

Looking across Rig, llm-chain, LangChain (Python), and LlamaIndex (Python), every LLM framework has these core components:

1. **Model Abstraction** -- a trait/interface that normalizes different LLM API shapes
2. **Prompt Management** -- constructing and templating prompts
3. **Tool/Function System** -- defining callable functions and marshaling to/from LLM tool-call format
4. **Orchestration Layer** -- sequencing, chaining, or graphing calls together
5. **Data Flow** -- moving outputs between steps

This project adds a DAG-based orchestrator, which is structurally more complex than a chain but architecturally cleaner (explicit dependencies vs. implicit sequential coupling).

---

## Recommended Architecture

### High-Level Component Diagram

```
                    +------------------+
                    |   User Code      |
                    |  (examples/)     |
                    +--------+---------+
                             |  uses builder API
                             v
                    +------------------+
                    |  WorkflowBuilder |  <-- Builder pattern entry point
                    +--------+---------+
                             | produces
                             v
+-------------------+------------------+-------------------+
|                   |                  |                   |
|   Workflow DAG    |   Step Types     |   Tool Registry   |
|   (nodes+edges)   |  (LlmStep,      |   (tool defs +    |
|                   |   FnStep, etc.)  |    dispatch)      |
+--------+----------+--------+---------+--------+----------+
         |                   |                  |
         v                   v                  v
+------------------+  +------------------+  +------------------+
|  DAG Executor    |  |  Model Trait     |  |  Tool Trait      |
|  (topo sort,     |  |  (completion,    |  |  (definition,    |
|   async dispatch)|  |   tool calling)  |  |   call)          |
+--------+---------+  +--------+---------+  +------------------+
         |                     |
         v                     v
+------------------+  +------------------+
|  Step Runtime    |  |  Providers       |
|  (execute step,  |  |  (OpenAI,        |
|   collect output)|  |   Gemini)        |
+------------------+  +------------------+
                              |
                              v
                      +------------------+
                      |  HTTP Client     |
                      |  (reqwest)       |
                      +------------------+
```

### Component Boundaries

| Component | Responsibility | Communicates With | Crate/Module |
|-----------|---------------|-------------------|--------------|
| **WorkflowBuilder** | Fluent API for constructing workflow DAGs. Validates structure at build time. | Produces Workflow, references Step types | `workflow::builder` |
| **Workflow** | Immutable DAG structure: nodes (steps) + edges (dependencies). The "plan." | Consumed by Executor | `workflow` |
| **Step** (enum/trait) | Represents a single unit of work: LLM call, function call, or input/output. | Executed by Executor, references Model and Tool | `step` |
| **DAG Executor** | Topological sort, tracks readiness, dispatches steps in parallel via async. The "runtime." | Reads Workflow, runs Steps, collects StepOutput | `executor` |
| **Model trait + Providers** | Abstraction over LLM APIs. Each provider implements the trait. | Called by LlmStep during execution | `model`, `providers::openai`, `providers::gemini` |
| **Tool trait + Registry** | Defining tools and dispatching tool calls from LLM responses. | Called by LlmStep when model returns tool calls | `tool` |
| **StepOutput / DataFlow** | Typed data passed between steps. Output of step A becomes input to step B. | Stored by Executor, passed to dependent Steps | `workflow::data` |

### Data Flow

```
1. User builds Workflow via WorkflowBuilder
   - Adds steps (LLM calls, functions, etc.)
   - Adds edges (step A -> step B means B depends on A)
   - Calls .build() which validates the DAG (acyclic, connected, types compatible)

2. Executor receives Workflow
   - Performs topological sort to determine execution order
   - Identifies steps with no dependencies (the "frontier")

3. Execution loop (async):
   a. Collect all "ready" steps (dependencies satisfied)
   b. Spawn each ready step as an async task (tokio::spawn or join_all)
   c. Each step executes:
      - FnStep: runs the user function with inputs from dependencies
      - LlmStep: constructs prompt from inputs, calls Model, optionally handles tool calls
   d. Step produces StepOutput
   e. Executor stores output, marks step complete
   f. Check which new steps are now ready (all deps complete)
   g. Repeat until all steps complete or error

4. Final output collected from terminal nodes
```

**Data between steps** travels as a `StepOutput` enum/type, stored in a `HashMap<StepId, StepOutput>` inside the executor. When a step runs, it receives the outputs of its dependencies.

---

## Core Trait Design

### Model Abstraction

This is the most critical trait in the library. It must normalize OpenAI and Gemini's different API shapes into one interface.

```rust
/// The core model abstraction. Each LLM provider implements this.
#[async_trait]
pub trait Model: Send + Sync {
    /// Human-readable model identifier (e.g., "gpt-4o", "gemini-1.5-pro")
    fn model_id(&self) -> &str;

    /// Send a completion request and get a response.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
}

/// A completion request, normalized across providers.
pub struct CompletionRequest {
    /// System prompt (if the model supports it)
    pub system: Option<String>,
    /// User messages / conversation history
    pub messages: Vec<Message>,
    /// Tool definitions available for this call
    pub tools: Vec<ToolDefinition>,
    /// Temperature, max_tokens, etc.
    pub parameters: ModelParameters,
}

/// A completion response, normalized across providers.
pub struct CompletionResponse {
    /// The text content of the response (if any)
    pub content: Option<String>,
    /// Tool calls the model wants to make (if any)
    pub tool_calls: Vec<ToolCall>,
    /// Usage metadata
    pub usage: Usage,
}

/// A message in the conversation.
pub enum Message {
    System(String),
    User(String),
    Assistant(String),
    ToolResult { call_id: String, content: String },
}
```

**Why this design:**
- `CompletionRequest`/`CompletionResponse` are provider-agnostic data types. The provider crate translates to/from the provider's wire format.
- `tools` as part of the request (not a separate channel) matches how both OpenAI and Gemini accept function definitions.
- `ToolCall` in the response captures the model's request to invoke a tool, which the step executor then dispatches.
- `async_trait` is used because `complete()` involves HTTP I/O. Note: as of Rust 1.75+, native async traits exist via RPITIT, but `async_trait` may still be needed for object safety (`dyn Model`). This is a decision point discussed in Pitfalls.

**Design note on `dyn Model` vs generics:**
For an educational library, prefer `Box<dyn Model>` over generic parameters like `M: Model` on workflow types. Reason: the workflow may contain steps using different models (e.g., a cheap model for classification, an expensive model for generation). Using trait objects keeps the workflow type simple. The performance cost of dynamic dispatch is negligible compared to HTTP latency.

### Tool System

```rust
/// A tool that an LLM can request to call.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The tool's name (must match what the LLM sees)
    fn name(&self) -> &str;

    /// JSON Schema definition for the tool's parameters.
    /// This is sent to the LLM so it knows the calling convention.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments (JSON).
    /// Returns the result as a string (to feed back to the LLM).
    async fn call(&self, args: serde_json::Value) -> Result<String>;
}

/// Tool definition sent to the LLM.
/// Matches the OpenAI/Gemini function-calling schema shape.
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// A tool call requested by the LLM in its response.
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Registry that holds available tools and dispatches calls.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Box<dyn Tool>) { ... }
    pub fn definitions(&self) -> Vec<ToolDefinition> { ... }
    pub async fn dispatch(&self, call: &ToolCall) -> Result<String> { ... }
}
```

**Why this design:**
- `Tool` is intentionally simple: name, schema, execute. This is the minimal interface.
- `ToolRegistry` provides lookup and dispatch -- the executor does not need to know which tools exist, it just asks the registry.
- `serde_json::Value` for arguments avoids complex generics. Tools deserialize internally. This is the pragmatic choice for an educational library.
- `async fn call()` because tools might do I/O (HTTP calls, file reads).

### Step Types

```rust
/// Unique identifier for a step in the workflow.
pub type StepId = String;

/// The output of a completed step.
#[derive(Clone, Debug)]
pub enum StepOutput {
    Text(String),
    Json(serde_json::Value),
    // Extensible for future types
}

/// Inputs provided to a step: the outputs of its dependencies.
pub type StepInputs = HashMap<StepId, StepOutput>;

/// A step in the workflow -- the unit of execution.
pub enum Step {
    /// An LLM completion step. Constructs a prompt from inputs,
    /// calls the model, optionally handles tool calls.
    Llm(LlmStep),

    /// A plain function step. Transforms data without calling an LLM.
    Fn(FnStep),
}

pub struct LlmStep {
    pub id: StepId,
    pub model: Box<dyn Model>,
    pub system_prompt: Option<String>,
    /// Builds the user prompt from dependency outputs
    pub prompt_builder: Box<dyn Fn(&StepInputs) -> String + Send + Sync>,
    /// Tools available to this step (optional)
    pub tools: Option<ToolRegistry>,
}

pub struct FnStep {
    pub id: StepId,
    /// Transforms inputs to output
    pub handler: Box<dyn Fn(StepInputs) -> Result<StepOutput> + Send + Sync>,
}
```

**Why this design:**
- `Step` as an enum rather than a trait keeps things simple and avoids trait object nesting. For an educational codebase, enum dispatch is easier to follow than dynamic dispatch.
- `prompt_builder` as a closure lets users define how dependency outputs become the prompt. This is the "data flow" mechanism.
- `LlmStep` owns its `Model` and optional `ToolRegistry`. This makes each step self-contained.
- `FnStep` enables data transformation without LLM calls (e.g., merging outputs, formatting).

### Workflow and Builder

```rust
/// An immutable workflow: a DAG of steps.
pub struct Workflow {
    pub steps: HashMap<StepId, Step>,
    pub edges: Vec<(StepId, StepId)>,  // (from, to) -- "from" must complete before "to"
}

/// Builder for constructing workflows fluently.
pub struct WorkflowBuilder {
    steps: HashMap<StepId, Step>,
    edges: Vec<(StepId, StepId)>,
}

impl WorkflowBuilder {
    pub fn new() -> Self { ... }

    /// Add an LLM step.
    pub fn add_llm_step(mut self, step: LlmStep) -> Self { ... }

    /// Add a function step.
    pub fn add_fn_step(mut self, step: FnStep) -> Self { ... }

    /// Declare a dependency: `from` must complete before `to` runs.
    pub fn add_edge(mut self, from: &str, to: &str) -> Self { ... }

    /// Validate and produce the Workflow.
    /// Fails if: cycle detected, unknown step references, disconnected nodes.
    pub fn build(self) -> Result<Workflow> { ... }
}
```

**Build-time validation matters.** The `build()` method should:
1. Check for cycles (Kahn's algorithm or DFS-based)
2. Verify all edge references point to existing steps
3. Optionally warn about disconnected components

### DAG Executor

```rust
/// Executes a workflow by running steps in dependency order.
pub struct Executor;

impl Executor {
    /// Run the workflow to completion. Returns outputs of all steps.
    pub async fn run(workflow: &Workflow) -> Result<HashMap<StepId, StepOutput>> {
        // 1. Compute in-degree for each step
        // 2. Initialize frontier: steps with in-degree 0
        // 3. Loop:
        //    a. For each step in frontier, spawn async execution
        //    b. Collect results (join_all or FuturesUnordered)
        //    c. Store outputs
        //    d. Decrease in-degree of dependents
        //    e. Add newly-ready steps (in-degree 0) to frontier
        //    f. Repeat until frontier empty
        // 4. Return all outputs
    }
}
```

**Key async pattern -- parallel step execution:**

```rust
use futures::stream::{FuturesUnordered, StreamExt};

// Inside the execution loop:
let mut in_progress = FuturesUnordered::new();

for step_id in ready_steps {
    let inputs = gather_inputs(&step_id, &edges, &outputs);
    let step = &workflow.steps[&step_id];
    in_progress.push(async move {
        let output = execute_step(step, inputs).await?;
        Ok::<_, Error>((step_id, output))
    });
}

while let Some(result) = in_progress.next().await {
    let (step_id, output) = result?;
    outputs.insert(step_id.clone(), output);
    // Update frontier with newly-ready steps
}
```

**Why `FuturesUnordered`:** It allows processing results as they complete (not in insertion order), which is ideal for a DAG where independent branches complete at different times. This is more efficient than `join_all` because we can update the frontier incrementally.

**Ownership note:** The executor borrows the workflow immutably and owns the outputs map. Steps themselves need `Send + Sync` because they execute across async task boundaries.

---

## Patterns to Follow

### Pattern 1: Provider Translation Layer

**What:** Each provider crate translates between the normalized `CompletionRequest`/`CompletionResponse` and the provider's wire format (HTTP JSON).

**Why:** OpenAI and Gemini have meaningfully different JSON schemas for tool calling. OpenAI uses `functions`/`tool_choice`, Gemini uses `functionDeclarations` within `tools`. The translation happens in one place per provider.

**Example structure:**

```rust
// providers/openai.rs
pub struct OpenAiModel {
    client: reqwest::Client,
    api_key: String,
    model_name: String, // e.g., "gpt-4o"
}

#[async_trait]
impl Model for OpenAiModel {
    fn model_id(&self) -> &str { &self.model_name }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // 1. Translate CompletionRequest -> OpenAI JSON body
        let body = self.build_request_body(&request);
        // 2. POST to https://api.openai.com/v1/chat/completions
        let response = self.client.post(OPENAI_URL).json(&body).send().await?;
        // 3. Parse response JSON
        let raw: OpenAiResponse = response.json().await?;
        // 4. Translate OpenAiResponse -> CompletionResponse
        Ok(self.parse_response(raw))
    }
}
```

### Pattern 2: Single-Shot Tool Calling Flow

**What:** When an LlmStep has tools, the execution follows a strict two-phase flow: (1) LLM call with tool definitions, (2) if tool calls returned, dispatch them and return results. No loop.

**Why:** The project explicitly scopes to single-shot tool calling (no agentic loops). This simplifies the mental model and the code.

```rust
async fn execute_llm_step(step: &LlmStep, inputs: StepInputs) -> Result<StepOutput> {
    // Phase 1: Build and send request
    let prompt = (step.prompt_builder)(&inputs);
    let mut request = CompletionRequest {
        system: step.system_prompt.clone(),
        messages: vec![Message::User(prompt)],
        tools: step.tools.as_ref()
            .map(|t| t.definitions())
            .unwrap_or_default(),
        parameters: ModelParameters::default(),
    };

    let response = step.model.complete(request.clone()).await?;

    // Phase 2: If tool calls, dispatch them and make one more LLM call
    if !response.tool_calls.is_empty() {
        let registry = step.tools.as_ref()
            .ok_or_else(|| anyhow!("Tool calls without registry"))?;

        // Dispatch each tool call
        let mut tool_results = Vec::new();
        for call in &response.tool_calls {
            let result = registry.dispatch(call).await?;
            tool_results.push(Message::ToolResult {
                call_id: call.id.clone(),
                content: result,
            });
        }

        // Add assistant message + tool results to conversation
        request.messages.push(Message::Assistant(
            response.content.unwrap_or_default()
        ));
        request.messages.extend(tool_results);
        request.tools = vec![]; // No tools on second call

        // Second (final) LLM call to synthesize tool results
        let final_response = step.model.complete(request).await?;
        Ok(StepOutput::Text(final_response.content.unwrap_or_default()))
    } else {
        Ok(StepOutput::Text(response.content.unwrap_or_default()))
    }
}
```

### Pattern 3: Builder Pattern with Chained Methods

**What:** Use consuming `self` (not `&mut self`) in builder methods so the API chains naturally.

**Why:** Idiomatic Rust builder pattern. Consuming self prevents using a partially-built workflow.

```rust
let workflow = WorkflowBuilder::new()
    .add_llm_step(classify_step)
    .add_llm_step(summarize_step)
    .add_fn_step(merge_step)
    .add_edge("classify", "merge")
    .add_edge("summarize", "merge")
    .build()?;
```

### Pattern 4: Error Propagation, Not Error Swallowing

**What:** Use `Result<T>` throughout. Steps that fail propagate errors to the executor. The executor can choose to fail-fast (stop all steps) or collect errors.

**Why:** Educational code should show idiomatic Rust error handling. Using `anyhow::Result` in the library internals is acceptable for an educational crate; for a "real" library you would use typed errors.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: God Trait

**What:** Putting everything (completion, tool calling, embeddings, streaming) into a single `Model` trait.
**Why bad:** Forces every provider to implement features they may not support. Makes the trait hard to understand.
**Instead:** Keep `Model` focused on completion. If embeddings are added later, use a separate `EmbeddingModel` trait. Traits should be small and cohesive.

### Anti-Pattern 2: Stringly-Typed Everything

**What:** Using raw strings for step IDs, tool names, data keys without any validation.
**Why bad:** Typos cause runtime failures that are hard to debug. "summarzie" vs "summarize" becomes a silent failure.
**Instead:** Validate at build time. The `WorkflowBuilder::build()` method should catch unknown step references. Consider a `StepId` newtype for type safety (even if it wraps String).

### Anti-Pattern 3: Implicit Data Flow

**What:** Steps reading from shared mutable state or global context instead of explicit inputs.
**Why bad:** Makes data dependencies invisible. Defeats the purpose of a DAG (where edges ARE the data flow).
**Instead:** Steps receive only the outputs of their declared dependencies via `StepInputs`. The executor enforces this.

### Anti-Pattern 4: Over-Abstracting for Extensibility

**What:** Creating traits, trait objects, and generic parameters for components that only have one implementation.
**Why bad:** For an educational library, unnecessary abstraction obscures the actual logic. A reader should be able to follow `Executor::run()` without jumping through five layers of indirection.
**Instead:** Use concrete types where there is only one implementation. Extract a trait only when there are (or will be) multiple implementations. The `Model` trait is justified (multiple providers). A `WorkflowExecutor` trait is NOT justified (there is one executor).

### Anti-Pattern 5: Async Trait Without `Send` Bounds

**What:** Defining `async fn` in traits without ensuring futures are `Send`.
**Why bad:** The DAG executor spawns steps as concurrent tasks. If the futures are not `Send`, they cannot cross task boundaries, and parallel execution breaks.
**Instead:** Use `#[async_trait]` (which adds `Send` by default) or manually bound with `-> impl Future<Output = ...> + Send`.

---

## Module / Crate Layout

### Recommendation: Single Crate with Feature Flags

For an educational library, a single crate is better than a workspace with multiple crates.

**Why single crate:**
- Easier to navigate for learners (one `src/` to explore)
- No inter-crate dependency management
- Feature flags (`openai`, `gemini`) still allow conditional compilation
- Simpler CI and publishing

**Why NOT a workspace:**
- Workspaces add cognitive overhead for learners
- Inter-crate trait boundaries require careful pub/pub(crate) management
- For two providers, the overhead is not worth it

### Proposed Layout

```
agentic-framework/
  Cargo.toml
  src/
    lib.rs              # Public API re-exports, crate-level docs
    model.rs            # Model trait, CompletionRequest/Response, Message types
    tool.rs             # Tool trait, ToolDefinition, ToolCall, ToolRegistry
    step.rs             # Step enum, LlmStep, FnStep, StepOutput, StepInputs
    workflow/
      mod.rs            # Workflow struct, re-exports
      builder.rs        # WorkflowBuilder
      dag.rs            # Topological sort, cycle detection, adjacency list utilities
    executor.rs         # Executor::run(), step dispatch, async coordination
    providers/
      mod.rs            # Provider re-exports, conditional compilation
      openai.rs         # OpenAiModel implementation
      gemini.rs         # GeminiModel implementation
    error.rs            # Error types (or use anyhow for simplicity)
  examples/
    simple_chain.rs     # Linear A -> B -> C workflow
    parallel_branch.rs  # Diamond DAG: A -> [B, C] -> D
    tool_calling.rs     # LLM step with tools
    multi_model.rs      # Different models for different steps
  tests/
    workflow_tests.rs   # DAG validation, builder tests
    executor_tests.rs   # Execution order, parallel behavior
    mock_model.rs       # Mock Model impl for testing without API calls
```

### Module Dependency Direction

```
lib.rs (root)
  |
  +-- model.rs         (depends on: nothing internal, only serde/async_trait)
  +-- tool.rs          (depends on: nothing internal, only serde/async_trait)
  +-- error.rs         (depends on: nothing)
  +-- step.rs          (depends on: model, tool)
  +-- workflow/
  |     +-- dag.rs     (depends on: step -- just StepId)
  |     +-- builder.rs (depends on: step, workflow, dag)
  |     +-- mod.rs     (depends on: step)
  +-- executor.rs      (depends on: workflow, step, model, tool)
  +-- providers/
        +-- openai.rs  (depends on: model)
        +-- gemini.rs  (depends on: model)
```

**Key invariant:** `model.rs` and `tool.rs` are leaf modules. They depend on nothing internal. Everything else depends downward toward them. This keeps the core abstractions stable.

---

## DAG Executor: Detailed Design

### Topological Sort

Use Kahn's algorithm (BFS-based) rather than DFS-based topological sort because:
1. It naturally detects cycles (if the result has fewer nodes than the graph, there is a cycle)
2. It identifies the "frontier" at each level, which maps directly to parallel execution waves
3. It is iterative (easier to understand than recursive DFS for educational purposes)

```rust
/// Kahn's algorithm: returns execution waves (groups of parallelizable steps).
fn topological_waves(
    steps: &HashMap<StepId, Step>,
    edges: &[(StepId, StepId)],
) -> Result<Vec<Vec<StepId>>> {
    let mut in_degree: HashMap<&StepId, usize> = steps.keys().map(|id| (id, 0)).collect();
    let mut dependents: HashMap<&StepId, Vec<&StepId>> = HashMap::new();

    for (from, to) in edges {
        *in_degree.get_mut(to).unwrap() += 1;
        dependents.entry(from).or_default().push(to);
    }

    let mut waves = Vec::new();
    let mut queue: Vec<&StepId> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut processed = 0;
    while !queue.is_empty() {
        waves.push(queue.iter().map(|id| (*id).clone()).collect());
        let mut next_queue = Vec::new();
        for id in &queue {
            processed += 1;
            if let Some(deps) = dependents.get(id) {
                for dep in deps {
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_queue.push(*dep);
                    }
                }
            }
        }
        queue = next_queue;
    }

    if processed != steps.len() {
        return Err(anyhow!("Cycle detected in workflow DAG"));
    }

    Ok(waves)
}
```

### Async Execution Strategy

Two viable approaches:

**Option A: Wave-based (recommended for educational clarity)**

Execute each "wave" (set of parallelizable steps) fully before moving to the next wave. Simpler to reason about.

```rust
for wave in waves {
    let mut futures = Vec::new();
    for step_id in wave {
        let inputs = gather_inputs(&step_id, &edges, &outputs);
        let step = &workflow.steps[&step_id];
        futures.push(async move {
            execute_step(step, inputs).await.map(|out| (step_id, out))
        });
    }
    let results = futures::future::join_all(futures).await;
    for result in results {
        let (id, output) = result?;
        outputs.insert(id, output);
    }
}
```

**Option B: Eager with FuturesUnordered (better parallelism)**

Start steps as soon as dependencies are met, not waiting for the whole wave to finish. Better throughput when steps in a wave have different durations.

```rust
// (pseudocode -- real implementation needs careful ownership)
let mut in_degree = compute_in_degrees(&workflow);
let mut in_progress = FuturesUnordered::new();
let mut outputs = HashMap::new();

// Seed with root steps
for (id, step) in &workflow.steps {
    if in_degree[id] == 0 {
        in_progress.push(spawn_step(id, step, &outputs));
    }
}

while let Some(result) = in_progress.next().await {
    let (id, output) = result?;
    outputs.insert(id.clone(), output);
    // Check dependents
    for dependent in dependents_of(&id) {
        in_degree[dependent] -= 1;
        if in_degree[dependent] == 0 {
            in_progress.push(spawn_step(dependent, &workflow.steps[dependent], &outputs));
        }
    }
}
```

**Recommendation:** Start with wave-based (Option A) for the initial implementation because it is dramatically easier to understand, debug, and explain. Add eager execution later if desired. Document both approaches in examples.

---

## Suggested Build Order

Build order follows the module dependency graph (leaves first, composition last).

### Phase 1: Core Types (Foundation)

Build the types that everything else depends on.

1. `error.rs` -- define error types or decide on `anyhow`
2. `model.rs` -- `Model` trait, `CompletionRequest`, `CompletionResponse`, `Message`, `ModelParameters`, `Usage`
3. `tool.rs` -- `Tool` trait, `ToolDefinition`, `ToolCall`, `ToolRegistry`

**Rationale:** These are leaf modules with no internal dependencies. They define the vocabulary of the entire system. Getting these right early avoids rework later.

### Phase 2: Steps and Workflow Structure

4. `step.rs` -- `Step` enum, `LlmStep`, `FnStep`, `StepOutput`, `StepInputs`
5. `workflow/dag.rs` -- topological sort, cycle detection
6. `workflow/mod.rs` -- `Workflow` struct
7. `workflow/builder.rs` -- `WorkflowBuilder`

**Rationale:** Depends on Phase 1 types. Can be fully built and tested with mock data (no real LLM calls needed).

### Phase 3: Execution Engine

8. `executor.rs` -- `Executor::run()`, step dispatch, async coordination

**Rationale:** Depends on Phase 1 + Phase 2. Can be tested with mock models and function steps. This is the most complex component.

### Phase 4: Providers

9. `providers/openai.rs` -- `OpenAiModel` implementing `Model`
10. `providers/gemini.rs` -- `GeminiModel` implementing `Model`

**Rationale:** Depends only on `model.rs` types. Can be built independently of the executor. Requires API keys for integration testing but can use recorded responses for unit tests.

### Phase 5: Integration and Examples

11. `lib.rs` -- public API surface, re-exports
12. `examples/` -- working programs demonstrating all features

**Rationale:** Everything must be built before integration makes sense. Examples serve as both documentation and integration tests.

### Build Order Dependency Graph

```
Phase 1: error -> model -> tool    (independent, parallel-safe)
              \      |      /
Phase 2:      step ----+
                |       |
              dag  workflow
                |       |
              builder --+
                    |
Phase 3:       executor
                    |
Phase 4:    openai, gemini          (independent, parallel-safe)
                    |
Phase 5:     lib.rs, examples/
```

---

## Key Design Decisions

### Decision 1: `Box<dyn Model>` vs Generics

**Recommendation:** `Box<dyn Model>` for step-level model ownership.

**Rationale:** A workflow may use different models for different steps. With generics, the `Workflow` type would need to be parameterized over all model types (or use `impl Model` which erases differently). `Box<dyn Model>` keeps types simple at negligible runtime cost (one vtable lookup per HTTP call is irrelevant).

### Decision 2: `anyhow::Result` vs Custom Error Types

**Recommendation:** `anyhow::Result` for the initial implementation.

**Rationale:** Custom error enums are good practice for production libraries but add boilerplate that obscures the core logic. For an educational library, `anyhow` keeps error handling visible but concise. A future phase could add typed errors.

### Decision 3: `async_trait` vs Native Async Traits

**Recommendation:** Use `async_trait` crate.

**Rationale:** Rust 1.75+ supports async fn in traits natively (RPITIT), but those async fns are not object-safe -- you cannot write `dyn Model` with a native async fn method. Since we need `Box<dyn Model>`, we need `#[async_trait]` which boxes the future. This is the pragmatic choice. Document the tradeoff for learners.

**Note (MEDIUM confidence):** Rust may have improved async trait object safety since my training cutoff. Verify whether `dyn`-compatible async traits are available in the current stable Rust.

### Decision 4: `StepOutput` as Enum vs Generic

**Recommendation:** `StepOutput` as an enum (`Text(String)`, `Json(Value)`).

**Rationale:** A generic `StepOutput<T>` would require the entire workflow to be parameterized over output types of every step, creating an unreadable type signature. An enum with common variants keeps things simple. Steps that need structured data use `Json(Value)` and deserialize internally.

### Decision 5: Sync vs Async Tool Execution

**Recommendation:** `async fn call()` on the `Tool` trait, even for tools that do not do I/O.

**Rationale:** Some tools will do I/O (web requests, file access). Making the trait async from the start avoids a breaking change later. Sync tools just return immediately within the async fn.

---

## Educational Considerations

### Readability Over Cleverness

- Prefer explicit match arms over trait-object chains
- Use named intermediate variables instead of long method chains
- Add comments explaining WHY, not WHAT (the code shows WHAT)
- Keep functions short enough to read on one screen

### Progressive Complexity in Examples

```
examples/
  01_simple_chain.rs       -- Linear: A -> B -> C (simplest DAG)
  02_parallel_branches.rs  -- Diamond: A -> [B, C] -> D
  03_tool_calling.rs       -- LLM step that uses tools
  04_multi_model.rs        -- Different models for different steps
  05_complex_workflow.rs   -- Real-world-ish example combining all features
```

### Testing Strategy

- **Mock Model:** A `MockModel` that returns predefined responses. Essential for testing without API keys.
- **DAG tests:** Verify topological sort, cycle detection, parallel execution order.
- **Integration tests:** Use real API calls (behind a feature flag or env var) for CI validation.

---

## Sources and Confidence

| Claim | Confidence | Basis |
|-------|------------|-------|
| Rig uses CompletionModel trait as core abstraction | MEDIUM | Training data; could not verify with docs.rs |
| Rig uses workspace with rig-core + provider crates | MEDIUM | Training data |
| llm-chain uses Step/Chain/Executor pattern | MEDIUM | Training data |
| Kahn's algorithm for topological sort | HIGH | Well-established algorithm, not library-dependent |
| FuturesUnordered for async DAG execution | HIGH | Standard Rust async pattern, documented in futures crate |
| async_trait needed for dyn-compatible async traits | HIGH | Well-known Rust limitation, though evolving |
| Builder pattern with consuming self is idiomatic Rust | HIGH | Standard Rust pattern |
| OpenAI and Gemini have different tool-calling schemas | HIGH | Well-documented API differences |
| Rust 1.75+ has native async fn in traits | HIGH | Announced in Rust release notes |
| Native async traits are not dyn-compatible | MEDIUM | True as of training data; may have changed |

**Overall confidence for architecture recommendations: MEDIUM-HIGH.** The architectural patterns (DAG execution, trait abstraction, module layout) are well-established and framework-independent. The specific API details of Rig and llm-chain could not be verified against current docs and should be treated as approximate.
