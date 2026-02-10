# Phase 3: Tool System - Research

**Researched:** 2026-02-10
**Domain:** Rust async trait design for tool abstraction, registry pattern, LLM tool-call dispatch
**Confidence:** HIGH

## Summary

Phase 3 defines the `Tool` trait (the developer-facing API for creating LLM-callable tools), a `ToolRegistry` struct (name-based lookup with `Box<dyn Tool>` ownership), and a single-step dispatch method that executes tool calls from a `ModelResponse::ToolCalls` and returns results. The caller controls looping -- the framework handles one round of tool execution.

The key design challenge is making the `Tool` trait both ergonomic for implementors and dyn-safe for storage in `Box<dyn Tool>`. The existing codebase already uses `async_trait` for the `Model` trait with `Send + Sync` supertraits, and the same pattern applies directly to `Tool`. The codebase also already has the `ToolCall` struct (with `id`, `name`, `arguments: String`), `ToolDefinition` struct, `Message::tool_result()` constructor, and `Error::ToolNotFound` / `Error::ToolExecutionFailed` variants -- Phase 3 wires these existing pieces together with the new `Tool` trait and `ToolRegistry`.

The OpenAI Chat Completions API expects tool results as a message with `role: "tool"`, `tool_call_id`, `name`, and `content` as a **string** (not structured JSON). This means the tool execute method should return `String`, matching what the API wire format needs. The `ToolCall.arguments` field is already `String` (raw JSON), so the tool receives a `String` and returns a `String` -- simple, consistent, and directly maps to the API contract.

**Primary recommendation:** Use `async_trait` with `Send + Sync` for the `Tool` trait (matching `Model`), accept `serde_json::Value` as input (parsed from the `String` arguments), return `String` as output (matching OpenAI's tool result content field), use a `HashMap<String, Box<dyn Tool>>` registry with fail-fast duplicate detection, and skip schema validation (let tools validate their own arguments -- more educational and avoids a heavy dependency).

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Tool trait shape:**
- Parameter schema declared by returning `serde_json::Value` -- developer builds JSON schema explicitly, no magic
- Tool must be **Send + Sync** -- consistent with Model trait, required for async task sharing

**Dispatch & looping:**
- **Single-step dispatch** -- framework executes one round of tool calls and returns. Caller decides whether to loop back to the LLM
- **Sequential execution** when model returns multiple tool calls -- one at a time, in order
- **Fail fast** on tool error -- stop immediately, don't execute remaining tools in a multi-tool response

**Parameter & result types:**
- Tool errors use **Result with the framework's Error type** -- consistent single error enum, add ToolExecution variant if needed

**Tool registry design:**
- **ToolRegistry struct** -- dedicated struct that holds tools, handles name lookup, validates no duplicates
- **Error on duplicate names** at registration time -- fail-fast, prevents subtle bugs
- Registry **owns tools via Box&lt;dyn Tool&gt;** -- simple lifetime story, tools live as long as the registry
- Dispatch is a **method on ToolRegistry** -- `registry.dispatch(tool_call)`, encapsulated and discoverable

### Claude's Discretion

- Async vs sync execute method (likely async given existing async_trait usage)
- Separate trait methods vs metadata struct grouping
- Max iteration safeguard at framework level
- Tool execute input type (serde_json::Value vs String)
- Tool execute return type (String vs serde_json::Value)
- Whether framework validates args against schema before calling execute

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope

</user_constraints>

## Standard Stack

### Core (already in Cargo.toml)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| async-trait | 0.1 | Dyn-safe async Tool trait | Already used for Model; same pattern applies to Tool |
| serde_json | 1.0 | Tool argument parsing, schema representation | Already a dependency; `serde_json::Value` for schemas and parsed args |
| tokio | 1.49 | Async runtime for tool execution | Already used throughout |
| thiserror | 2.0 | Error enum with ToolExecutionFailed variant | Already used; variant already exists |

### No New Dependencies Needed

Phase 3 requires **zero new crate dependencies**. Everything needed is already in `Cargo.toml`:
- `async-trait` for the dyn-safe `Tool` trait
- `serde_json` for `Value` (tool schemas and argument parsing)
- `thiserror` for the existing `Error` enum (which already has `ToolNotFound` and `ToolExecutionFailed`)

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| async_trait for Tool | Native async fn in trait | Not dyn-safe (Rust 1.75 stabilized async fn in traits but NOT for dyn Trait). Cannot use Box<dyn Tool>. |
| No schema validation | jsonschema crate (v0.38+) | Adds a significant dependency (requires Rust 1.83+) for validation the LLM tools can do themselves. Not educational for the core goal. |
| serde_json::Value input | Raw String input | Value is already parsed JSON; avoids every tool implementor repeating `serde_json::from_str`. One parse at the dispatch layer. |
| String output | serde_json::Value output | OpenAI API content field is a string. Value would require serialization back to String at the message layer -- unnecessary indirection. |

## Architecture Patterns

### Recommended Project Structure

```
src/
├── lib.rs              # Add `pub mod tool;`, re-export Tool, ToolRegistry
├── tool.rs             # Tool trait + ToolRegistry struct + dispatch logic
├── error.rs            # Already has ToolNotFound, ToolExecutionFailed
├── types.rs            # Already has ToolCall, ToolDefinition
├── message.rs          # Already has Message::tool_result()
├── model.rs            # Unchanged
└── openai/             # Unchanged
```

Single file (`tool.rs`) is appropriate because the Tool trait and ToolRegistry are tightly coupled and the total code is modest (~150-200 lines). No need for a module directory.

### Pattern 1: Dyn-Safe Async Tool Trait with async_trait

**What:** Define the Tool trait with `#[async_trait]` and `Send + Sync` supertraits, matching the existing Model trait pattern.

**Why:** The codebase already uses this exact pattern for Model. Consistency is paramount in an educational codebase. Using async_trait makes the trait dyn-safe, enabling `Box<dyn Tool>` storage in the registry.

**Recommended trait shape:**

```rust
use async_trait::async_trait;
use crate::error::Result;
use crate::types::ToolDefinition;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the tool's name (must be unique within a registry).
    fn name(&self) -> &str;

    /// Returns the tool's description for the LLM.
    fn description(&self) -> &str;

    /// Returns the JSON Schema describing the tool's parameters.
    fn parameters(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments.
    ///
    /// `args` is the parsed JSON from the LLM's tool call arguments.
    /// Returns a string result suitable for inclusion in a tool result message.
    async fn execute(&self, args: serde_json::Value) -> Result<String>;
}
```

**Design choice: Separate methods (not metadata struct)**

Separate methods (`name()`, `description()`, `parameters()`) are preferred over a single `fn definition(&self) -> ToolDefinition` for these reasons:
1. More educational -- each piece of metadata has its own method, making the trait surface explicit
2. The trait can provide a default `fn definition(&self) -> ToolDefinition` that assembles from the individual methods
3. Implementors see exactly what they need to provide
4. Consistent with how Rust standard library traits expose individual methods (e.g., `Iterator::size_hint()` rather than a metadata struct)

**Default method for ToolDefinition assembly:**

```rust
/// Assemble a ToolDefinition from the trait methods.
/// This is the format sent to the LLM API.
fn definition(&self) -> ToolDefinition {
    ToolDefinition {
        name: self.name().to_string(),
        description: self.description().to_string(),
        parameters: self.parameters(),
    }
}
```

### Pattern 2: HashMap-Based ToolRegistry with Box<dyn Tool>

**What:** A dedicated struct wrapping `HashMap<String, Box<dyn Tool>>` with registration and lookup methods.

**Why:** Simple ownership story (registry owns tools), O(1) name lookup, fail-fast duplicate detection at registration time.

```rust
use std::collections::HashMap;

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Register a tool. Returns Error if a tool with this name already exists.
    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<()> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(Error::DuplicateTool(name));  // New error variant
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Look up a tool by name.
    fn get(&self, name: &str) -> Result<&dyn Tool> {
        self.tools
            .get(name)
            .map(|t| t.as_ref())
            .ok_or_else(|| Error::ToolNotFound(name.to_string()))
    }

    /// Return ToolDefinitions for all registered tools (for sending to the LLM).
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
}
```

### Pattern 3: Single-Step Dispatch on ToolRegistry

**What:** A `dispatch` method on `ToolRegistry` that takes a `&ToolCall`, finds the tool, parses arguments, executes, and returns the result as a `Message`.

**Why:** Encapsulates the full tool-call round trip. The caller gets back a `Message::tool_result()` ready to append to conversation history.

```rust
impl ToolRegistry {
    /// Execute a single tool call and return the result as a Message.
    pub async fn dispatch(&self, tool_call: &ToolCall) -> Result<Message> {
        let tool = self.get(&tool_call.name)?;

        // Parse raw JSON string into Value
        let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)
            .map_err(|e| Error::ToolExecutionFailed {
                name: tool_call.name.clone(),
                message: format!("invalid arguments: {e}"),
            })?;

        let result = tool.execute(args).await?;

        Ok(Message::tool_result(
            &tool_call.id,
            &tool_call.name,
            result,
        ))
    }
}
```

### Pattern 4: Batch Dispatch with Fail-Fast

**What:** A convenience method that dispatches multiple tool calls sequentially, stopping on first error.

**Why:** `ModelResponse::ToolCalls` contains `Vec<ToolCall>`. This method processes them in order with fail-fast semantics (locked decision).

```rust
impl ToolRegistry {
    /// Execute multiple tool calls sequentially. Stops on first error.
    pub async fn dispatch_all(&self, tool_calls: &[ToolCall]) -> Result<Vec<Message>> {
        let mut results = Vec::with_capacity(tool_calls.len());
        for call in tool_calls {
            results.push(self.dispatch(call).await?);
        }
        Ok(results)
    }
}
```

### Anti-Patterns to Avoid

- **Generic associated types on Tool:** Using `type Args: Deserialize` and `type Output: Serialize` (like Rig does) makes the trait NOT dyn-safe. Cannot use `Box<dyn Tool>`. Rig works around this with a separate `ToolDyn` trait and blanket impl -- unnecessary complexity for an educational codebase.
- **Schema validation at dispatch time:** Adding the `jsonschema` crate for argument validation adds a heavy dependency and doesn't teach the core pattern. Let tools validate their own args in `execute()`.
- **Returning serde_json::Value from execute:** The OpenAI API expects tool content as a string. Returning Value requires a `.to_string()` serialization step at the message boundary. String output is simpler and maps directly to the wire format.
- **Storing tools in a Vec with linear search:** O(n) lookup by name. HashMap gives O(1) and is the natural data structure for name-keyed registries.
- **Mutable &mut self on execute:** Tools should be stateless or use interior mutability (Mutex/RwLock). The `&self` receiver on execute is correct for `Send + Sync`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async dyn-safe traits | Manual Pin<Box<dyn Future>> desugaring | `async_trait` crate | Already used for Model; consistent pattern |
| JSON parsing of tool args | Custom string parsing | `serde_json::from_str` | Standard, well-tested, already a dependency |
| Tool result messages | Manual Message construction | `Message::tool_result()` | Already exists in the codebase (Phase 1) |
| Error types for tool failures | New error types | Existing `Error::ToolNotFound` / `Error::ToolExecutionFailed` | Already in the Error enum (Phase 1) |
| Tool definition assembly | Manual struct construction per tool | Default method on Tool trait (`fn definition()`) | Reduces boilerplate, prevents name mismatch |

**Key insight:** Phase 1 was designed with Phase 3 in mind. The `ToolCall`, `ToolDefinition`, `Message::tool_result()`, `Error::ToolNotFound`, and `Error::ToolExecutionFailed` are all already defined. Phase 3 primarily creates the `Tool` trait, `ToolRegistry` struct, and wires them to these existing types.

## Common Pitfalls

### Pitfall 1: Making Tool Trait Not Dyn-Safe

**What goes wrong:** Using associated types (`type Args`, `type Output`) on the Tool trait prevents `Box<dyn Tool>` because the compiler cannot determine concrete types at runtime.
**Why it happens:** It's tempting to make tools type-safe with generic args/output (like Rig's `Tool` trait), but this makes the trait NOT object-safe.
**How to avoid:** Use concrete types (`serde_json::Value` for input, `String` for output) directly on the trait. No associated types needed.
**Warning signs:** Compiler error "the trait `Tool` cannot be made into an object" when trying `Box<dyn Tool>`.

### Pitfall 2: Forgetting to Parse Arguments Before Calling Execute

**What goes wrong:** Passing the raw `ToolCall.arguments` (a `String`) directly to `execute()` forces every tool implementor to parse JSON themselves.
**Why it happens:** `ToolCall.arguments` is a `String` (raw JSON from the API). It's easy to pass it through without parsing.
**How to avoid:** Parse `ToolCall.arguments` into `serde_json::Value` at the dispatch layer (in `ToolRegistry::dispatch`), then pass the `Value` to `execute()`. One parse point, not N.
**Warning signs:** Every tool implementation starts with `serde_json::from_str`.

### Pitfall 3: Not Echoing tool_call_id in Results

**What goes wrong:** OpenAI API rejects tool result messages that don't include the `tool_call_id` matching the original `ToolCall.id`.
**Why it happens:** It's easy to forget this linkage when building tool result messages manually.
**How to avoid:** The dispatch method should construct the `Message::tool_result()` using the `ToolCall.id`, not the tool itself. The tool's `execute()` method should not need to know about the call ID.
**Warning signs:** API returns "No tool call found for function call output with call_id" errors.

### Pitfall 4: Mut Receiver on Execute Breaking Send + Sync

**What goes wrong:** Using `&mut self` on `execute()` makes the trait incompatible with shared references and concurrent dispatch.
**Why it happens:** Developers want to mutate tool state during execution.
**How to avoid:** Use `&self` receiver. If tools need mutable state, use interior mutability (`Mutex`, `RwLock`, `AtomicU64`, etc.). This is consistent with the `Model` trait which also uses `&self`.
**Warning signs:** Cannot call `execute()` from `&dyn Tool` reference.

### Pitfall 5: Duplicate Tool Names Causing Silent Overwrites

**What goes wrong:** HashMap::insert silently replaces existing values. Two tools with the same name would silently shadow each other.
**Why it happens:** HashMap's default behavior is upsert, not insert-or-fail.
**How to avoid:** Check `contains_key()` before `insert()` and return an error on duplicates (locked decision: error on duplicate names).
**Warning signs:** Tool A registered, Tool B registered with same name, Tool A silently lost.

## Code Examples

### Complete Tool Implementation Example

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use agentic_framework::{Tool, Error, Result};

/// A calculator tool that adds two numbers.
struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Add two numbers together"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "First number" },
                "b": { "type": "number", "description": "Second number" }
            },
            "required": ["a", "b"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let a = args["a"].as_f64().ok_or_else(|| Error::ToolExecutionFailed {
            name: "calculator".to_string(),
            message: "missing or invalid 'a' parameter".to_string(),
        })?;
        let b = args["b"].as_f64().ok_or_else(|| Error::ToolExecutionFailed {
            name: "calculator".to_string(),
            message: "missing or invalid 'b' parameter".to_string(),
        })?;
        Ok(format!("{}", a + b))
    }
}
```

### Registry Usage Example

```rust
use agentic_framework::{ToolRegistry, Tool, Model, Message, ModelOptions, ModelResponse};

async fn example(model: &dyn Model) -> Result<()> {
    // Build registry
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Calculator))?;

    // Get tool definitions for the LLM
    let definitions = registry.definitions();

    // Send conversation with tools
    let messages = vec![Message::user("What is 2 + 3?")];
    let response = model.chat_with_tools(&messages, &definitions, &ModelOptions::default()).await?;

    // Handle tool calls
    match response {
        ModelResponse::ToolCalls(calls) => {
            let results = registry.dispatch_all(&calls).await?;
            // results is Vec<Message> with role=Tool, ready to append to conversation
        }
        ModelResponse::Text(text) => {
            // Model responded with text, no tool calls
        }
    }

    Ok(())
}
```

### Error Handling in Tools

```rust
#[async_trait]
impl Tool for FileReader {
    // ... name, description, parameters ...

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().ok_or_else(|| Error::ToolExecutionFailed {
            name: "file_reader".to_string(),
            message: "missing 'path' parameter".to_string(),
        })?;

        std::fs::read_to_string(path).map_err(|e| Error::ToolExecutionFailed {
            name: "file_reader".to_string(),
            message: format!("failed to read file: {e}"),
        })
    }
}
```

## Discretion Recommendations

These are recommendations for the areas marked as "Claude's discretion" in CONTEXT.md.

### 1. Async Execute Method -- RECOMMENDED: async

**Recommendation:** Use `async fn execute(&self, args: Value) -> Result<String>`

**Rationale:**
- The codebase already uses `async_trait` for `Model` -- consistency matters in an educational project
- Tools that call external APIs (HTTP requests, database queries) need async
- An async tool can trivially wrap sync code; a sync tool cannot call async code without blocking
- The `ToolRegistry::dispatch` method is already async (it builds Messages), so async execute is natural
- **Confidence: HIGH** -- this is a straightforward extension of existing patterns

### 2. Separate Trait Methods -- RECOMMENDED: Separate methods with default definition()

**Recommendation:** Use individual methods `name()`, `description()`, `parameters()` plus a default `definition()` method

**Rationale:**
- More explicit -- implementors see exactly what metadata they need to provide
- The default `definition()` method eliminates boilerplate while keeping individual methods accessible
- Matches how the Rust standard library designs traits (individual methods, not metadata structs)
- A metadata struct approach (`fn metadata(&self) -> ToolMetadata`) hides what's needed behind an intermediate type
- Educational value: shows how default trait methods can compose other trait methods
- **Confidence: HIGH** -- clear educational advantage

### 3. Max Iteration Safeguard -- RECOMMENDED: No framework-level safeguard

**Recommendation:** Do not add a max iteration safeguard in Phase 3

**Rationale:**
- Phase 3 is explicitly single-step dispatch -- the framework does NOT loop. The caller controls looping.
- A max iteration safeguard implies the framework is looping, which contradicts the phase boundary
- If Phase 4 (Workflows) adds agent loops, iteration limits belong there, where the looping actually happens
- Adding it at the registry level conflates dispatch (Phase 3) with orchestration (Phase 4)
- **Confidence: HIGH** -- follows directly from the "single-step dispatch" locked decision

### 4. Tool Execute Input Type -- RECOMMENDED: serde_json::Value

**Recommendation:** `async fn execute(&self, args: serde_json::Value) -> Result<String>`

**Rationale:**
- `ToolCall.arguments` is `String` (raw JSON from the API). Someone must parse it.
- Parsing once at the dispatch layer (in `ToolRegistry::dispatch`) is better than every tool repeating `serde_json::from_str`
- `Value` gives tools structured access to arguments (`args["key"]`, `args.get("key")`)
- The alternative (passing raw `String`) forces every tool to start with JSON parsing boilerplate
- Rig's dyn-safe `ToolDyn` trait uses `String` input, but their design must handle type-erased generics. Our simpler design benefits from `Value`.
- **Confidence: HIGH** -- clear practical advantage, reduces boilerplate

### 5. Tool Execute Output Type -- RECOMMENDED: String

**Recommendation:** `async fn execute(&self, args: Value) -> Result<String>`

**Rationale:**
- OpenAI Chat Completions API tool result message `content` field is a **string** (verified from official docs and cookbook examples)
- `Message::tool_result()` already takes `content: impl Into<String>`
- Returning `String` means zero conversion at the dispatch/message boundary
- Returning `serde_json::Value` would require `.to_string()` serialization before creating the tool result message -- unnecessary
- Tools that compute structured data can `serde_json::to_string(&result)` in their execute method
- Rig's dyn-safe `ToolDyn` also returns `String`
- **Confidence: HIGH** -- directly matches API wire format

### 6. Schema Validation Before Execute -- RECOMMENDED: No framework validation

**Recommendation:** Do not validate tool arguments against the JSON schema before calling execute

**Rationale:**
- Adding the `jsonschema` crate (v0.38+, requires Rust 1.83+) is a significant new dependency for a feature that provides marginal value
- LLMs with structured outputs (OpenAI's `strict: true`) already validate against the schema server-side
- Tools should validate their own arguments in `execute()` and return meaningful errors (more educational: shows error handling patterns)
- Schema validation catches structural issues but not semantic issues -- tools need their own validation regardless
- Keeping the dispatch layer thin (parse JSON, call execute, build message) is more educational
- If validation is desired later, it can be added as an opt-in middleware without changing the Tool trait
- **Confidence: HIGH** -- YAGNI principle, avoids dependency, more educational

## Error Enum Considerations

### Existing Variants (Already Defined)

The `Error` enum already has the tool-related variants needed:

```rust
// In src/error.rs (already exists)
#[error("tool not found: {0}")]
ToolNotFound(String),

#[error("tool execution failed: {name}: {message}")]
ToolExecutionFailed { name: String, message: String },
```

### New Variant Needed: DuplicateTool

A new variant is needed for the duplicate registration error (locked decision: error on duplicate names):

```rust
#[error("duplicate tool name: {0}")]
DuplicateTool(String),
```

This fits naturally in the "Tool errors" comment group in `error.rs`.

## Integration Points with Existing Code

Phase 3 connects to several existing types and should NOT modify them:

| Existing Type | Location | How Phase 3 Uses It |
|---------------|----------|---------------------|
| `ToolCall` | `src/types.rs` | Input to `ToolRegistry::dispatch()` -- provides `id`, `name`, `arguments` |
| `ToolDefinition` | `src/types.rs` | Output of `Tool::definition()` default method; passed to `Model::chat_with_tools()` |
| `ModelResponse::ToolCalls` | `src/types.rs` | Checked by caller to decide whether to call `dispatch_all()` |
| `Message::tool_result()` | `src/message.rs` | Used by `ToolRegistry::dispatch()` to build result messages |
| `Error::ToolNotFound` | `src/error.rs` | Returned by `ToolRegistry::get()` when tool name not found |
| `Error::ToolExecutionFailed` | `src/error.rs` | Returned by tools when execution fails; also used for JSON parse errors |
| `Model::chat_with_tools()` | `src/model.rs` | Caller uses `registry.definitions()` to get tool defs for this method |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Generic associated types on Tool trait | Concrete types (Value/String) with dyn-safe trait | N/A (design choice) | Enables Box<dyn Tool>, simpler lifetime story |
| OpenAI function_call (singular) | OpenAI tool_calls (plural, parallel) | 2023-11 (OpenAI API update) | Multiple tool calls per response; sequential execution per locked decision |
| Schema validation required | LLM structured outputs (strict: true) | 2024 (OpenAI structured outputs) | Server-side validation reduces need for client-side schema checking |
| async-trait crate | Still required for dyn-safe async traits | Ongoing (native dyn async traits not stable) | No timeline for native support; async-trait remains the standard |

## Open Questions

1. **Should `ToolRegistry` implement `Default`?**
   - Likely yes, for ergonomics: `ToolRegistry::default()` equivalent to `ToolRegistry::new()`
   - Low risk, can add during implementation
   - Recommendation: Yes, derive Default

2. **Should `dispatch` return `Message` or `String`?**
   - Returning `Message` (via `Message::tool_result()`) is more useful -- caller can directly append to conversation
   - Returning just `String` would require caller to construct the Message themselves
   - Recommendation: Return `Message` -- encapsulates the full round-trip

3. **Should the registry provide a convenience method to convert to `&[ToolDefinition]`?**
   - `definitions()` returns `Vec<ToolDefinition>` which can be borrowed as `&[ToolDefinition]` for `chat_with_tools()`
   - Could cache definitions, but premature optimization for an educational codebase
   - Recommendation: Return `Vec<ToolDefinition>`, no caching

## Sources

### Primary (HIGH confidence)
- Existing codebase analysis: `src/types.rs` (ToolCall, ToolDefinition), `src/error.rs` (ToolNotFound, ToolExecutionFailed), `src/message.rs` (Message::tool_result), `src/model.rs` (Model trait with async_trait)
- OpenAI function calling cookbook (https://developers.openai.com/cookbook/examples/how_to_call_functions_with_chat_models) -- tool result message format
- Azure OpenAI function calling docs (https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/function-calling) -- confirmed tool result content is String

### Secondary (MEDIUM confidence)
- Rig framework Tool/ToolDyn traits (https://docs.rs/rig-core/latest/rig/tool/trait.Tool.html, https://docs.rs/rig-core/latest/rig/tool/trait.ToolDyn.html) -- design reference for dyn-safe tool traits
- Rig tools concept docs (https://docs.rig.rs/docs/concepts/tools) -- tool registration and agent integration patterns
- Will Crichton's Registry Pattern (https://willcrichton.net/rust-api-type-patterns/registries.html) -- HashMap-based registry design

### Tertiary (LOW confidence)
- jsonschema crate (https://github.com/Stranger6667/jsonschema) -- evaluated and rejected for Phase 3
- Niko Matsakis "box box box" post on dyn async traits (https://smallcultfollowing.com/babysteps/blog/2025/03/24/box-box-box/) -- confirms native dyn async still not stable

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- zero new dependencies, all existing crates
- Architecture: HIGH -- direct extension of existing Model trait pattern; verified against codebase
- Discretion recommendations: HIGH -- all backed by API documentation and codebase consistency
- Pitfalls: HIGH -- drawn from real Rust trait design constraints and OpenAI API requirements

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain, no expected breaking changes)
