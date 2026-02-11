# Phase 6: Examples - Research

**Researched:** 2026-02-10
**Domain:** Rust example programs demonstrating agentic-framework API
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Example structure & layout:**
- Standard Rust `examples/` directory -- each `.rs` file is a standalone binary (`cargo run --example name`)
- Exactly 3 examples matching the success criteria: simple call, tool calling, workflow
- Example tools defined inline in each example file -- fully self-contained, no shared modules
- Each example has a top-of-file doc comment block explaining what it demonstrates, plus inline comments at key decision points

**Example tool design:**
- Tool choice: Claude's discretion on which tools best demonstrate the Tool trait (calculator and weather are suggestions, not requirements)
- JSON parameter schemas written by hand using `serde_json::json!({...})` -- nothing hidden behind macros or derives
- Tool-calling example must handle both paths: tool-call response AND plain-text response -- shows the full ModelResponse enum usage

**Running experience:**
- Check for `OPENAI_API_KEY` at startup; print a clear, helpful error message if missing, then exit (don't panic)
- Formatted output with section headers and labels (e.g., `--- Simple Chat ---\nResponse: ...`) so terminal output tells a story
- All 3 examples use real LLM calls via OpenAI (no mock models) -- requires API key to run
- Workflow example uses a concrete, realistic use case (e.g., summarize-then-translate, extract-then-format) rather than abstract step names

### Claude's Discretion
- Which specific example tools to create (calculator, weather, or alternatives)
- Whether tools return real computed results vs realistic fake data
- The specific concrete use case for the workflow example
- Exact formatting style for terminal output

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Summary

Phase 6 creates exactly 3 standalone example programs in `examples/` that demonstrate every major framework concept: simple LLM call, tool calling with dispatch, and multi-step workflow with data flow. The examples are the final deliverable of the project and serve as the primary learning resource for developers.

The research focused on: (1) how the existing framework API maps to example code, (2) the Cargo examples convention and how examples are compiled/run, (3) which specific example tools and workflow use cases best demonstrate the API, and (4) common pitfalls when writing Rust examples that depend on external services.

The framework API is well-defined and stable from Phases 1-5. Every type the examples need is publicly re-exported from `lib.rs`. The key challenge is pedagogical: structuring each example so a developer reads top-to-bottom and understands the framework's design decisions (enum responses, trait objects, DAG data flow) without needing to look at the library source.

**Primary recommendation:** Write three progressively complex examples (simple_chat.rs, tool_calling.rs, workflow.rs) that each begin with an API key check, use `OpenAiProvider::from_env()`, and demonstrate one major concept layer. Use a calculator tool (real computation) and a get_weather tool (realistic fake data) for tool calling. Use a summarize-then-translate pipeline for the workflow example.

## Standard Stack

### Core (already in Cargo.toml -- examples inherit library dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.49 | Async runtime for `#[tokio::main]` | Already a dependency; examples need async main |
| serde_json | 1.0 | `json!()` macro for hand-written schemas | Already a dependency; needed for tool parameters |
| async-trait | 0.1 | `#[async_trait]` on Tool impl | Already a dependency; needed for Tool trait |

### No Additional Dependencies Needed
Examples use only the framework's public API and its existing transitive dependencies. No new crates need to be added to `Cargo.toml`. The `tokio` dependency already has `features = ["full"]` which includes the `macros` feature needed for `#[tokio::main]`.

### Cargo.toml -- No Changes Required
Cargo auto-discovers `.rs` files in `examples/`. No `[[example]]` sections needed. No new dependencies needed. The existing `Cargo.toml` is sufficient as-is.

## Architecture Patterns

### Project Structure
```
examples/
    simple_chat.rs      # Example 1: single LLM call (simplest possible)
    tool_calling.rs     # Example 2: tool definition + dispatch + both response paths
    workflow.rs         # Example 3: multi-step workflow with data flow
```

Naming uses snake_case per Rust module conventions. Each file is run via:
```bash
cargo run --example simple_chat
cargo run --example tool_calling
cargo run --example workflow
```

### Pattern 1: API Key Guard at Entry Point
**What:** Every example checks for `OPENAI_API_KEY` before doing anything, printing a helpful message and exiting cleanly on failure.
**When to use:** Every example file.
**Example:**
```rust
// Source: Verified against OpenAiProvider::from_env() in src/openai/mod.rs
#[tokio::main]
async fn main() {
    // Check for API key before proceeding.
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: OPENAI_API_KEY environment variable not set.");
            eprintln!();
            eprintln!("To run this example, set your OpenAI API key:");
            eprintln!("  export OPENAI_API_KEY=sk-...");
            std::process::exit(1);
        }
    };

    // Use the key to create a provider.
    let provider = OpenAiProvider::new(api_key, "gpt-4o-mini");
    // ...
}
```

**Design note:** Use `std::env::var` + `std::process::exit(1)` rather than `OpenAiProvider::from_env().unwrap()` because the user decision requires a "clear, helpful error message" -- not a panic backtrace. However, the provider internally also checks via `from_env()`, so an alternative is to call `from_env()` and match on the `Result`, converting `Err` to a friendly message. Either approach works; the key requirement is no panic output.

### Pattern 2: Sectioned Terminal Output
**What:** Each example prints labeled sections so the terminal output tells a readable story.
**When to use:** Every example file.
**Example:**
```rust
println!("--- Simple Chat Example ---");
println!();
println!("Sending message to OpenAI...");
// ... make API call ...
println!();
println!("Response:");
println!("{}", response_text);
```

### Pattern 3: ModelResponse Enum Match (Tool Calling)
**What:** The tool-calling example demonstrates matching on `ModelResponse::Text` vs `ModelResponse::ToolCalls` to show the reader the full enum API.
**When to use:** Tool-calling example must show both paths.
**Example:**
```rust
// Source: Verified against ModelResponse enum in src/types.rs
match response {
    ModelResponse::Text(text) => {
        println!("Model responded with text (no tool call):");
        println!("{}", text);
    }
    ModelResponse::ToolCalls(calls) => {
        println!("Model requested {} tool call(s):", calls.len());
        for call in &calls {
            println!("  Tool: {} | Args: {}", call.name, call.arguments);
        }
        // Dispatch tool calls via registry
        let results = registry.dispatch_all(&calls).await.unwrap();
        // ...
    }
}
```

### Pattern 4: Workflow Builder with Closures
**What:** The workflow example uses `Workflow::builder()` with `llm_step()` and `transform_step()` to show the fluent API.
**When to use:** Workflow example.
**Example:**
```rust
// Source: Verified against WorkflowBuilder in src/workflow/builder.rs
let workflow = Workflow::builder()
    .llm_step(
        "summarize",
        Box::new(OpenAiProvider::new(api_key.clone(), "gpt-4o-mini")),
        |_inputs| {
            format!("Summarize this article in 2-3 sentences:\n\n{}", ARTICLE_TEXT)
        },
    )
    .llm_step(
        "translate",
        Box::new(OpenAiProvider::new(api_key.clone(), "gpt-4o-mini")),
        |inputs| {
            let summary = inputs["summarize"].as_str().unwrap_or("No summary available");
            format!("Translate the following English text to Spanish:\n\n{}", summary)
        },
    )
    .edge("summarize", "translate")
    .build()
    .expect("workflow should be valid");
```

### Anti-Patterns to Avoid
- **Using `.unwrap()` without explanation:** Every `.unwrap()` or `?` should have a comment explaining why it is safe or acceptable in example context.
- **Abstracting away framework usage:** Don't create helper functions that hide the framework API calls. The reader should see every framework call inline.
- **Using `from_env()` with raw `.unwrap()`:** This produces a panic backtrace, not a friendly error message. Use `match` or map the error to a clean exit.
- **Hardcoding API keys:** Never embed API keys in example code. Always use environment variables.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| API key validation | Custom env parsing logic | `std::env::var("OPENAI_API_KEY")` | Standard library covers this exactly |
| Tool dispatch loop | Manual HashMap lookup + execute | `registry.dispatch_all(&calls)` | Framework already has this (tool.rs) |
| Tool definitions | Derive macros or code generation | `serde_json::json!({...})` hand-written schemas | User decision: explicit, nothing hidden |
| Workflow construction | Manual `Workflow::new()` with raw vecs | `Workflow::builder()` fluent API | Builder is the intended public API |
| Async runtime setup | Manual tokio Runtime::new() | `#[tokio::main]` attribute | Standard Rust async entry point |

**Key insight:** The examples should use the framework's public API exactly as a real user would. The whole point is to demonstrate the API, not to add abstraction layers on top.

## Common Pitfalls

### Pitfall 1: Examples That Don't Compile Because Library API Changed
**What goes wrong:** Examples reference types or methods that were renamed or moved during earlier phases.
**Why it happens:** Examples are written last but reference every prior phase's output.
**How to avoid:** After writing each example, run `cargo build --examples` to verify compilation. Use the exact public API from `lib.rs` re-exports.
**Warning signs:** Using `use agentic_framework::openai::types::*` (internal types) instead of `use agentic_framework::OpenAiProvider` (public re-exports).

### Pitfall 2: Forgetting `async-trait` Import for Tool Implementation
**What goes wrong:** `#[async_trait]` attribute on `impl Tool for MyTool` won't compile without importing the macro.
**Why it happens:** The Tool trait uses `#[async_trait]` and implementations must also use it.
**How to avoid:** Include `use async_trait::async_trait;` at the top of every example that implements Tool.
**Warning signs:** Compiler error about async fn not being allowed in trait implementations.

### Pitfall 3: Cloning API Key for Multiple OpenAiProvider Instances
**What goes wrong:** Workflow example needs multiple `OpenAiProvider` instances (one per LLM step) but `api_key` is moved into the first one.
**Why it happens:** `OpenAiProvider::new()` takes `impl Into<String>`, which consumes a `String`.
**How to avoid:** Either clone the API key string before each provider creation, or read it once into a `let api_key = ...` binding and `.clone()` it for each provider.
**Warning signs:** "value used after move" compiler error.

### Pitfall 4: Tool-Calling Example Only Handling One Response Path
**What goes wrong:** The example only handles `ModelResponse::ToolCalls` and panics if the model returns `ModelResponse::Text` instead.
**Why it happens:** The model may not always call tools -- it can respond with plain text even when tools are available.
**How to avoid:** User decision explicitly requires handling both paths with a `match`. The example MUST show a `match response { ModelResponse::Text(...) => ..., ModelResponse::ToolCalls(...) => ... }`.
**Warning signs:** Using `.tool_calls().unwrap()` instead of matching on the enum.

### Pitfall 5: Workflow Prompt Builder Closures With Wrong Input Key
**What goes wrong:** A prompt_builder closure references `inputs["step_a"]` but the step is named `"step_A"` or uses a different name.
**Why it happens:** StepInput keys are step names (strings) and there's no compile-time check.
**How to avoid:** Define step names as constants or use the exact same string literals in step definitions and input lookups.
**Warning signs:** `.as_str().unwrap_or("...")` silently returning the fallback default.

### Pitfall 6: Examples Not Reflecting the Correct Import Paths
**What goes wrong:** Examples use `use agentic_framework::model::Model` instead of `use agentic_framework::Model`.
**Why it happens:** The framework re-exports types at the crate root, but developers may reference internal module paths.
**How to avoid:** Use only the re-exported paths from `lib.rs`. The correct imports are:
```rust
use agentic_framework::{
    Message, Model, ModelOptions, ModelResponse,
    OpenAiProvider, Tool, ToolRegistry, ToolCall,
    Workflow, WorkflowBuilder, Step,
};
```
**Warning signs:** Import paths with multiple segments like `agentic_framework::workflow::step::Step`.

## Code Examples

### Example 1: Complete Simple Chat Structure
```rust
// Source: Verified against src/lib.rs re-exports and src/openai/mod.rs
//! Simple Chat Example
//!
//! Demonstrates the simplest possible usage of the agentic-framework:
//! create an OpenAI provider, send a single message, and print the response.
//!
//! Run: cargo run --example simple_chat
//! Requires: OPENAI_API_KEY environment variable

use agentic_framework::{Message, ModelOptions, ModelResponse, OpenAiProvider, Model};

#[tokio::main]
async fn main() {
    // 1. Check for API key.
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: OPENAI_API_KEY environment variable not set.");
            eprintln!();
            eprintln!("Set your API key and try again:");
            eprintln!("  export OPENAI_API_KEY=sk-...");
            std::process::exit(1);
        }
    };

    println!("--- Simple Chat ---");
    println!();

    // 2. Create the OpenAI provider.
    let provider = OpenAiProvider::new(api_key, "gpt-4o-mini");

    // 3. Build the conversation.
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is the capital of France?"),
    ];

    // 4. Send the request.
    let options = ModelOptions::new().with_temperature(0.7);
    let response = provider.chat(&messages, &options).await.unwrap();

    // 5. Print the response.
    match response {
        ModelResponse::Text(text) => {
            println!("Response: {}", text);
        }
        ModelResponse::ToolCalls(_) => {
            println!("Unexpected: model returned tool calls without tools provided.");
        }
    }
}
```

### Example 2: Tool Implementation Pattern (Calculator)
```rust
// Source: Verified against Tool trait in src/tool.rs
use async_trait::async_trait;
use agentic_framework::{Tool, error::Result};
use serde_json::json;

struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluates basic arithmetic. Supports add, subtract, multiply, divide."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "The first operand"
                },
                "b": {
                    "type": "number",
                    "description": "The second operand"
                }
            },
            "required": ["operation", "a", "b"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let op = args["operation"].as_str().unwrap_or("add");
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);

        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Ok("Error: division by zero".to_string());
                }
                a / b
            }
            _ => return Ok(format!("Unknown operation: {}", op)),
        };

        Ok(format!("{}", result))
    }
}
```

### Example 3: Tool Implementation Pattern (Weather -- Fake Data)
```rust
// Source: Verified against Tool trait in src/tool.rs
struct GetWeather;

#[async_trait]
impl Tool for GetWeather {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "Gets the current weather for a given city. Returns temperature and conditions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name to get weather for"
                }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let city = args["city"].as_str().unwrap_or("Unknown");

        // Return realistic fake data -- this is a demonstration tool.
        // A real implementation would call a weather API.
        Ok(format!(
            "Weather in {}: 72°F (22°C), partly cloudy, humidity 45%",
            city
        ))
    }
}
```

### Example 4: Workflow Builder Usage Pattern
```rust
// Source: Verified against WorkflowBuilder in src/workflow/builder.rs
// and executor in src/workflow/executor.rs
let api_key = std::env::var("OPENAI_API_KEY").unwrap();

let workflow = Workflow::builder()
    .llm_step(
        "summarize",
        Box::new(OpenAiProvider::new(api_key.clone(), "gpt-4o-mini")),
        |_inputs| {
            format!(
                "Summarize the following article in 2-3 sentences:\n\n{}",
                ARTICLE_TEXT
            )
        },
    )
    .llm_step(
        "translate",
        Box::new(OpenAiProvider::new(api_key.clone(), "gpt-4o-mini")),
        |inputs| {
            let summary = inputs["summarize"]
                .as_str()
                .unwrap_or("No summary available");
            format!(
                "Translate the following English text to Spanish:\n\n{}",
                summary
            )
        },
    )
    .edge("summarize", "translate")
    .build()
    .expect("workflow definition is valid");

let outputs = workflow.execute().await.unwrap();
```

## Discretion Recommendations

### Tool Selection: Calculator + GetWeather
**Recommendation:** Use both a `Calculator` tool (real computation) and a `GetWeather` tool (realistic fake data).

**Rationale:**
- **Calculator** returns real computed results -- demonstrates that tools can do actual work, not just return canned strings. The arithmetic is trivially verifiable by the reader.
- **GetWeather** returns realistic fake data -- demonstrates the common pattern of wrapping external APIs. The "fake data" approach is honest (commented as such) and avoids requiring a second API key.
- Together they show two common tool patterns: computation and data retrieval.
- Both are widely recognized in LLM tool-calling tutorials, making the examples immediately familiar.

### Workflow Use Case: Summarize-Then-Translate
**Recommendation:** Use a two-step LLM workflow that summarizes an article then translates the summary to Spanish.

**Rationale:**
- **Realistic:** Summarize-then-translate is something a developer might actually build (content localization, news aggregation).
- **Clear data flow:** The translate step obviously depends on the summarize step's output, making the DAG dependency visible and intuitive.
- **Two LLM calls:** Shows that each step gets its own `OpenAiProvider` instance and that data flows as JSON Values between steps.
- **Verifiable output:** The reader can see the English summary and the Spanish translation in the terminal, confirming the pipeline worked.
- **No extra dependencies:** Both steps use the same `OpenAiProvider` (with different prompts), no additional tools or APIs needed.

**Alternative considered:** Extract-then-format (extract structured data, then reformat). This is also good but less visually dramatic in terminal output -- translation produces clearly different text that proves the pipeline worked.

### Tool Result Strategy: Mixed (Real Computation + Fake Data)
**Recommendation:** Calculator returns real computed results. GetWeather returns realistic hardcoded data.

**Rationale:** This shows both patterns developers encounter in practice. The calculator's real results demonstrate that `execute()` can do meaningful work. The weather tool's fake data demonstrates the wrapper pattern for external services.

### Terminal Output Style
**Recommendation:** Use `---` separator lines with section names, blank lines between sections, and labeled values.

```
--- Tool Calling Example ---

Registering tools: calculator, get_weather
Sending message with 2 tool(s) available...

Model requested 1 tool call(s):
  Tool: calculator
  Arguments: {"operation":"add","a":15,"b":27}

Executing tool calls...
  calculator -> 42

Sending tool results back to model...

Final response: The sum of 15 and 27 is 42.
```

**Rationale:** This format is readable in a terminal, works in CI logs, and makes the multi-step nature of tool calling visible.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[tokio::main]` needs separate `tokio-macros` crate | `tokio` with `features = ["full"]` includes macros | tokio 1.0 (2021) | Already correct in Cargo.toml |
| `OpenAI functions` parameter in API | `tools` parameter with `{"type": "function", ...}` wrapper | OpenAI API mid-2023 | Already handled in Phase 2 wire types |

**Deprecated/outdated:**
- The `functions` and `function_call` parameters in OpenAI API have been superseded by `tools` and `tool_choice`. The framework already uses the `tools` format (verified in `src/openai/types.rs`).

## Open Questions

1. **Model name for examples: `gpt-4o-mini` vs `gpt-4o`?**
   - What we know: `gpt-4o-mini` is cheaper and faster, appropriate for examples that will be run repeatedly during development. `gpt-4o` is more capable.
   - What's unclear: Whether `gpt-4o-mini` reliably uses tools when prompted. Based on training data, it does support tool calling.
   - Recommendation: Use `gpt-4o-mini` for all examples (cheaper, faster) but mention in comments that any `gpt-4o` family model works.

2. **Article text for workflow example: inline const or separate?**
   - What we know: User decision says examples must be fully self-contained, no shared modules.
   - Recommendation: Define the article text as a `const ARTICLE: &str = "..."` at the top of the workflow example file. Keep it short (1-2 paragraphs) so the example is readable.

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection: all files in `src/` read and verified
  - `src/lib.rs` -- public re-exports confirming import paths
  - `src/model.rs` -- Model trait with `chat()` and `chat_with_tools()` signatures
  - `src/types.rs` -- ModelResponse enum, ModelOptions builder, ToolCall, ToolDefinition
  - `src/message.rs` -- Message constructors: `system()`, `user()`, `assistant()`, `tool_result()`
  - `src/tool.rs` -- Tool trait, ToolRegistry with `register()`, `dispatch()`, `dispatch_all()`, `definitions()`
  - `src/openai/mod.rs` -- OpenAiProvider with `new()`, `from_env()`, `with_base_url()`
  - `src/workflow/builder.rs` -- WorkflowBuilder with `llm_step()`, `transform_step()`, `edge()`, `chain()`, `build()`
  - `src/workflow/executor.rs` -- `Workflow::execute()` returning `HashMap<String, Value>`
  - `src/workflow/step.rs` -- Step enum (Llm/Transform), StepInput/StepOutput type aliases
- Cargo.toml -- dependency versions (tokio 1.49, serde 1.0, serde_json 1.0, async-trait 0.1, reqwest 0.13, petgraph 0.8)

### Secondary (MEDIUM confidence)
- [Cargo Project Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html) -- examples/ directory convention
- [Cargo Target Auto-Discovery](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery) -- confirmed no `[[example]]` sections needed
- [Cargo Conventions - Rust By Example](https://doc.rust-lang.org/rust-by-example/cargo/conventions.html) -- kebab-case naming convention (though snake_case also works and is more Rust-idiomatic for module names)
- [std::env::var](https://doc.rust-lang.org/std/env/fn.var.html) -- environment variable reading API

### Tertiary (LOW confidence)
- None -- all findings verified against codebase or official documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- direct codebase inspection, no external libraries needed
- Architecture: HIGH -- patterns derived from verified API surface in source code
- Pitfalls: HIGH -- derived from actual API signatures and Rust compiler behavior
- Discretion recommendations: MEDIUM -- tool and workflow choices are subjective but well-reasoned

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (30 days -- stable domain, no moving parts)
