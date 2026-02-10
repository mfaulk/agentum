# Phase 1: Core Abstractions - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the Model trait, message types, response types, and structured error hierarchy that the entire library builds on. This is the API contract — everything downstream (OpenAI provider, tool system, workflow engine) programs against these types. No provider implementations in this phase.

</domain>

<decisions>
## Implementation Decisions

### Message Types
- Role enum + content struct: `Message { role: Role, content: String }` where Role is System/User/Assistant/Tool
- Text-only content (plain `String`, no multimodal Content enum)
- Separate types for input messages and output responses — makes data flow direction explicit in the type system

### Model Trait Shape
- Separate methods: `chat(messages)` and `chat_with_tools(messages, tools)` — simpler signatures for simple cases
- Model name/identifier is a constructor config detail, not exposed on the trait
- Dynamic dispatch via `Box<dyn Model>` — open to extension, teaches the trait object pattern

### Response Representation
- Tool call arguments carried as raw JSON string: `ToolCall { name: String, arguments: String }` — parsing deferred to the tool
- No usage metadata (token counts, model name) in the response — keep it minimal, just content

### Error Hierarchy
- Single flat enum with variants for every failure mode (ApiError, InvalidWorkflow, ToolNotFound, etc.)
- Source error chaining via thiserror's `#[from]` — wraps reqwest::Error, serde_json::Error, etc.
- Library exposes `pub type Result<T> = std::result::Result<T, Error>` alias

### Claude's Discretion
- Exact model options/parameters struct design (temperature, max_tokens, etc.)
- Whether response is enum (Text vs ToolCalls) or struct with Option fields — pick based on what OpenAI actually returns
- Message metadata fields needed for tool call round-trips (tool_call_id, name on tool role messages)

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The key constraint is educational clarity: a Rust developer reading the trait definitions should understand the full LLM interaction contract without needing to look at any provider implementation.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-core-abstractions*
*Context gathered: 2026-02-10*
