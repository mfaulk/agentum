# Phase 5: Builder API - Research

**Researched:** 2026-02-10
**Domain:** Rust builder pattern for DAG workflow construction with build-time validation
**Confidence:** HIGH

## Summary

Phase 5 wraps the existing `Workflow::new(Vec<(String, Step)>, Vec<(String, String)>)` interface with a fluent builder that makes workflow construction readable and catches structural errors at build time. The domain is pure Rust API design -- no new external dependencies are needed. The builder accumulates steps and edges via chainable methods, then `build()` performs all validation (cycles, missing dependencies, disconnected steps) and delegates to the existing `Workflow::new` internals (or equivalent logic) to produce a validated `Workflow`.

The key design decisions are: (1) use **consuming `self`** (move semantics) for builder methods since `Step` contains non-Clone types (`Box<dyn Model>`, boxed closures), making `&mut self` with a terminal `build(self)` awkward; (2) `build()` should **collect all errors** rather than fail-fast, so developers see every structural problem at once; (3) provide **typed helper methods** (`.llm_step()`, `.transform_step()`) alongside a raw `.step()` method for ergonomics; (4) add **disconnected step detection** as required by the user's locked decision; (5) keep `Workflow::new()` as an internal/advanced API while `Workflow::builder()` becomes the recommended public entry point.

**Primary recommendation:** Implement a `WorkflowBuilder` struct returned by `Workflow::builder()`. Builder methods consume and return `Self` (move semantics). Provide `.step()` for raw Step values, plus `.llm_step()` and `.transform_step()` convenience methods. Provide `.edge()` for explicit edges and `.chain()` for linear sequences. `build()` validates everything (cycles, missing deps, disconnected steps, empty workflow) and returns `Result<Workflow, BuilderError>` where `BuilderError` collects multiple validation errors.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Disconnected step detection
- `build()` must flag steps that have no edges to any other step (no incoming or outgoing edges in a multi-step workflow)
- User explicitly wants this as a warning or error, not silently accepted
- Single-step workflows are exempt (a lone step is inherently disconnected)

### Claude's Discretion

The user deferred all other design decisions. Claude has full flexibility on:

**Step addition style:**
- Whether to provide typed helper methods (`.llm_step()`, `.transform_step()`) or accept raw `Step` values
- Whether step names are always explicit or can be auto-generated
- Method chaining style (move `Self` vs `&mut Self`)

**Entry point:**
- Whether `Workflow::builder()` coexists with `Workflow::new()` or replaces it

**Edge/dependency syntax:**
- Whether edges are added via `.edge("A", "B")` or inline `.depends_on("A")` on steps
- Whether forward references (edge before step) are allowed
- Whether convenience methods like `.chain()` for linear sequences are provided
- Whether steps with no edges are implicitly roots

**Build error reporting:**
- Whether `build()` collects all errors or fails fast
- Whether to reuse existing `Error` enum or create a dedicated builder error type
- Whether empty workflows are rejected

**Step construction helpers:**
- Whether tools are attached at step creation or separately
- Whether per-step system prompts or `ModelOptions` are supported in the builder
- Level of sugar vs raw `Step` construction

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope.
</user_constraints>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| petgraph | 0.8 (already in Cargo.toml) | Graph operations for disconnected-node detection | Already a dependency; `externals()` and `neighbors_directed()` provide the building blocks for disconnected step detection |
| (no new deps) | -- | Builder pattern is pure Rust API design | No crates needed; hand-rolled builder is standard for types containing non-Clone trait objects |

### Supporting

No supporting libraries needed. The builder pattern is implemented entirely with standard Rust language features.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled builder | `derive_builder` crate | derive_builder generates boilerplate via proc macro, but cannot handle non-Clone types like `Box<dyn Model>` or boxed closures. It also cannot express custom validation logic (cycle detection, disconnected steps). Hand-rolling is the only viable option here. |
| Hand-rolled builder | `typed-builder` crate | typed-builder uses typestate to enforce required fields at compile time, but is designed for simple structs with Clone-able fields. Not suitable for accumulating variable-length collections of steps and edges. |
| Hand-rolled builder | `bon` crate | bon generates builders via derive macro. Same limitations as derive_builder for non-Clone types with complex validation. |

**Key insight:** Builder-derive crates are designed for simple struct construction, not for accumulating collections of non-Clone types with complex graph validation at build time. A hand-rolled builder is the correct approach.

## Architecture Patterns

### Recommended Project Structure

```
src/
├── workflow/
│   ├── mod.rs          # Add `pub mod builder;`, re-export WorkflowBuilder
│   ├── builder.rs      # NEW: WorkflowBuilder struct and all builder methods
│   ├── step.rs         # Unchanged
│   ├── workflow.rs     # Add `Workflow::builder()` associated function
│   └── executor.rs     # Unchanged
├── error.rs            # Add BuilderError type (or extend Error enum)
└── lib.rs              # Re-export WorkflowBuilder if desired
```

A single new file (`builder.rs`) contains all builder logic. This keeps the existing `workflow.rs` and `executor.rs` unchanged except for the `Workflow::builder()` entry point.

### Pattern 1: Consuming Self Builder (Move Semantics)

**What:** Builder methods take `self` by value and return `Self`, enabling method chaining. The `build()` method consumes the builder.

**When to use:** When the type being built contains non-Clone types (like `Box<dyn Model>`, `Box<dyn Fn(...)>`). Since `Step` is not `Clone`, the builder cannot use `&mut self` for setter methods and then `self` for `build()` -- that would require the caller to explicitly `drop` or not reference the builder after calling non-consuming setters, which is awkward. Move semantics keep the API clean.

**Example:**
```rust
pub struct WorkflowBuilder {
    steps: Vec<(String, Step)>,
    edges: Vec<(String, String)>,
}

impl WorkflowBuilder {
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a step with a given name.
    pub fn step(mut self, name: impl Into<String>, step: Step) -> Self {
        self.steps.push((name.into(), step));
        self
    }

    /// Add a dependency edge: `from` must complete before `to`.
    pub fn edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.edges.push((from.into(), to.into()));
        self
    }

    /// Build the workflow, validating the graph.
    pub fn build(self) -> Result<Workflow, Vec<BuilderError>> {
        // ... validation logic ...
    }
}
```

**Why move semantics over &mut self:**

The codebase's `Step` enum contains `Box<dyn Model>` and `Box<dyn Fn(&StepInput) -> ... >` -- neither is `Clone`. When a user calls `.step("name", some_step)`, the step is moved into the builder. If builder methods took `&mut self`, the user would need to do:

```rust
// With &mut self -- awkward split across statements:
let mut builder = Workflow::builder();
builder.step("A", step_a);  // moves step_a into builder
builder.step("B", step_b);  // moves step_b into builder
builder.edge("A", "B");
let wf = builder.build()?;  // build() would need to consume builder anyway
```

With consuming self, the same code chains naturally:

```rust
// With consuming self -- fluent and natural:
let wf = Workflow::builder()
    .step("A", step_a)
    .step("B", step_b)
    .edge("A", "B")
    .build()?;
```

Both work, but consuming self reads more naturally and is consistent with the readability goal ("a Rust developer can understand the workflow structure from the builder calls alone").

**Important note:** `ModelOptions` in the existing codebase already uses consuming self (`pub fn with_temperature(mut self, ...) -> Self`), so this is consistent with the project's established pattern.

### Pattern 2: Typed Step Helper Methods

**What:** Convenience methods that construct `Step::Llm` and `Step::Transform` variants inline, reducing boilerplate.

**Example:**
```rust
impl WorkflowBuilder {
    /// Add a transform step.
    pub fn transform_step(
        self,
        name: impl Into<String>,
        transform: impl Fn(&StepInput) -> Result<Value> + Send + Sync + 'static,
    ) -> Self {
        self.step(name, Step::Transform {
            transform: Box::new(transform),
        })
    }

    /// Add an LLM step.
    pub fn llm_step(
        self,
        name: impl Into<String>,
        model: Box<dyn Model>,
        prompt_builder: impl Fn(&StepInput) -> String + Send + Sync + 'static,
    ) -> Self {
        self.step(name, Step::Llm {
            model,
            prompt_builder: Box::new(prompt_builder),
            tools: None,
            options: ModelOptions::default(),
        })
    }

    /// Add an LLM step with tools and options.
    pub fn llm_step_with_tools(
        self,
        name: impl Into<String>,
        model: Box<dyn Model>,
        prompt_builder: impl Fn(&StepInput) -> String + Send + Sync + 'static,
        tools: ToolRegistry,
        options: ModelOptions,
    ) -> Self {
        self.step(name, Step::Llm {
            model,
            prompt_builder: Box::new(prompt_builder),
            tools: Some(tools),
            options,
        })
    }
}
```

**Rationale:** The raw `Step::Llm { ... }` construction requires the user to box closures and wrap tools in `Some/None` manually. Typed helpers hide this boilerplate. The raw `.step()` method remains available for advanced use cases.

### Pattern 3: Chain Convenience Method

**What:** A `.chain()` method that connects a sequence of step names with edges automatically.

**Example:**
```rust
impl WorkflowBuilder {
    /// Connect steps in a linear chain: A -> B -> C -> ...
    pub fn chain(mut self, steps: &[&str]) -> Self {
        for window in steps.windows(2) {
            self.edges.push((window[0].to_string(), window[1].to_string()));
        }
        self
    }
}
```

**When to use:** Linear workflows (the most common case). Instead of:
```rust
.edge("format_input", "generate")
.edge("generate", "format_output")
```

Write:
```rust
.chain(&["format_input", "generate", "format_output"])
```

### Pattern 4: Build-Time Error Collection

**What:** The `build()` method runs all validations and collects every error, rather than stopping at the first one.

**Example:**
```rust
/// Errors that can occur during workflow construction.
#[derive(Debug)]
pub enum BuilderError {
    /// A step name was used more than once.
    DuplicateStep(String),
    /// An edge references a step that was not added.
    MissingStep { edge_endpoint: String },
    /// The workflow graph contains a cycle.
    CycleDetected(String),
    /// A step has no connections to any other step (in a multi-step workflow).
    DisconnectedStep(String),
    /// No steps were added to the builder.
    EmptyWorkflow,
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuilderError::DuplicateStep(name) => write!(f, "duplicate step name: {name}"),
            BuilderError::MissingStep { edge_endpoint } => {
                write!(f, "edge references undefined step: {edge_endpoint}")
            }
            BuilderError::CycleDetected(step) => {
                write!(f, "cycle detected involving step: {step}")
            }
            BuilderError::DisconnectedStep(name) => {
                write!(f, "step '{name}' has no edges to any other step")
            }
            BuilderError::EmptyWorkflow => write!(f, "workflow has no steps"),
        }
    }
}

impl std::error::Error for BuilderError {}
```

**Rationale:** Collecting all errors is better for developer experience. If a workflow has 3 problems, showing all 3 at once lets the developer fix them in one pass rather than playing whack-a-mole with one error at a time.

**Integration with existing Error enum:** Rather than adding builder-specific variants to the main `Error` enum (which is used at runtime too), define a dedicated `BuilderError` enum. The `build()` method returns `Result<Workflow, Vec<BuilderError>>`. If the caller wants to convert to the main `Error` type, provide a `From` impl or helper:

```rust
impl From<Vec<BuilderError>> for Error {
    fn from(errors: Vec<BuilderError>) -> Self {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        Error::InvalidWorkflow(messages.join("; "))
    }
}
```

### Pattern 5: Disconnected Step Detection

**What:** After building the graph, check that every step in a multi-step workflow has at least one edge (incoming or outgoing). This is the user's locked requirement.

**Implementation approach using petgraph:**

```rust
fn find_disconnected_steps(
    graph: &DiGraph<String, ()>,
) -> Vec<String> {
    if graph.node_count() <= 1 {
        return Vec::new(); // Single-step workflows are exempt
    }

    graph
        .node_indices()
        .filter(|&idx| {
            // A node is disconnected if it has zero incoming AND zero outgoing edges
            graph.neighbors_directed(idx, Direction::Incoming).count() == 0
                && graph.neighbors_directed(idx, Direction::Outgoing).count() == 0
        })
        .map(|idx| graph[idx].clone())
        .collect()
}
```

**Note on `externals()`:** petgraph's `externals(Direction::Incoming)` returns nodes with no incoming edges (source nodes), and `externals(Direction::Outgoing)` returns nodes with no outgoing edges (sink nodes). Source and sink nodes are normal in a DAG (root steps have no incoming, terminal steps have no outgoing). We specifically want nodes with **neither** incoming **nor** outgoing edges -- truly isolated nodes. The manual check using `neighbors_directed` in both directions is the correct approach.

### Anti-Patterns to Avoid

- **Using `&mut self` for setters with `self` for `build()`:** This creates an awkward API where the builder cannot be used in a single chained expression if `build()` consumes self but setters don't. Since `Step` is not Clone, the builder cannot be borrowed mutably and then moved -- pick one style consistently. Use consuming self throughout.

- **Auto-generating step names:** Step names appear in edge definitions and in downstream code that reads workflow outputs. Auto-generated names (like "step_1", "step_2") make the builder calls harder to read and defeat the success criterion that "a Rust developer can understand the workflow structure from the builder calls alone." Always require explicit names.

- **Forward-reference edges (edge before step):** Allowing `.edge("A", "B")` before `.step("A", ...)` is added means validation must be deferred entirely to `build()`. This is acceptable and simpler than tracking order -- the builder just accumulates everything and `build()` validates the complete picture.

- **Making `Workflow::new()` private:** Some users or tests may prefer the raw API. Keep `Workflow::new()` public (it's already in use in Phase 4 tests). The builder is the recommended API, not the only API.

- **Putting builder in workflow.rs:** The builder is a distinct concern (construction API) from the workflow itself (graph structure and accessors). A separate `builder.rs` file keeps responsibilities clear and avoids growing `workflow.rs` to an unwieldy size.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cycle detection in builder | Custom DFS cycle detection | `petgraph::algo::toposort` (same as `Workflow::new`) | Already proven in Phase 4; returns `Err(Cycle)` with the node involved |
| Disconnected node detection | Custom graph traversal | `graph.neighbors_directed(idx, Incoming).count() == 0 && graph.neighbors_directed(idx, Outgoing).count() == 0` | petgraph's neighbor iteration is O(edges-of-node), efficient and correct |
| Builder derive macro | proc macro for generating builder methods | Hand-rolled builder | Step contains non-Clone Box<dyn Model> and closures; no derive macro handles this correctly with custom validation |

**Key insight:** The builder's validation logic reuses the same petgraph operations already used by `Workflow::new`. The only truly new validation is disconnected step detection, which is straightforward with petgraph's `neighbors_directed()`.

## Common Pitfalls

### Pitfall 1: Duplicating Validation Logic Between Builder and Workflow::new

**What goes wrong:** The builder implements its own cycle detection, missing-dep check, etc., duplicating what `Workflow::new` already does.
**Why it happens:** Not realizing the builder can delegate to existing validation.
**How to avoid:** The builder should either (a) call `Workflow::new` internally after its own pre-validation (disconnected steps, empty workflow) and translate errors, or (b) extract the shared validation into a common function. Option (a) is simpler -- the builder does its unique checks (disconnected steps, empty workflow, collect-all-errors), then delegates to `Workflow::new` for the graph construction and cycle/missing-dep validation.
**Warning signs:** Same validation error messages appearing in two different code paths.

### Pitfall 2: Returning Result<Workflow, Error> Instead of Collecting Errors

**What goes wrong:** `build()` returns the first error found, requiring the developer to fix-and-rebuild repeatedly.
**Why it happens:** Using `?` operator / fail-fast out of habit.
**How to avoid:** Accumulate all errors in a `Vec<BuilderError>`, return `Ok(workflow)` only if the vec is empty. This means running all validation checks even if earlier ones found problems. The tricky part: cycle detection via `toposort` only reports one cycle participant, not all cycles. Accept this limitation -- one cycle error per build is acceptable since fixing one cycle often resolves others.
**Warning signs:** Users complaining about "whack-a-mole" error fixing.

### Pitfall 3: Inconsistent Edge Convention with Workflow::new

**What goes wrong:** The builder's `.edge()` method uses a different convention than `Workflow::new`'s `(from, to)` edges.
**Why it happens:** Not checking the existing convention.
**How to avoid:** Match the existing convention exactly: `.edge("A", "B")` means "A must complete before B" (same as `Workflow::new`'s `(from, to)`). Document it clearly in the method's doc comment.
**Warning signs:** Workflows built via the builder execute steps in the wrong order compared to workflows built via `Workflow::new`.

### Pitfall 4: Move Semantics Breaking Conditional Builder Logic

**What goes wrong:** A user wants to conditionally add a step: `if condition { builder = builder.step("X", x); }` -- this requires reassignment because the builder was moved.
**Why it happens:** Consuming self means the old binding is invalidated after each method call.
**How to avoid:** This is an inherent tradeoff of consuming self. Document it. The reassignment pattern (`builder = builder.step(...)`) works fine. The chained style (`Workflow::builder().step(...).step(...).build()`) is the primary usage pattern and works perfectly. Conditional logic uses the multi-statement style with reassignment.
**Warning signs:** Compiler errors about "use of moved value" in conditional builder code.

### Pitfall 5: Disconnected Check Flagging Root and Leaf Steps

**What goes wrong:** The disconnected check incorrectly flags steps that have only incoming edges (leaf/terminal steps) or only outgoing edges (root/source steps) as disconnected.
**Why it happens:** Checking for zero incoming OR zero outgoing instead of zero incoming AND zero outgoing.
**How to avoid:** A step is disconnected only if it has **neither** incoming **nor** outgoing edges. Root steps (no incoming) and terminal steps (no outgoing) are perfectly normal in a DAG. The check must use AND, not OR.
**Warning signs:** Valid diamond-shaped or fan-out workflows being rejected.

## Code Examples

### Complete Builder Usage (Target API)

```rust
use agentic_framework::workflow::{Workflow, WorkflowBuilder};
use agentic_framework::workflow::step::Step;
use agentic_framework::types::ModelOptions;
use serde_json::json;

// Fluent construction of a three-step workflow:
let wf = Workflow::builder()
    .transform_step("format_input", |_inputs| {
        Ok(json!({"topic": "Rust"}))
    })
    .llm_step("generate", Box::new(my_model), |inputs| {
        let topic = inputs["format_input"]["topic"]
            .as_str()
            .unwrap_or("unknown");
        format!("Write about {topic}")
    })
    .transform_step("format_output", |inputs| {
        let text = inputs["generate"].as_str().unwrap_or("");
        Ok(json!({"result": text}))
    })
    .chain(&["format_input", "generate", "format_output"])
    .build()?;
```

Compare with the raw API:
```rust
// Same workflow using Workflow::new -- more verbose:
let wf = Workflow::new(
    vec![
        ("format_input".to_string(), Step::Transform {
            transform: Box::new(|_inputs| Ok(json!({"topic": "Rust"}))),
        }),
        ("generate".to_string(), Step::Llm {
            model: Box::new(my_model),
            prompt_builder: Box::new(|inputs| {
                let topic = inputs["format_input"]["topic"]
                    .as_str()
                    .unwrap_or("unknown");
                format!("Write about {topic}")
            }),
            tools: None,
            options: ModelOptions::default(),
        }),
        ("format_output".to_string(), Step::Transform {
            transform: Box::new(|inputs| {
                let text = inputs["generate"].as_str().unwrap_or("");
                Ok(json!({"result": text}))
            }),
        }),
    ],
    vec![
        ("format_input".to_string(), "generate".to_string()),
        ("generate".to_string(), "format_output".to_string()),
    ],
)?;
```

The builder version is more readable: step types are explicit method names, edge convention is clear via `.chain()`, and there is no manual `Box::new()` or `String` conversion noise.

### Disconnected Step Detection

```rust
// This should produce a BuilderError::DisconnectedStep("orphan"):
let result = Workflow::builder()
    .transform_step("A", |_| Ok(json!("a")))
    .transform_step("B", |_| Ok(json!("b")))
    .transform_step("orphan", |_| Ok(json!("orphan")))
    .edge("A", "B")
    .build();

// result is Err(vec![BuilderError::DisconnectedStep("orphan".into())])
```

### Build-Time Error Collection

```rust
// Multiple errors collected at once:
let result = Workflow::builder()
    .transform_step("A", |_| Ok(json!("a")))
    .transform_step("A", |_| Ok(json!("a2")))  // duplicate!
    .edge("A", "nonexistent")                    // missing step!
    .build();

// result contains BOTH errors, not just the first one
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw Vec-of-tuples constructor | Fluent builder wrapping the same validation | This phase | Better developer ergonomics, same runtime behavior |
| Single error from Workflow::new | Collected errors from builder | This phase | Fix all problems in one pass |
| No disconnected step detection | Builder flags disconnected steps | This phase | Catches a common workflow construction mistake |

**No deprecated patterns apply.** This is purely additive -- the builder wraps existing functionality.

## Open Questions

1. **Should `build()` return `Result<Workflow, Vec<BuilderError>>` or `Result<Workflow, BuilderErrors>` (a wrapper struct)?**
   - A `Vec<BuilderError>` is simple and direct. A wrapper struct (`BuilderErrors`) could implement `Display` to format all errors nicely and implement `std::error::Error`.
   - Recommendation: Use a wrapper struct `BuilderErrors` that contains a `Vec<BuilderError>` and implements `Display` and `Error`. This gives a better experience when the user prints the error. It can also implement `From<BuilderErrors> for Error` to integrate with the existing error type.

2. **Should the builder support removing or replacing steps?**
   - Remove/replace adds complexity and is unusual for builder patterns. The builder is a write-once construction API.
   - Recommendation: No. Steps are added, never removed or replaced. If the user needs to change a step, they build a new workflow.

3. **Should `Workflow::new()` also detect disconnected steps?**
   - The user's requirement is about `build()` specifically. Adding it to `Workflow::new()` would be a breaking change (existing tests have disconnected steps).
   - Recommendation: No. Only the builder performs disconnected step detection. `Workflow::new()` remains unchanged for backward compatibility.

## Sources

### Primary (HIGH confidence)
- Existing codebase analysis: `src/workflow/workflow.rs`, `src/workflow/step.rs`, `src/error.rs`, `src/types.rs` -- all integration points verified by direct code reading
- petgraph 0.8.3 `Graph::externals()` documentation (https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.Graph.html#method.externals) -- verified `externals` only finds nodes with zero edges in ONE direction, not suitable alone for disconnected detection
- petgraph 0.8.3 `Graph::neighbors_directed()` documentation (https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.Graph.html#method.neighbors_directed) -- verified for checking both directions
- petgraph 0.8.3 `algo::toposort` documentation (https://docs.rs/petgraph/0.8.3/petgraph/algo/fn.toposort.html) -- confirmed cycle detection behavior, already used in Workflow::new
- petgraph 0.8.3 `algo::connected_components` documentation (https://docs.rs/petgraph/0.8.3/petgraph/algo/fn.connected_components.html) -- evaluates weakly connected components, but too coarse for disconnected-step detection (counts components, does not identify isolated nodes)
- Rust builder pattern official style guide (https://doc.rust-lang.org/1.0.0/style/ownership/builders.html) -- consuming self pattern
- Rust Design Patterns book: Builder (https://rust-unofficial.github.io/patterns/patterns/creational/builder.html) -- consuming self vs &mut self tradeoffs

### Secondary (MEDIUM confidence)
- Effective Rust, Item 7: Builders (https://www.lurklurk.org/effective-rust/builders.html) -- consuming self vs &mut self tradeoffs, build() consuming the builder
- Rust Users Forum: Builder self vs &mut self (https://users.rust-lang.org/t/builder-type-design-return-mut-self-vs-self/74012) -- community discussion on tradeoffs

### Tertiary (LOW confidence)
- WebSearch results for builder crate comparisons (derive_builder, typed-builder, bon) -- confirmed these do not support non-Clone types with custom validation, but did not verify against crate docs directly

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies needed; pure Rust API design; all petgraph APIs verified against docs.rs
- Architecture: HIGH -- builder pattern is well-established in Rust; consuming self pattern verified in official style guide and design patterns book; consistent with existing ModelOptions pattern in codebase
- Pitfalls: HIGH -- derived from direct analysis of Rust borrow checker constraints with non-Clone types, and from examination of the existing Workflow::new validation logic
- Code examples: HIGH -- builder API examples are aspirational (target API), but validated against the existing Step/Workflow types to ensure type compatibility

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain -- Rust builder pattern is well-established, petgraph 0.8.x is current)
