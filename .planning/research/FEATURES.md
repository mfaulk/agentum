# Feature Landscape

**Domain:** Educational Rust LLM Agentic Framework Library
**Researched:** 2026-02-10
**Source basis:** Training data (cutoff ~May 2025). WebSearch and WebFetch were unavailable during this research session. Findings are based on extensive knowledge of LangChain, LangGraph, CrewAI, Rig (Rust), and llm-chain (Rust) from training data. Confidence is MEDIUM overall -- the feature landscape of LLM frameworks is well-established, but specific version details may have shifted.

## What LLM Frameworks Typically Provide

Before categorizing features for this project, here is the full landscape of what established LLM frameworks offer. This provides context for what to include vs exclude.

### Model Abstraction Layer
Every framework provides a unified interface over multiple LLM providers. LangChain abstracts ChatOpenAI, ChatAnthropic, etc. behind a BaseChatModel. Rig (Rust) uses a `CompletionModel` trait. This is the foundation everything else builds on.

**Sub-features:**
- Unified request/response types
- Provider-specific API translation (OpenAI, Anthropic, Gemini all have different JSON shapes)
- Tool calling normalization (each provider formats tool calls differently)
- Structured output / JSON mode
- Token counting and context window management
- Streaming vs batch responses
- Model configuration (temperature, max tokens, stop sequences)

### Prompt Management
Templates, variables, system prompts, few-shot examples. LangChain has PromptTemplate and ChatPromptTemplate. Most Rust frameworks handle this more simply with string formatting.

### Tool / Function Calling
Defining tools the LLM can invoke, sending tool schemas to the LLM, parsing tool call responses, executing tools, returning results. LangChain uses `@tool` decorators. Rig uses derive macros (`#[derive(Tool)]`). This is the heart of agentic behavior.

### Chains / Pipelines
Sequential execution: prompt -> LLM -> parse -> next step. LangChain's original abstraction. llm-chain in Rust follows this model. Simple but limited (no branching).

### Graph/DAG Workflows
LangGraph introduced stateful graph-based workflows with conditional edges, parallel branches, cycles. This is the more powerful evolution of chains. Nodes are functions, edges define data/control flow, state is passed through the graph.

### Agent Loops
Autonomous reasoning: LLM decides what tool to call, observes result, decides next action, repeats until done. ReAct pattern. LangChain agents, CrewAI agents, AutoGen agents. This is the "agentic" core -- but also the most complex and hardest to debug.

### Memory / Conversation History
Maintaining context across turns. Buffer memory, summary memory, vector memory. Critical for chatbots, less relevant for workflow-oriented systems.

### RAG (Retrieval-Augmented Generation)
Document loading, chunking, embedding, vector storage, retrieval, context injection. Rig has a full RAG pipeline. LangChain has extensive RAG tooling.

### Output Parsing / Structured Output
Extracting structured data from LLM responses. JSON parsing, Pydantic models (Python), serde structs (Rust). Rig calls these "extractors."

### Callbacks / Observability
Hooks into execution for logging, tracing, monitoring. LangChain has a callbacks system. LangSmith provides tracing. Important for debugging but not core.

### Multi-Agent Orchestration
Multiple agents collaborating. CrewAI's crews, AutoGen's group chat. Agents can delegate to each other, share context, work on sub-tasks.

---

## Table Stakes

Features users expect from an LLM framework library. Missing these and the library feels incomplete or unusable. Ordered by dependency (build earlier items first).

### TS-1: Model Abstraction Trait

| Aspect | Detail |
|--------|--------|
| **Why expected** | Without this, every workflow step is coupled to a specific provider's API. This IS the framework. |
| **Complexity** | Medium |
| **Educational value** | HIGH -- teaches trait-based polymorphism, async traits, how LLM APIs actually work under the hood |
| **What it includes** | A `Model` or `CompletionModel` trait; request/response types; configuration (temperature, max_tokens); at least two implementations (OpenAI, Gemini) |
| **What it excludes** | Streaming, token counting, embeddings (defer these) |
| **Notes** | The fact that OpenAI and Gemini have different tool-calling JSON shapes makes this genuinely interesting to implement. The abstraction layer has to normalize tool schemas, tool call responses, and tool results across providers. |

### TS-2: HTTP Client Integration

| Aspect | Detail |
|--------|--------|
| **Why expected** | LLM providers are HTTP APIs. The library needs to make authenticated HTTP requests. |
| **Complexity** | Low |
| **Educational value** | LOW -- reqwest is well-known, this is plumbing |
| **What it includes** | Async HTTP client, API key configuration, request serialization, response deserialization |
| **What it excludes** | Retry logic, rate limiting, connection pooling tuning |
| **Notes** | Use reqwest + serde_json. Not interesting in itself but a prerequisite for everything else. Keep thin. |

### TS-3: Tool Definition System

| Aspect | Detail |
|--------|--------|
| **Why expected** | Tool calling is the defining feature of agentic AI. Without tools, you just have a chat wrapper. |
| **Complexity** | Medium |
| **Educational value** | HIGH -- teaches how function calling works end-to-end: defining schemas, sending them to the LLM, parsing calls, dispatching execution, returning results |
| **What it includes** | A `Tool` trait with name/description/schema/execute; JSON Schema generation for tool parameters; a registry of available tools; dispatch logic |
| **What it excludes** | Derive macros for automatic tool generation (nice-to-have, not table stakes) |
| **Notes** | The schema generation is where educational value peaks. Developers see exactly what JSON gets sent to the LLM and how the LLM's response maps back to function calls. Manual tool definition (implementing the trait) is more educational than derive macros. |

### TS-4: Single-Shot Tool Execution

| Aspect | Detail |
|--------|--------|
| **Why expected** | The mechanism of send-tools-to-LLM, get-tool-calls-back, execute-them, return-results-to-LLM is the core agentic pattern. Even without loops, this one-pass version demonstrates the full mechanism. |
| **Complexity** | Medium |
| **Educational value** | HIGH -- shows the complete tool-calling lifecycle in one clear pass |
| **What it includes** | Send prompt + tool definitions to LLM; parse tool call from response; execute the tool; send tool result back to LLM; get final response |
| **What it excludes** | Multi-turn loops, autonomous decision-making about when to stop |
| **Notes** | This is the project's deliberate design choice. Single-shot is enough to teach the mechanism. Loops add complexity without proportional educational value for v1. |

### TS-5: DAG-Based Workflow Engine

| Aspect | Detail |
|--------|--------|
| **Why expected** | This is the project's primary differentiator and the stated core feature. Without it, the library is just another LLM wrapper. |
| **Complexity** | High |
| **Educational value** | VERY HIGH -- teaches graph theory (topological sort), async concurrency (parallel branch execution), data flow patterns, and workflow orchestration |
| **What it includes** | Step definition; edge definition (dependencies); topological sorting; parallel execution of independent steps; data passing between steps; execution tracking |
| **What it excludes** | Conditional edges, cycles, checkpointing, error recovery |
| **Notes** | This is the most architecturally significant feature. The DAG executor needs to: (1) validate the graph has no cycles, (2) determine execution order, (3) run independent branches concurrently, (4) pass outputs from completed steps to dependent steps. Each of these is a distinct, teachable concept. |

### TS-6: Builder Pattern API

| Aspect | Detail |
|--------|--------|
| **Why expected** | Stated project requirement. The primary way users construct workflows. |
| **Complexity** | Medium |
| **Educational value** | MEDIUM -- builder pattern is a known Rust idiom, but applying it to workflow construction is a good demonstration |
| **What it includes** | WorkflowBuilder with methods for adding steps, adding edges, setting data flow; validation at build time; produces an immutable Workflow |
| **What it excludes** | Macro-based DSL, YAML/JSON workflow definitions |
| **Notes** | Type-safe builder that catches errors at compile time where possible (e.g., referencing nonexistent steps). Good Rust-specific educational value. |

### TS-7: Data Flow Between Steps

| Aspect | Detail |
|--------|--------|
| **Why expected** | A DAG without data flow is just task scheduling. The interesting part is how outputs become inputs. |
| **Complexity** | Medium-High |
| **Educational value** | HIGH -- teaches type erasure vs generics tradeoffs, serde-based serialization for inter-step communication, and how to design flexible data passing in a statically typed language |
| **What it includes** | Step output capture; input mapping for downstream steps; a data store or context object that accumulates results; type-safe access patterns |
| **What it excludes** | Schema validation, complex transformations between steps |
| **Notes** | This is where Rust's type system creates interesting design challenges vs Python. In Python you just pass dicts. In Rust you need to decide: serde_json::Value (dynamic), trait objects (type-erased), or generics (compile-time). Each choice teaches something. Recommend serde_json::Value for simplicity with typed accessor helpers. |

### TS-8: Working Examples

| Aspect | Detail |
|--------|--------|
| **Why expected** | An educational library without examples fails its core mission. |
| **Complexity** | Medium (writing good examples is harder than it seems) |
| **Educational value** | CRITICAL -- examples ARE the product for an educational library |
| **What it includes** | At minimum: (1) simple single-step LLM call, (2) tool-calling example, (3) multi-step workflow with parallel branches, (4) multi-model workflow (OpenAI for one step, Gemini for another) |
| **What it excludes** | Full application templates, web server integration |
| **Notes** | Each example should demonstrate one concept clearly. Include extensive comments explaining what happens and why. Examples should be runnable (with API keys). |

### TS-9: Error Handling

| Aspect | Detail |
|--------|--------|
| **Why expected** | LLM APIs fail. Network errors, rate limits, malformed responses, tool execution failures. Without error handling, examples crash unhelpfully. |
| **Complexity** | Medium |
| **Educational value** | MEDIUM -- teaches Rust error handling patterns (thiserror, custom error types, Result propagation) in a real-world context |
| **What it includes** | Custom error enum covering: HTTP errors, API errors (auth, rate limit, invalid request), JSON parse errors, tool execution errors, workflow errors (cycle detected, missing dependency); Display impl for human-readable messages |
| **What it excludes** | Error recovery strategies, retry logic |
| **Notes** | Use thiserror for derive. Good opportunity to teach how to design error types that are informative without being overwhelming. |

---

## Differentiators

Features that set this project apart. Not expected from every LLM framework, but valuable for this project's educational mission and Rust-specific context.

### D-1: Transparent API Request/Response Logging

| Aspect | Detail |
|--------|--------|
| **Value proposition** | Most frameworks hide the actual HTTP requests/responses. For education, showing exactly what JSON goes to the API and what comes back is invaluable. |
| **Complexity** | Low |
| **Educational value** | VERY HIGH -- demystifies LLM APIs completely. Developers see the raw reality. |
| **What it includes** | Optional verbose/debug mode that prints: the exact JSON body sent to the LLM API, the raw response, the parsed tool calls, the tool results sent back |
| **Notes** | Use tracing crate with configurable log levels. At DEBUG level, show full request/response bodies. This costs almost nothing to implement but dramatically increases educational value. |

### D-2: Compile-Time Workflow Validation

| Aspect | Detail |
|--------|--------|
| **Value proposition** | Rust's type system can catch workflow errors at compile time that Python frameworks only catch at runtime. This is THE Rust-specific advantage. |
| **Complexity** | Medium-High |
| **Educational value** | HIGH -- teaches how to encode invariants in Rust's type system |
| **What it includes** | Builder that enforces: all referenced steps exist, no duplicate step names, graph is acyclic (at build time, not runtime where possible via typestate pattern or validation in build()) |
| **Notes** | Full typestate-based compile-time DAG validation is very complex and likely not worth it for v1. But build-time validation (panics or Returns Error at .build() call) is straightforward and still demonstrates the principle. |

### D-3: Explicit Async Concurrency Model

| Aspect | Detail |
|--------|--------|
| **Value proposition** | The DAG executor naturally demonstrates async Rust patterns: spawning tasks, joining futures, sharing state across tasks. Most Python LLM frameworks hide their concurrency. This one makes it visible. |
| **Complexity** | Already included in DAG executor (no extra cost) |
| **Educational value** | HIGH -- teaches tokio::spawn, JoinHandle, Arc/Mutex patterns in a real application |
| **Notes** | The DAG executor IS the async concurrency lesson. Parallel branches = spawned tasks. Data passing = shared state. This is a differentiator because the Rust implementation makes concurrency explicit where Python hides it. |

### D-4: Provider API Comparison Documentation

| Aspect | Detail |
|--------|--------|
| **Value proposition** | Document how OpenAI and Gemini APIs differ and how the abstraction layer normalizes them. This teaches API design. |
| **Complexity** | Low (documentation, not code) |
| **Educational value** | HIGH -- shows why abstraction layers exist by showing the concrete differences they abstract over |
| **Notes** | Include as comments in model implementation files and in the examples README. Show the raw OpenAI format, the raw Gemini format, and how the trait maps between them. |

### D-5: Zero-Magic Implementation

| Aspect | Detail |
|--------|--------|
| **Value proposition** | No proc macros, no hidden codegen, no framework magic. Every abstraction is visible, traceable, understandable. |
| **Complexity** | Negative (it is the ABSENCE of complexity) |
| **Educational value** | VERY HIGH -- the user can read every line of code and understand what it does |
| **What it includes** | Manual trait implementations instead of derive macros for tools; explicit type conversions instead of blanket impls; straightforward module structure |
| **Notes** | This is a deliberate anti-Rig stance. Rig uses derive macros (`#[derive(Tool)]`, `#[derive(Embed)]`) which are convenient but opaque. This project should show what those macros generate, manually. A reader should never wonder "where does this behavior come from?" |

### D-6: Step Type Variety

| Aspect | Detail |
|--------|--------|
| **Value proposition** | Workflows need more than just LLM calls. Having different step types (LLM, transform, conditional) shows how to design an extensible step system. |
| **Complexity** | Medium |
| **Educational value** | HIGH -- teaches enum-based dispatch, trait objects, and how to make a step system extensible |
| **What it includes** | LLM step (calls a model), transform step (pure function on data), prompt-template step (formats input into prompt). Each is a variant showing different patterns. |
| **Notes** | Keep the step types minimal but distinct. The point is showing how the trait/enum design allows extension, not providing every possible step type. |

---

## Anti-Features

Features to deliberately NOT build for v1. These are common in LLM frameworks but would add complexity without proportional educational value, or would conflict with the project's design philosophy.

### AF-1: Autonomous Agent Loops (ReAct Pattern)

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | The observe-think-act loop is the signature "agentic" pattern, but it introduces unbounded execution, halting problems, and debugging nightmares. It obscures the underlying mechanism (tool calling) with control flow complexity. The project explicitly chooses single-shot tool calling to teach the mechanism cleanly. |
| **What to do instead** | Single-shot tool execution demonstrates the SAME mechanism (tool schemas, LLM tool calls, execution, result passing) without the loop complexity. Document why loops exist and how to add them as an extension. |
| **Educational note** | A comment or doc section explaining "here is where you would add a loop if you wanted autonomous agents" is more educational than actually building the loop. |

### AF-2: RAG Pipeline

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | RAG requires: document loading, text chunking, embedding models, vector storage, similarity search, context injection. This is an entire subsystem orthogonal to workflow orchestration. It would double the project scope. |
| **What to do instead** | If RAG-like behavior is needed in an example, fake it: hard-code a "retrieval" step that returns pre-defined context. This shows where RAG would plug in without building it. |

### AF-3: Conversation Memory / Chat History

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Memory management (buffer, summary, sliding window) is relevant for chatbots but orthogonal to DAG-based workflows. The project is workflow-oriented, not conversation-oriented. |
| **What to do instead** | Workflows are stateless between runs by design. Within a workflow run, data flows through the DAG. This IS the "memory" model for workflows. |

### AF-4: Derive Macros for Tool Definition

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Proc macros are powerful but opaque. They hide the very thing this project exists to teach: how tool schemas are constructed and how dispatch works. Rig uses `#[derive(Tool)]` -- this project should show what that macro does. |
| **What to do instead** | Manual trait implementations with clear comments. After v1, a derive macro could be added as a convenience layer that references the manual implementation for understanding. |

### AF-5: Streaming Responses

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Streaming adds significant complexity (SSE parsing, chunked response assembly, partial tool call handling) without teaching core agentic patterns. It is a UX feature, not an architectural one. |
| **What to do instead** | Batch responses only. Document that streaming is a natural extension for production use. |

### AF-6: Retry/Resilience Logic

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Retry policies, exponential backoff, circuit breakers, rate limit handling are production concerns. They add noise to educational code and obscure the core patterns. |
| **What to do instead** | Return errors clearly. Let the caller decide retry strategy. An example could show a simple manual retry wrapper if desired. |

### AF-7: Multiple Output Formats (YAML, XML, Custom Parsers)

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Output parsing is a rabbit hole. JSON mode + serde covers the educational need. Supporting YAML, XML, regex-based extraction, etc. adds breadth without depth. |
| **What to do instead** | JSON output parsing via serde_json. If structured output is needed, use the model's JSON mode or a simple JSON extraction from markdown code blocks. |

### AF-8: Plugin/Extension System

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | A plugin architecture (dynamic loading, registry, lifecycle hooks) is overengineering for a library with two model providers and a handful of step types. It introduces abstraction for abstraction's sake. |
| **What to do instead** | Traits are the extension system. Users implement `Model` for new providers, `Tool` for new tools, and `Step` for new step types. Rust's trait system IS the plugin system. Document this explicitly. |

### AF-9: Web UI, Dashboard, or Visualization

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Completely out of scope. This is a library, not an application. |
| **What to do instead** | Debug logging that shows workflow execution order and data flow. |

### AF-10: Multi-Agent Collaboration (CrewAI-style)

| Anti-Feature | Detail |
|--------------|--------|
| **Why avoid** | Multi-agent systems (delegation, negotiation, shared context) are a superset of what this project teaches. They require agent loops (AF-1) as a prerequisite and add agent-to-agent communication protocols. Way too much scope. |
| **What to do instead** | A DAG workflow where different steps use different models (OpenAI for step 1, Gemini for step 2) gives a taste of "multi-agent" without the orchestration complexity. |

---

## Feature Dependencies

```
TS-2: HTTP Client
  |
  v
TS-1: Model Abstraction Trait  (needs HTTP client to call APIs)
  |
  +--> TS-3: Tool Definition System  (tools need model to send schemas to)
  |      |
  |      v
  |    TS-4: Single-Shot Tool Execution  (needs tools + model)
  |
  +--> TS-9: Error Handling  (used by all features, but defined alongside model)
  |
  v
TS-7: Data Flow Between Steps  (defines how step outputs become inputs)
  |
  v
TS-5: DAG Workflow Engine  (needs steps, data flow, model access)
  |
  v
TS-6: Builder Pattern API  (constructs what the engine executes)
  |
  v
TS-8: Working Examples  (needs everything above to demonstrate)

Differentiators slot in alongside their dependencies:
  D-1 (Logging): alongside TS-1 (model layer)
  D-2 (Compile-time validation): alongside TS-6 (builder)
  D-3 (Async concurrency): inherent in TS-5 (DAG engine)
  D-4 (API comparison docs): alongside TS-1 (model layer)
  D-5 (Zero-magic): design philosophy, applies everywhere
  D-6 (Step type variety): alongside TS-5 (DAG engine)
```

### Critical Path

The dependency chain for the minimum demonstrable library is:

```
HTTP Client -> Model Trait -> Tool System -> Tool Execution -> Data Flow -> DAG Engine -> Builder -> Examples
```

This is largely sequential. Parallelism exists in:
- Error handling can be developed alongside model trait
- Tool system and data flow design can overlap
- Builder API and DAG engine can be co-developed
- Differentiators D-1, D-4 are low-cost additions during model layer work

---

## MVP Recommendation

### Phase 1: Foundation (Model + Tools)

Build first, demonstrate with standalone examples (no DAG yet):

1. **TS-2** HTTP Client integration (reqwest setup)
2. **TS-1** Model abstraction trait with OpenAI implementation
3. **TS-9** Error handling (custom error types)
4. **TS-3** Tool definition system (Tool trait, JSON schema generation)
5. **TS-4** Single-shot tool execution
6. **D-1** Request/response logging (tracing integration)

**Milestone:** A working example that sends a prompt to OpenAI, the LLM calls a tool, the tool executes, and the result is returned. With debug logging showing every HTTP request/response.

### Phase 2: Multi-Model (Gemini + Abstraction Proof)

7. **TS-1** (continued) Gemini implementation of the Model trait
8. **D-4** Provider API comparison documentation

**Milestone:** Same tool-calling example works with both OpenAI and Gemini, demonstrating the abstraction layer's value.

### Phase 3: Workflow Engine

9. **TS-7** Data flow system (step output/input mapping)
10. **TS-5** DAG workflow engine (topological sort, parallel execution)
11. **D-3** Async concurrency patterns (inherent in DAG engine)
12. **D-6** Step type variety (LLM step, transform step)

**Milestone:** A multi-step workflow with parallel branches executes correctly, with data flowing between steps.

### Phase 4: Builder API + Polish

13. **TS-6** Builder pattern API
14. **D-2** Build-time workflow validation
15. **TS-8** Comprehensive examples (4+ examples covering all concepts)
16. **D-5** Code review pass for zero-magic clarity

**Milestone:** Complete library with builder API, comprehensive examples, and thorough documentation.

### Defer to Post-v1

- **AF-1** Agent loops: document as extension point
- **AF-2** RAG: entirely separate concern
- **AF-4** Derive macros: convenience after understanding is established
- **AF-5** Streaming: UX improvement for production use
- Additional model providers (Anthropic, local models)
- Structured output / JSON mode extraction
- Conditional edges in DAG (if/else branching)
- Workflow composition (sub-workflows as steps)

---

## Feature Comparison: This Project vs Existing Frameworks

| Feature | LangChain (Python) | Rig (Rust) | llm-chain (Rust) | This Project |
|---------|-------------------|------------|-------------------|--------------|
| Model abstraction | Yes (many) | Yes (many) | Yes (few) | Yes (2: OpenAI + Gemini) |
| Tool calling | Yes (agent loops) | Yes (derive macro) | No | Yes (single-shot, manual) |
| Workflows/DAG | LangGraph (separate) | No | Sequential only | Yes (core feature) |
| RAG | Yes (extensive) | Yes (built-in) | No | No (deliberate) |
| Memory | Yes (many types) | No | No | No (deliberate) |
| Streaming | Yes | Yes | Yes | No (deliberate) |
| Builder API | No (declarative) | No (struct-based) | No (chain-based) | Yes (core feature) |
| Educational focus | No | No | No | Yes (core mission) |
| Parallel execution | LangGraph | No | No | Yes (DAG branches) |
| Debug logging | LangSmith (paid) | Basic | Basic | Yes (built-in, free) |
| Derive macros | N/A (Python) | Yes | No | No (deliberate -- zero magic) |

### This Project's Unique Position

The niche this library fills is: **"I want to understand how LLM frameworks work by reading clear Rust code."**

No existing framework serves this audience:
- **LangChain** is comprehensive but opaque and Python-only
- **Rig** is Rust but uses derive macros that hide the mechanism
- **llm-chain** is Rust but abandoned (last meaningful update was 2023) and only supports sequential chains
- **LangGraph** has the DAG concept but is Python, complex, and not educational

---

## Educational Value Assessment

| Feature | Rust Concepts Taught | AI/LLM Concepts Taught | Combined Value |
|---------|---------------------|----------------------|----------------|
| Model trait | Async traits, trait objects, generics, serde | LLM API protocols, provider differences | VERY HIGH |
| Tool system | Trait definition, JSON Schema, enum dispatch | Function calling protocol, tool schemas | VERY HIGH |
| DAG engine | Topological sort, async spawn/join, Arc/Mutex | Workflow orchestration, parallel execution | VERY HIGH |
| Builder API | Builder pattern, ownership/move semantics | Workflow definition, validation | HIGH |
| Data flow | Type erasure (Any/serde_json::Value), downcasting | Step composition, data pipelines | HIGH |
| Error handling | thiserror, Result chains, error conversion | API error taxonomy | MEDIUM |
| HTTP client | reqwest, async/await, serde | REST API interaction | LOW-MEDIUM |
| Debug logging | tracing crate, subscriber configuration | Understanding LLM request/response | MEDIUM |

The features with VERY HIGH educational value are the ones where Rust concepts and AI concepts reinforce each other. The model trait teaches traits AND LLM APIs simultaneously. The DAG engine teaches async Rust AND workflow orchestration simultaneously. These dual-learning features should be the priority.

---

## Confidence Notes

| Finding | Confidence | Basis |
|---------|------------|-------|
| LangChain feature set | HIGH | Extensive training data, well-documented framework |
| CrewAI feature set | HIGH | Well-documented, widely discussed in training data |
| LangGraph DAG concepts | HIGH | Well-documented in training data |
| Rig (Rust) features | MEDIUM | Training data; unable to verify current version via web |
| llm-chain status (abandoned) | MEDIUM | Last known activity was 2023 in training data; could not verify current state |
| OpenAI/Gemini API shape differences | HIGH | Direct API knowledge from training data |
| Rust async patterns for DAG execution | HIGH | Well-established Rust patterns |
| Feature categorization (table stakes vs differentiators) | HIGH | Based on project requirements + framework analysis |

## Sources

- LangChain documentation (from training data, ~2024-2025 versions)
- LangGraph documentation (from training data)
- CrewAI documentation (from training data)
- Rig GitHub repository and docs (from training data, ~2024-2025)
- llm-chain GitHub repository (from training data, ~2023)
- OpenAI API documentation (from training data)
- Google Gemini API documentation (from training data)
- Project requirements from `.planning/PROJECT.md`

**Note:** WebSearch and WebFetch were unavailable during this research. All framework-specific claims are based on training data with a cutoff around May 2025. The feature landscape of LLM frameworks is mature enough that the categorizations above are likely still valid, but specific library versions and feature additions after that date are not reflected.
