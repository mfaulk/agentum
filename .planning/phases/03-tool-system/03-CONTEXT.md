# Phase 3: Tool System - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the Tool trait so developers can create tools the LLM can call. Build a ToolRegistry for collecting and looking up tools by name. Implement dispatch that locates a tool, executes it, and returns the result. Handle the tool-calling round trip at the single-step level — the caller controls looping. Creating workflows or multi-step orchestration is Phase 4.

</domain>

<decisions>
## Implementation Decisions

### Tool trait shape
- Sync vs async execute: **Claude's discretion** — pick what fits the codebase (async_trait is already used for Model)
- Parameter schema declared by returning `serde_json::Value` — developer builds JSON schema explicitly, no magic
- Tool must be **Send + Sync** — consistent with Model trait, required for async task sharing
- Separate methods vs metadata struct: **Claude's discretion** — pick what reads best as educational Rust code

### Dispatch & looping
- **Single-step dispatch** — framework executes one round of tool calls and returns. Caller decides whether to loop back to the LLM
- **Sequential execution** when model returns multiple tool calls — one at a time, in order
- **Fail fast** on tool error — stop immediately, don't execute remaining tools in a multi-tool response
- Max iteration safeguard: **Claude's discretion** — determine if any safeguard belongs at the framework level

### Parameter & result types
- Tool execute input type: **Claude's discretion** — pick the most educational and practical option
- Tool execute output type: **Claude's discretion** — pick based on what OpenAI's API expects for tool results
- Tool errors use **Result with the framework's Error type** — consistent single error enum, add ToolExecution variant if needed
- Argument validation (malformed LLM args): **Claude's discretion** — pick what's most educational and practical

### Tool registry design
- **ToolRegistry struct** — dedicated struct that holds tools, handles name lookup, validates no duplicates
- **Error on duplicate names** at registration time — fail-fast, prevents subtle bugs
- Registry **owns tools via Box<dyn Tool>** — simple lifetime story, tools live as long as the registry
- Dispatch is a **method on ToolRegistry** — `registry.dispatch(tool_call)`, encapsulated and discoverable

### Claude's Discretion
- Async vs sync execute method (likely async given existing async_trait usage)
- Separate trait methods vs metadata struct grouping
- Max iteration safeguard at framework level
- Tool execute input type (serde_json::Value vs String)
- Tool execute return type (String vs serde_json::Value)
- Whether framework validates args against schema before calling execute

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-tool-system*
*Context gathered: 2026-02-10*
