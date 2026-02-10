# Phase 1: Core Abstractions - Research

**Researched:** 2026-02-10
**Domain:** Rust async trait design, LLM API type modeling, structured error handling
**Confidence:** HIGH

## Summary

Phase 1 defines the foundational types that the entire library builds against: the `Model` trait (async, dyn-dispatchable), message/response types modeled after the OpenAI chat completions API, and a structured error hierarchy using `thiserror`. The key technical decisions are (1) how to make an async trait dyn-safe, (2) how to represent LLM responses that can be either text or tool calls, and (3) how to structure tool-call metadata fields so round-trip tool calling works in Phase 3.

The async trait + dyn dispatch question is the most consequential decision. Native `async fn` in traits is stable (since Rust 1.75), but `dyn Trait` with async methods is NOT stable and has no timeline. The `async-trait` crate (v0.1.89) remains the standard workaround and is the correct choice for this educational library. Manual desugaring is possible but adds boilerplate that obscures the educational goal.

**Primary recommendation:** Use `async-trait` for the Model trait (teaches a real-world pattern), `thiserror` 2.x for errors, and an enum-based response type (`ModelResponse::Text` / `ModelResponse::ToolCalls`) that matches how OpenAI actually returns data (content is null when tool_calls are present, and vice versa).

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Message Types:**
- Role enum + content struct: `Message { role: Role, content: String }` where Role is System/User/Assistant/Tool
- Text-only content (plain `String`, no multimodal Content enum)
- Separate types for input messages and output responses -- makes data flow direction explicit in the type system

**Model Trait Shape:**
- Separate methods: `chat(messages)` and `chat_with_tools(messages, tools)` -- simpler signatures for simple cases
- Model name/identifier is a constructor config detail, not exposed on the trait
- Dynamic dispatch via `Box<dyn Model>` -- open to extension, teaches the trait object pattern

**Response Representation:**
- Tool call arguments carried as raw JSON string: `ToolCall { name: String, arguments: String }` -- parsing deferred to the tool
- No usage metadata (token counts, model name) in the response -- keep it minimal, just content

**Error Hierarchy:**
- Single flat enum with variants for every failure mode (ApiError, InvalidWorkflow, ToolNotFound, etc.)
- Source error chaining via thiserror's `#[from]` -- wraps reqwest::Error, serde_json::Error, etc.
- Library exposes `pub type Result<T> = std::result::Result<T, Error>` alias

### Claude's Discretion

- Exact model options/parameters struct design (temperature, max_tokens, etc.)
- Whether response is enum (Text vs ToolCalls) or struct with Option fields -- pick based on what OpenAI actually returns
- Message metadata fields needed for tool call round-trips (tool_call_id, name on tool role messages)

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope.

</user_constraints>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.49 | Async runtime | The Rust async runtime; required by reqwest and all downstream async code |
| serde | 1.0.228 | Serialization framework | Universal Rust serialization; derive macros for JSON round-tripping |
| serde_json | 1.0.149 | JSON parsing/generation | Standard JSON crate; needed for tool call arguments (raw JSON strings) |
| thiserror | 2.0.18 | Error derive macros | Generates Display, Error, From impls with minimal boilerplate |
| async-trait | 0.1.89 | Async fn in dyn-safe traits | Required for `Box<dyn Model>` -- native dyn async traits are NOT stable |

### Supporting (Phase 1 only defines types; these are used in later phases but declared now)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest | 0.13.2 | HTTP client | Phase 2 (OpenAI provider) -- listed here for `#[from]` error wrapping |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| async-trait | Manual desugaring (`fn chat(...) -> Pin<Box<dyn Future<Output=...> + Send + '_>>`) | No macro dependency, but verbose boilerplate obscures educational clarity. async-trait IS the community standard and teaches a real pattern. |
| async-trait | trait-variant 0.1.2 | Experimental, not widely adopted, limited docs. Not appropriate for educational code. |
| async-trait | Wait for native dyn async traits | No stable timeline. The Rust lang team is still in design phase (Niko Matsakis "box box box" post, March 2025). Could be years away. |
| thiserror 2.x | thiserror 1.x | v2 is current (2.0.18), v1 is legacy. v2 improves format arg capture and is the actively maintained line. |
| thiserror | anyhow | anyhow is for applications, thiserror is for libraries. This is a library. |

**Installation (Phase 1 Cargo.toml):**
```toml
[package]
name = "agentic-framework"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.49", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
async-trait = "0.1"
reqwest = { version = "0.13", features = ["json"], default-features = false }
```

Note: reqwest 0.13 changed defaults (rustls instead of native-tls, aws-lc instead of ring). Using `default-features = false` with explicit feature selection avoids pulling in unnecessary crypto backends during Phase 1. Phase 2 will finalize reqwest feature flags.

## Architecture Patterns

### Recommended Project Structure

```
src/
├── lib.rs           # Re-exports, crate-level docs
├── model.rs         # Model trait, ModelOptions, ModelResponse
├── message.rs       # Role, Message (input), tool call round-trip metadata
├── error.rs         # Error enum, Result alias
└── types.rs         # ToolCall, ToolDefinition (shared value types)
```

Flat module structure -- no nested directories for Phase 1. Each file is small and focused. Later phases add modules alongside these (e.g., `provider/`, `tool.rs`, `workflow/`).

### Pattern 1: Async Trait with Dynamic Dispatch

**What:** Define `Model` as an `#[async_trait]` trait so it can be used as `Box<dyn Model>`.
**When to use:** Any trait that will have multiple implementations selected at runtime (OpenAI, Gemini, mock).
**Example:**

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Model: Send + Sync {
    /// Send messages and get a text or tool-call response.
    async fn chat(
        &self,
        messages: &[Message],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;

    /// Send messages with tool definitions available for the model to call.
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;
}
```

The `#[async_trait]` macro desugars each async fn to return `Pin<Box<dyn Future<Output = Result<ModelResponse>> + Send + '_>>`, which makes the trait dyn-safe. The `Send + Sync` supertraits ensure `Box<dyn Model>` can be shared across async tasks.

### Pattern 2: Enum-Based Response (Recommended)

**What:** Model the response as an enum with Text and ToolCalls variants, not a struct with Option fields.
**Why:** OpenAI's chat completions API returns EITHER content (text) OR tool_calls, never both simultaneously. When `finish_reason` is `"stop"`, content is populated and tool_calls is null. When `finish_reason` is `"tool_calls"`, tool_calls is populated and content is null. An enum makes this mutual exclusivity type-safe.

```rust
/// Response from a Model. Either text content or one or more tool calls.
#[derive(Debug, Clone)]
pub enum ModelResponse {
    /// The model returned a text completion.
    Text(String),
    /// The model is requesting one or more tool calls.
    ToolCalls(Vec<ToolCall>),
}

impl ModelResponse {
    /// Returns the text content if this is a Text response.
    pub fn text(&self) -> Option<&str> {
        match self {
            ModelResponse::Text(s) => Some(s),
            ModelResponse::ToolCalls(_) => None,
        }
    }

    /// Returns the tool calls if this is a ToolCalls response.
    pub fn tool_calls(&self) -> Option<&[ToolCall]> {
        match self {
            ModelResponse::Text(_) => None,
            ModelResponse::ToolCalls(calls) => Some(calls),
        }
    }
}
```

### Pattern 3: Owned Types at Async Boundaries

**What:** Use `String`, `Vec`, and owned types everywhere in async trait signatures. Never use `&str` or borrowed types in the trait definition.
**Why:** Async functions capture references for the lifetime of the future. Owned types avoid lifetime entanglement and make `Box<dyn Model>` straightforward to use. The messages parameter uses `&[Message]` (borrowed slice of owned messages) which is fine -- the slice borrow is for the duration of the call, not stored.

### Anti-Patterns to Avoid

- **Generic return types on the trait:** Don't make `chat()` return `impl Future` -- this makes the trait non-dyn-safe. Let `#[async_trait]` handle the desugaring.
- **Lifetimes in Message types:** Don't use `Message<'a>` with borrowed content. Educational code should not fight the borrow checker at API boundaries.
- **God struct for responses:** Don't use `struct ModelResponse { content: Option<String>, tool_calls: Option<Vec<ToolCall>> }` -- this allows invalid states (both None, both Some) that the enum prevents.
- **Stringly-typed roles:** Don't use `role: String`. Use an enum. The set of roles is fixed by the API spec.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Error boilerplate | Manual Display/Error/From impls | `thiserror` derive | Dozens of lines per variant; easy to get wrong |
| Async dyn dispatch | Manual `Pin<Box<dyn Future>>` return types | `async-trait` macro | Verbose, error-prone lifetime annotations; the macro is battle-tested |
| JSON serialization | Custom parsing/formatting | `serde` + `serde_json` | JSON parsing has edge cases (unicode escapes, number precision) |
| Result type alias | Repeating `std::result::Result<T, crate::Error>` | `pub type Result<T>` alias | Standard library pattern, reduces noise |

**Key insight:** Phase 1 is pure type definitions with no runtime behavior. The "don't hand-roll" items are about derive macros and type-level utilities, not runtime libraries.

## Common Pitfalls

### Pitfall 1: Forgetting Send + Sync bounds on the Model trait

**What goes wrong:** `Box<dyn Model>` cannot be sent between async tasks or held across await points.
**Why it happens:** `#[async_trait]` adds `Send` to the future, but the trait itself also needs `Send + Sync` supertraits for the trait object to be usable in async contexts.
**How to avoid:** Always define `pub trait Model: Send + Sync`.
**Warning signs:** Compiler errors about `dyn Model` not implementing `Send` when storing in structs or passing to `tokio::spawn`.

### Pitfall 2: Missing tool_call_id on tool result messages

**What goes wrong:** The OpenAI API rejects tool result messages that don't include the `tool_call_id` field matching the original tool call's `id`.
**Why it happens:** The `Message` struct only has `role` and `content`, missing the metadata fields needed for tool call round-trips.
**How to avoid:** Add `tool_call_id: Option<String>` and `name: Option<String>` fields to `Message`, or create a richer message variant. See the Message Metadata section below.
**Warning signs:** Phase 3 (tool system) fails because there's no way to construct valid tool result messages.

### Pitfall 3: Using thiserror #[from] with multiple variants wrapping the same source type

**What goes wrong:** Compile error -- `From<reqwest::Error>` can only be implemented once.
**Why it happens:** Two error variants both have `#[from] reqwest::Error`.
**How to avoid:** Only one variant per source error type gets `#[from]`. Others use `#[source]` (for chaining without automatic From) or manual construction.
**Warning signs:** Compiler error: "conflicting implementations of trait `From<reqwest::Error>`".

### Pitfall 4: Making ToolCall/ToolDefinition non-Clone

**What goes wrong:** Can't pass tool definitions to multiple calls or store tool calls for later processing.
**Why it happens:** Forgetting `#[derive(Clone)]` on value types.
**How to avoid:** Derive `Debug, Clone, PartialEq` on all value types (Message, ToolCall, ToolDefinition, ModelResponse). These are data types, not resources.
**Warning signs:** Borrow checker fights when trying to use tool definitions in multiple places.

### Pitfall 5: thiserror v2 implicit argument capture

**What goes wrong:** Error messages reference fields that don't exist or reference the wrong field due to implicit capture.
**Why it happens:** thiserror 2.x supports implicit named argument capture (like `format!`), so `{name}` in `#[error("...")]` captures a field named `name` rather than requiring explicit positional arguments.
**How to avoid:** Be deliberate with field names in error messages. Use `{0}` for tuple variants.
**Warning signs:** Unexpected content in error Display output.

## Code Examples

### Message Types with Tool Call Metadata

Based on the OpenAI chat completions API, tool call round-trips require these metadata fields:

```rust
use serde::{Deserialize, Serialize};

/// Roles in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation.
///
/// For most messages, only `role` and `content` are needed.
/// Tool-related fields are used during tool call round-trips:
/// - Assistant messages may carry `tool_calls` (populated by the provider from the API response)
/// - Tool result messages require `tool_call_id` and `name` to match the original call
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// The role of the message sender.
    pub role: Role,
    /// The text content of the message.
    pub content: String,
    /// Tool calls requested by the assistant (only on Role::Assistant messages).
    /// Populated by the provider when parsing an API response with tool calls.
    pub tool_calls: Option<Vec<ToolCall>>,
    /// The ID of the tool call this message responds to (only on Role::Tool messages).
    pub tool_call_id: Option<String>,
    /// The name of the tool (only on Role::Tool messages).
    pub name: Option<String>,
}
```

**Why these fields:** The OpenAI API requires:
1. When the assistant requests tool calls, the response message contains `tool_calls: [{id, type, function: {name, arguments}}]`
2. When sending tool results back, each message needs `role: "tool"`, `tool_call_id: "call_xxx"`, `name: "function_name"`, `content: "result"`
3. The full assistant message (including its tool_calls) must be included in the conversation history for the next request

Convenience constructors keep the common case simple:

```rust
impl Message {
    /// Create a simple message with just role and content.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Create a tool result message.
    pub fn tool_result(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }
}
```

### ToolCall and ToolDefinition Types

```rust
/// A tool call requested by the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Unique ID for this tool call (used to match results back).
    /// Corresponds to OpenAI's `tool_calls[].id` field.
    pub id: String,
    /// The name of the function to call.
    pub name: String,
    /// The arguments as a raw JSON string. Parsing is deferred to the tool.
    pub arguments: String,
}

/// A tool definition sent to the model so it knows what tools are available.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    /// The name of the tool/function.
    pub name: String,
    /// A description of what the tool does (helps the model decide when to use it).
    pub description: String,
    /// JSON Schema describing the tool's parameters.
    pub parameters: serde_json::Value,
}
```

**Note on ToolCall.id:** The user's locked decision specified `ToolCall { name, arguments }` without an `id` field. However, the OpenAI API assigns each tool call a unique `id` (like `"call_abc123"`) that MUST be echoed back in the tool result message's `tool_call_id`. Without this field, tool call round-trips in Phase 3 will fail. The `id` field is essential for correctness.

### ModelOptions (Claude's Discretion)

```rust
/// Options for controlling model behavior.
///
/// Uses the builder-lite pattern: construct with defaults, override what you need.
#[derive(Debug, Clone)]
pub struct ModelOptions {
    /// Sampling temperature (0.0 = deterministic, 2.0 = maximum randomness).
    /// None means use the model's default.
    pub temperature: Option<f64>,
    /// Maximum number of tokens to generate in the response.
    /// None means use the model's default.
    pub max_tokens: Option<u32>,
}

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            temperature: None,
            max_tokens: None,
        }
    }
}

impl ModelOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}
```

Keep this minimal. Only `temperature` and `max_tokens` for v1. These are the two parameters that nearly every LLM API supports. Other parameters (top_p, frequency_penalty, etc.) can be added later without breaking changes since all fields are `Option`.

### Error Hierarchy

```rust
use thiserror::Error;

/// All errors produced by the agentic-framework library.
#[derive(Debug, Error)]
pub enum Error {
    // --- Runtime errors (things that go wrong at call time) ---

    /// HTTP request to the LLM API failed.
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

    /// Failed to parse the API response.
    #[error("failed to parse API response: {0}")]
    ResponseParse(#[from] serde_json::Error),

    /// The API returned an error status with a message.
    #[error("API error ({status}): {message}")]
    ApiResponse {
        status: u16,
        message: String,
    },

    /// The API response was missing expected content.
    #[error("unexpected API response: {0}")]
    UnexpectedResponse(String),

    // --- Tool errors ---

    /// A tool requested by the model was not found.
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    /// A tool failed during execution.
    #[error("tool execution failed: {name}: {message}")]
    ToolExecutionFailed {
        name: String,
        message: String,
    },

    // --- Framework errors (structural/configuration problems) ---

    /// The workflow definition is invalid (e.g., contains cycles).
    #[error("invalid workflow: {0}")]
    InvalidWorkflow(String),

    /// A workflow step referenced a dependency that doesn't exist.
    #[error("missing dependency: step '{step}' depends on '{dependency}' which does not exist")]
    MissingDependency {
        step: String,
        dependency: String,
    },

    /// Configuration error (e.g., missing API key).
    #[error("configuration error: {0}")]
    Config(String),
}

/// Convenience Result alias for this library.
pub type Result<T> = std::result::Result<T, Error>;
```

**Design notes:**
- `Api` and `ResponseParse` use `#[from]` for automatic conversion with `?`
- `ApiResponse` is separate from `Api` -- it represents a successful HTTP request that returned an error status (e.g., 401, 429), while `Api` covers connection/network failures
- Framework errors (InvalidWorkflow, MissingDependency) are structurally distinct from runtime errors (Api, ToolNotFound) per QLT-02
- All variants defined upfront even though some (ToolNotFound, InvalidWorkflow) won't be used until later phases -- this is deliberate so downstream phases don't need to modify the error enum

### Complete Model Trait

```rust
use async_trait::async_trait;

/// A model that can generate chat completions.
///
/// Implementations handle the details of communicating with a specific
/// LLM API (OpenAI, Gemini, etc.). Consumer code programs against this
/// trait and uses `Box<dyn Model>` for runtime provider selection.
#[async_trait]
pub trait Model: Send + Sync {
    /// Send a conversation and get a response.
    async fn chat(
        &self,
        messages: &[Message],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;

    /// Send a conversation with tool definitions available for the model to call.
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;
}
```

## OpenAI Chat Completions API Reference

This section documents the OpenAI chat completions API structures that Phase 2 will serialize/deserialize. Phase 1 doesn't implement HTTP calls, but the library's types are designed to map cleanly to these structures.

### Request: Tool Definitions

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get current weather for a location",
        "parameters": {
          "type": "object",
          "properties": {
            "location": { "type": "string", "description": "City name" }
          },
          "required": ["location"]
        }
      }
    }
  ]
}
```

Maps to: `ToolDefinition { name, description, parameters: serde_json::Value }`

### Response: Text Completion

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "The weather is sunny.",
      "tool_calls": null
    },
    "finish_reason": "stop"
  }]
}
```

Maps to: `ModelResponse::Text("The weather is sunny.".into())`

### Response: Tool Calls

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "get_weather",
          "arguments": "{\"location\": \"San Francisco\"}"
        }
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```

Maps to: `ModelResponse::ToolCalls(vec![ToolCall { id: "call_abc123".into(), name: "get_weather".into(), arguments: "{\"location\": \"San Francisco\"}".into() }])`

### Tool Result Message (sent in next request)

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "name": "get_weather",
  "content": "{\"temp\": 72, \"condition\": \"sunny\"}"
}
```

Maps to: `Message::tool_result("call_abc123", "get_weather", "{\"temp\": 72, ...}")`

### Key Observation: content vs tool_calls Mutual Exclusivity

When `finish_reason` is `"stop"`: `content` is a string, `tool_calls` is null.
When `finish_reason` is `"tool_calls"`: `content` is null, `tool_calls` is an array.
This confirms the enum-based `ModelResponse` design is correct.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `async-trait` required for any async trait | Native `async fn` in traits (static dispatch only) | Rust 1.75, Dec 2023 | `async-trait` still needed for dyn dispatch; static dispatch no longer needs it |
| thiserror 1.x | thiserror 2.x | Late 2024 | v2 improves format arg capture, adds no-std support. Non-breaking for users since thiserror doesn't appear in public API |
| reqwest 0.12 (native-tls default) | reqwest 0.13 (rustls default, aws-lc) | Early 2026 | Feature flags changed; `default-features = false` recommended if you want explicit control |
| OpenAI `functions`/`function_call` params | `tools`/`tool_choice` params | 2023-2024 | Old params deprecated. Always use `tools` array format |
| OpenAI Chat Completions API only | Responses API introduced alongside | 2025 | Chat Completions API still fully supported; Responses API is newer but different shape. This library targets Chat Completions for simplicity and broad compatibility. |

**Deprecated/outdated:**
- `functions` and `function_call` parameters in OpenAI API: replaced by `tools` and `tool_choice`
- thiserror 1.x: still works but 2.x is the maintained line
- reqwest 0.12: 0.13 is current with different TLS defaults

## Open Questions

1. **Should ToolCall include an `id` field?**
   - What we know: OpenAI assigns each tool call an `id` (e.g., `"call_abc123"`) that MUST be echoed back in tool result messages via `tool_call_id`. Without it, tool calling round-trips are impossible.
   - What's unclear: The user's locked decision specified `ToolCall { name, arguments }` without `id`. This appears to be an oversight since the field is functionally required.
   - Recommendation: Include `id: String` on ToolCall. Flag this to the user during planning -- it's a necessary addition for correctness, not a design change.

2. **Should Message have serde derives?**
   - What we know: Message is an internal library type, not directly serialized to/from JSON. The OpenAI provider in Phase 2 will map Message to its own request/response serde structs.
   - What's unclear: Whether adding Serialize/Deserialize now helps or hurts.
   - Recommendation: Skip serde derives on Message for now. Add them later if needed. The provider will have its own API-specific serde types. Keeps Phase 1 focused on the domain model.

3. **Default implementation for chat_with_tools?**
   - What we know: `chat_with_tools` could have a default impl that ignores the tools parameter and calls `chat`. This would make implementing simple models easier.
   - What's unclear: Whether a default impl is helpful or misleading for an educational library.
   - Recommendation: No default impl. Both methods must be implemented. This makes the contract explicit and avoids subtle bugs where a provider silently ignores tools.

## Sources

### Primary (HIGH confidence)
- [OpenAI Chat Completions API Reference](https://platform.openai.com/docs/api-reference/chat) -- response structure, tool_calls format
- [Microsoft Azure OpenAI Function Calling Docs](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/function-calling) -- verified tool calling JSON structures with exact field names
- [Mirascope OpenAI Function Calling Guide](https://mirascope.com/blog/openai-function-calling) -- confirmed request/response JSON format
- [docs.rs/thiserror/2.0.18](https://docs.rs/thiserror/latest/thiserror/) -- current thiserror API and attributes
- [docs.rs/async-trait/0.1.89](https://docs.rs/async-trait/latest/async_trait/) -- macro expansion pattern, Send bounds
- [crates.io API](https://crates.io) -- verified current versions: tokio 1.49.0, reqwest 0.13.2, serde 1.0.228, serde_json 1.0.149, thiserror 2.0.18, async-trait 0.1.89

### Secondary (MEDIUM confidence)
- [Rust Blog: Stabilizing async fn in traits](https://blog.rust-lang.org/inside-rust/2023/05/03/stabilizing-async-fn-in-trait.html) -- native async trait history
- [Niko Matsakis: Dyn async traits, part 10](https://smallcultfollowing.com/babysteps/blog/2025/03/24/box-box-box/) -- confirms dyn async traits still in design phase as of March 2025
- [Comprehensive Rust: async traits](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/async-traits.html) -- educational reference on the problem space
- [GitHub: dtolnay/thiserror releases](https://github.com/dtolnay/thiserror/releases) -- thiserror 2.0 changelog
- [reqwest CHANGELOG.md](https://github.com/seanmonstar/reqwest/blob/HEAD/CHANGELOG.md) -- reqwest 0.13 breaking changes

### Tertiary (LOW confidence)
- [State of the Crates 2025](https://ohadravid.github.io/posts/2024-12-state-of-the-crates/) -- ecosystem overview, general version trends

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all versions verified against crates.io API, all crates are mature and stable
- Architecture: HIGH -- async-trait pattern is well-documented; OpenAI API structure verified from multiple official sources; enum-based response matches API behavior
- Pitfalls: HIGH -- based on well-known Rust async patterns and verified OpenAI API requirements
- OpenAI API format: HIGH -- cross-verified across Microsoft docs, Mirascope guide, and OpenAI reference

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain; crate versions may bump but APIs won't change)
