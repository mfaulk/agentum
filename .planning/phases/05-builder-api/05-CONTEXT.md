# Phase 5: Builder API - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Fluent builder for constructing validated workflows. Wraps `Workflow::new`'s raw Vec-of-tuples interface with chainable builder methods and build-time validation. Does not add new step types, execution capabilities, or workflow features — only a better construction API.

</domain>

<decisions>
## Implementation Decisions

### Disconnected step detection
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

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The success criteria from the roadmap are the guiding constraint: the builder API should read naturally so a Rust developer can understand workflow structure from the builder calls alone.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 05-builder-api*
*Context gathered: 2026-02-10*
