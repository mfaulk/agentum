# Phase 6: Examples - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Create exactly 3 working example programs (simple LLM call, tool calling, multi-step workflow) that demonstrate every major framework concept. Each example is a standalone file in the `examples/` directory. Example tools are defined inline. This phase does not add new library functionality.

</domain>

<decisions>
## Implementation Decisions

### Example structure & layout
- Standard Rust `examples/` directory — each `.rs` file is a standalone binary (`cargo run --example name`)
- Exactly 3 examples matching the success criteria: simple call, tool calling, workflow
- Example tools defined inline in each example file — fully self-contained, no shared modules
- Each example has a top-of-file doc comment block explaining what it demonstrates, plus inline comments at key decision points

### Example tool design
- Tool choice: Claude's discretion on which tools best demonstrate the Tool trait (calculator and weather are suggestions, not requirements)
- JSON parameter schemas written by hand using `serde_json::json!({...})` — nothing hidden behind macros or derives
- Tool-calling example must handle both paths: tool-call response AND plain-text response — shows the full ModelResponse enum usage

### Running experience
- Check for `OPENAI_API_KEY` at startup; print a clear, helpful error message if missing, then exit (don't panic)
- Formatted output with section headers and labels (e.g., `--- Simple Chat ---\nResponse: ...`) so terminal output tells a story
- All 3 examples use real LLM calls via OpenAI (no mock models) — requires API key to run
- Workflow example uses a concrete, realistic use case (e.g., summarize-then-translate, extract-then-format) rather than abstract step names

### Claude's Discretion
- Which specific example tools to create (calculator, weather, or alternatives)
- Whether tools return real computed results vs realistic fake data
- The specific concrete use case for the workflow example
- Exact formatting style for terminal output

</decisions>

<specifics>
## Specific Ideas

- Tool-calling example should demonstrate both response paths (tool call and plain text) — reader learns how the ModelResponse enum works in practice
- Workflow example should feel like something a developer might actually build, not a toy

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-examples*
*Context gathered: 2026-02-10*
