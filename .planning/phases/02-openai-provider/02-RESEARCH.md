# Phase 2: OpenAI Provider - Research

**Researched:** 2026-02-10
**Domain:** OpenAI Chat Completions API integration via reqwest in Rust
**Confidence:** HIGH

## Summary

This phase implements an OpenAI provider struct that implements the existing `Model` trait (from Phase 1) using raw HTTP via reqwest against the OpenAI Chat Completions API. The research covers the exact API request/response JSON formats, serde type mapping patterns, reqwest TLS configuration (currently broken -- no TLS backend enabled), and error handling strategies.

The OpenAI Chat Completions API (`POST https://api.openai.com/v1/chat/completions`) is a stable, well-documented JSON API. It is NOT being deprecated -- OpenAI has stated "we intend to continue supporting this API indefinitely." The newer Responses API is a separate product; Chat Completions remains the standard for basic model interaction. Tool calling is built into Chat Completions via the `tools` request parameter and `tool_calls` response field.

The main implementation challenge is the serde type layer: the OpenAI wire format differs from the internal `Message` type (which deliberately does NOT derive Serialize/Deserialize). The provider must define its own serde structs for the API request/response and convert to/from the internal types. The assistant message `content` field is nullable (null when tool_calls are present), which requires `Option<String>` on the response side.

**Primary recommendation:** Define provider-specific serde request/response structs in a dedicated `openai` module, convert to/from internal types, use reqwest `Client` with explicit `rustls` TLS feature, and handle errors by checking status before consuming the response body.

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.13 | HTTP client for API calls | De facto Rust async HTTP client |
| serde | 1.0 | Serialization for API request/response types | Standard Rust serialization |
| serde_json | 1.0 | JSON serialization/deserialization | Required for JSON API |
| async-trait | 0.1 | Model trait impl | Already used for trait definition |
| tokio | 1.49 | Async runtime | Already used |
| thiserror | 2.0 | Error types | Already used for Error enum |

### Cargo.toml Change Required

**CRITICAL:** The current Cargo.toml has `default-features = false` for reqwest, which disables ALL TLS backends. HTTPS calls to `api.openai.com` will fail at runtime without a TLS backend.

**Current (broken for HTTPS):**
```toml
reqwest = { version = "0.13", features = ["json"], default-features = false }
```

**Required fix:**
```toml
reqwest = { version = "0.13", features = ["json", "rustls"], default-features = false }
```

This adds the `rustls` TLS backend (reqwest 0.13's default TLS, using AWS-LC-RS crypto provider with platform certificate verification) while keeping `default-features = false` to avoid pulling in `charset`, `http2`, and `system-proxy` features that are unnecessary for this use case.

**Why `rustls` over `native-tls`:** reqwest 0.13 defaults to rustls. It is safer (memory-safe TLS implementation) and faster than native-tls for most use cases. The 93% adoption rate among hyper users confirms this is the standard choice. No platform-specific C library dependencies.

### No New Dependencies Needed

No additional crates are required. The existing dependency set (reqwest + serde + serde_json + thiserror + async-trait) is sufficient for the OpenAI provider.

## Architecture Patterns

### Recommended Project Structure
```
src/
├── lib.rs              # Add `pub mod openai;`
├── openai/
│   ├── mod.rs          # Re-exports, OpenAiProvider struct
│   ├── types.rs        # API-specific serde request/response structs
│   └── client.rs       # HTTP client logic (or inline in mod.rs)
├── error.rs            # Existing (may need minor additions)
├── message.rs          # Existing (no changes)
├── model.rs            # Existing (no changes)
└── types.rs            # Existing (no changes)
```

Alternative simpler structure (recommended for this scope):
```
src/
├── lib.rs              # Add `pub mod openai;`
├── openai/
│   ├── mod.rs          # OpenAiProvider struct, Model impl
│   └── types.rs        # API-specific serde request/response structs
├── error.rs            # Existing
├── message.rs          # Existing
├── model.rs            # Existing
└── types.rs            # Existing
```

### Pattern 1: Provider-Specific Serde Types (Wire Format Layer)

**What:** Define separate serde structs that match the exact OpenAI JSON wire format. Convert to/from internal types at the provider boundary.

**Why:** The internal `Message` type deliberately does NOT derive Serialize/Deserialize (decision from 01-01). Each provider owns its own wire format. This keeps the internal types clean and allows different providers to have different JSON shapes.

**Request types needed:**
```rust
// openai/types.rs

#[derive(Debug, Serialize)]
pub(crate) struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "role")]
pub(crate) enum ChatMessage {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "user")]
    User { content: String },
    #[serde(rename = "assistant")]
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCallWire>>,
    },
    #[serde(rename = "tool")]
    Tool {
        content: String,
        tool_call_id: String,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct ChatTool {
    #[serde(rename = "type")]
    pub tool_type: String,  // always "function"
    pub function: ChatFunction,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChatFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

**Response types needed:**
```rust
#[derive(Debug, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,  // null when tool_calls present
    pub tool_calls: Option<Vec<ToolCallWire>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ToolCallWire {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,  // always "function"
    pub function: FunctionCallWire,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FunctionCallWire {
    pub name: String,
    pub arguments: String,  // JSON string, not parsed
}

#[derive(Debug, Deserialize)]
pub(crate) struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// Error response from OpenAI
#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorBody {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}
```

### Pattern 2: Provider Struct with Client Reuse

**What:** The `OpenAiProvider` struct holds a `reqwest::Client` (which pools connections) and configuration.

```rust
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            model,
        }
    }

    /// Create from environment variable OPENAI_API_KEY
    pub fn from_env(model: String) -> crate::error::Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| crate::error::Error::Config(
                "OPENAI_API_KEY environment variable not set".to_string()
            ))?;
        Ok(Self::new(api_key, model))
    }
}
```

**Why:** `reqwest::Client` is designed to be reused. It manages an internal connection pool. Creating a new client per request is wasteful. The struct also encapsulates base_url (enabling Azure OpenAI or other compatible endpoints later) and model name.

### Pattern 3: Error Handling -- Check Status Before Body Consumption

**What:** Read the response status BEFORE consuming the body. If non-success, parse the body as an error response. If success, parse as the success type.

```rust
let response = self.client
    .post(&url)
    .header("Authorization", format!("Bearer {}", self.api_key))
    .header("Content-Type", "application/json")
    .json(&request_body)
    .send()
    .await?;  // Network errors become Error::Api via #[from]

let status = response.status();
if !status.is_success() {
    let body = response.text().await.unwrap_or_default();
    // Try to parse as OpenAI error response
    let message = serde_json::from_str::<ApiErrorResponse>(&body)
        .map(|e| e.error.message)
        .unwrap_or(body);
    return Err(Error::ApiResponse {
        status: status.as_u16(),
        message,
    });
}

let completion: ChatCompletionResponse = response.json().await?;
```

**Why NOT `error_for_status()`:** The `error_for_status()` method consumes the response, so you lose the body. The OpenAI error body contains structured information (error message, type, code) that is essential for good error messages. Always read status first, then conditionally parse the body.

### Pattern 4: Message Conversion (Internal <-> Wire)

**What:** Convert between internal `Message` and wire `ChatMessage` at the provider boundary.

```rust
fn to_chat_message(msg: &Message) -> ChatMessage {
    match msg.role {
        Role::System => ChatMessage::System {
            content: msg.content.clone(),
        },
        Role::User => ChatMessage::User {
            content: msg.content.clone(),
        },
        Role::Assistant => ChatMessage::Assistant {
            content: if msg.content.is_empty() { None } else { Some(msg.content.clone()) },
            tool_calls: msg.tool_calls.as_ref().map(|calls| {
                calls.iter().map(|tc| ToolCallWire {
                    id: tc.id.clone(),
                    call_type: "function".to_string(),
                    function: FunctionCallWire {
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    },
                }).collect()
            }),
        },
        Role::Tool => ChatMessage::Tool {
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
        },
    }
}
```

### Anti-Patterns to Avoid

- **Deriving Serialize/Deserialize on internal Message:** Decision 01-01 explicitly forbids this. Providers own their own wire types.
- **Using `error_for_status()` for API errors:** Consumes the response body, losing the structured error information from OpenAI.
- **Creating a new `reqwest::Client` per request:** Wastes connection pooling. Store client in the provider struct.
- **Hardcoding the base URL without allowing override:** Makes Azure OpenAI or proxy setups impossible. Store base_url in the struct.
- **Panicking on API errors:** All errors must surface as `Error` enum variants, not panics.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client | Custom TCP/TLS handling | reqwest::Client | Connection pooling, TLS, encoding, compression all handled |
| JSON serialization | Manual string building | serde_json with derive | Type-safe, handles escaping, null/optional fields correctly |
| TLS | OpenSSL bindings | reqwest with rustls feature | Memory-safe, no C dependencies, platform cert verification |
| Error types | String-based errors | Existing Error enum with thiserror | Already defined, structured, typed |
| Connection pooling | Custom pool | reqwest::Client (built-in) | Client reuse gives free connection pooling |

**Key insight:** The entire HTTP stack is handled by reqwest. The implementation work is purely in the type mapping layer (internal types <-> JSON wire format) and the conversion logic.

## Common Pitfalls

### Pitfall 1: No TLS Backend Enabled
**What goes wrong:** HTTPS requests to api.openai.com fail at runtime with a connection error.
**Why it happens:** `default-features = false` in Cargo.toml disables all TLS backends. The `json` feature does not include TLS.
**How to avoid:** Add `"rustls"` to reqwest features. This is the FIRST thing to fix.
**Warning signs:** Runtime error on first API call, not a compile error.

### Pitfall 2: Assistant Message Content is Nullable
**What goes wrong:** Deserialization fails when the model returns tool calls because `content` is `null` in the JSON.
**Why it happens:** OpenAI returns `"content": null` (not absent, but explicitly null) when the assistant makes tool calls instead of generating text.
**How to avoid:** Use `Option<String>` for the `content` field in the response message struct. Serde handles JSON `null` -> `None` automatically.
**Warning signs:** `serde_json::Error` when parsing tool call responses.

### Pitfall 3: Tool Calls Have Nested Structure
**What goes wrong:** Tool call deserialization fails because the JSON structure has `tool_calls[].function.name` and `tool_calls[].function.arguments`, not flat fields.
**Why it happens:** OpenAI wraps tool calls in a `{"type": "function", "function": {...}}` structure. The `id` is at the top level, but `name` and `arguments` are inside `function`.
**How to avoid:** Define `ToolCallWire` with nested `FunctionCallWire` struct matching the exact JSON shape. Flatten into internal `ToolCall` during conversion.
**Warning signs:** Deserialization errors or empty tool call names.

### Pitfall 4: Tool Definition Wrapping
**What goes wrong:** OpenAI rejects the request because tools are not in the expected format.
**Why it happens:** The OpenAI API expects `{"type": "function", "function": {"name": ..., "description": ..., "parameters": ...}}`, not a flat tool definition. The internal `ToolDefinition` struct is flat.
**How to avoid:** Wrap each `ToolDefinition` in a `ChatTool` struct during request construction that adds the `"type": "function"` wrapper.
**Warning signs:** 400 Bad Request from the API.

### Pitfall 5: Forgetting skip_serializing_if for Optional Fields
**What goes wrong:** OpenAI receives `"temperature": null` or `"tools": null` in the request, which may cause unexpected behavior or errors.
**Why it happens:** Serde serializes `None` as `null` by default, but OpenAI expects the field to be ABSENT, not null.
**How to avoid:** Use `#[serde(skip_serializing_if = "Option::is_none")]` on all optional request fields (temperature, max_tokens, tools).
**Warning signs:** Subtle API behavior differences or 400 errors.

### Pitfall 6: Error Response Body Lost
**What goes wrong:** API errors surface as generic "request failed: 401" without the helpful message from OpenAI (e.g., "Incorrect API key provided").
**Why it happens:** Using `error_for_status()` or `response.json::<SuccessType>()` on error responses discards the error body.
**How to avoid:** Check `response.status()` first. If non-success, read body as text, try to parse as `ApiErrorResponse`, extract the message.
**Warning signs:** Unhelpful error messages during debugging.

### Pitfall 7: Sending Assistant Tool Call Messages Back
**What goes wrong:** Multi-turn tool call conversations fail because the assistant's tool_calls are not included when sending the message history back to OpenAI.
**Why it happens:** When converting assistant messages with tool_calls back to the wire format, the tool_calls field must be included. If you only send `content`, OpenAI cannot match subsequent tool result messages to the original calls.
**How to avoid:** The `to_chat_message` conversion for `Role::Assistant` must include `tool_calls` when present. The `Message` struct already has `tool_calls: Option<Vec<ToolCall>>` for this purpose.
**Warning signs:** "tool result message without preceding tool call" errors from OpenAI.

## Code Examples

### Complete Model Trait Implementation Pattern
```rust
// Source: derived from OpenAI API docs + existing Model trait

#[async_trait]
impl Model for OpenAiProvider {
    async fn chat(
        &self,
        messages: &[Message],
        options: &ModelOptions,
    ) -> Result<ModelResponse> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.iter().map(to_chat_message).collect(),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            tools: None,
        };
        self.send_request(&request).await
    }

    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ModelOptions,
    ) -> Result<ModelResponse> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.iter().map(to_chat_message).collect(),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            tools: Some(tools.iter().map(to_chat_tool).collect()),
        };
        self.send_request(&request).await
    }
}
```

### Tool Definition Conversion
```rust
fn to_chat_tool(tool: &ToolDefinition) -> ChatTool {
    ChatTool {
        tool_type: "function".to_string(),
        function: ChatFunction {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        },
    }
}
```

### Response Conversion
```rust
fn parse_response(response: ChatCompletionResponse) -> Result<ModelResponse> {
    let choice = response.choices.into_iter().next()
        .ok_or_else(|| Error::UnexpectedResponse(
            "no choices in response".to_string()
        ))?;

    // Check for tool calls first (content is null when tool_calls present)
    if let Some(tool_calls) = choice.message.tool_calls {
        if !tool_calls.is_empty() {
            let calls = tool_calls.into_iter().map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: tc.function.arguments,
            }).collect();
            return Ok(ModelResponse::ToolCalls(calls));
        }
    }

    // Otherwise, extract text content
    let content = choice.message.content
        .ok_or_else(|| Error::UnexpectedResponse(
            "no content or tool_calls in response".to_string()
        ))?;
    Ok(ModelResponse::Text(content))
}
```

### OpenAI API Request Format (Reference)
```json
// POST https://api.openai.com/v1/chat/completions
// Authorization: Bearer sk-...
// Content-Type: application/json

// Basic chat request:
{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello!"}
  ],
  "temperature": 0.7,
  "max_tokens": 1024
}

// Request with tools:
{
  "model": "gpt-4o",
  "messages": [
    {"role": "user", "content": "What's the weather in SF?"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get the current weather",
        "parameters": {
          "type": "object",
          "properties": {
            "location": {"type": "string", "description": "City name"}
          },
          "required": ["location"]
        }
      }
    }
  ]
}
```

### OpenAI API Response Format (Reference)
```json
// Text response:
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1738960610,
  "model": "gpt-4o-2024-08-06",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help?",
        "tool_calls": null
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 13,
    "completion_tokens": 7,
    "total_tokens": 20
  }
}

// Tool call response:
{
  "id": "chatcmpl-def456",
  "object": "chat.completion",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_pOsKdUlqvdyttYB67MOj434b",
            "type": "function",
            "function": {
              "name": "get_weather",
              "arguments": "{\"location\":\"San Francisco\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ]
}

// Error response:
{
  "error": {
    "message": "Incorrect API key provided: sk-...xxxx.",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_api_key"
  }
}
```

### Tool Result Round-Trip Message Format
```json
// After executing the tool, send back:
{
  "role": "tool",
  "tool_call_id": "call_pOsKdUlqvdyttYB67MOj434b",
  "name": "get_weather",
  "content": "{\"temperature\": 72, \"condition\": \"sunny\"}"
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `functions` parameter | `tools` parameter | 2023 (GPT-4 era) | Use `tools` array with `type: "function"` wrapper |
| `function_call` field | `tool_calls` array | 2023 (GPT-4 era) | Response uses `tool_calls` (plural, array), not `function_call` |
| native-tls default | rustls default | reqwest 0.13 (2024) | Use `rustls` feature explicitly with `default-features = false` |
| Chat Completions only | Chat Completions + Responses API | 2025 | Chat Completions NOT deprecated, supported indefinitely. Use Chat Completions for this project. |

**Deprecated/outdated:**
- `functions` request parameter: replaced by `tools` (with `type: "function"` wrapper)
- `function_call` response field: replaced by `tool_calls` array
- native-tls as reqwest default: rustls is now default in 0.13+

## Open Questions

1. **Parallel tool calls handling**
   - What we know: OpenAI can return multiple tool_calls in a single response (parallel function calling). Our `ModelResponse::ToolCalls(Vec<ToolCall>)` already handles this since it's a Vec.
   - What's unclear: Whether we need `tool_choice` parameter support in this phase.
   - Recommendation: Don't add `tool_choice` yet. The default behavior ("auto") is correct. Can be added later as a `ModelOptions` field if needed.

2. **Configurable base URL for Azure / compatible endpoints**
   - What we know: The provider struct should store a `base_url` field. Azure OpenAI and many local LLM servers use the same API format with a different base URL.
   - What's unclear: Whether we need this for v1 scope.
   - Recommendation: Include `base_url` in the struct with a sensible default. It costs nothing and enables flexibility. A constructor like `with_base_url()` is trivial.

3. **Model name validation**
   - What we know: Invalid model names produce an API error at runtime.
   - What's unclear: Whether we should validate model names before sending.
   - Recommendation: Don't validate. Model names change frequently. Let the API reject invalid names and surface as `Error::ApiResponse`.

4. **reqwest `http2` feature**
   - What we know: HTTP/2 is not strictly required for OpenAI API calls but can improve performance with multiplexing.
   - What's unclear: Whether the benefit is material for non-streaming use.
   - Recommendation: Omit for now. HTTP/1.1 works fine. Can be added later if needed.

## Sources

### Primary (HIGH confidence)
- [OpenAI API Reference - Chat Completions](https://platform.openai.com/docs/api-reference/chat) - API format (accessed via search results and cookbook)
- [OpenAI Cookbook - Function Calling](https://developers.openai.com/cookbook/examples/how_to_call_functions_with_chat_models) - Tool calling JSON formats
- [Azure OpenAI Function Calling](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/function-calling) - Complete request/response JSON examples for OpenAI-compatible API
- [reqwest 0.13.2 Cargo.toml](https://docs.rs/crate/reqwest/latest/source/Cargo.toml.orig) - Feature flags, default features
- [reqwest docs](https://docs.rs/reqwest/0.13.1/reqwest/) - Client API, Response methods
- [async-openai source](https://github.com/64bit/async-openai) - Rust serde type patterns for OpenAI types

### Secondary (MEDIUM confidence)
- [reqwest v0.13 announcement](https://seanmonstar.com/blog/reqwest-v013-rustls-default/) - TLS default change details
- [OpenAI Responses vs Chat Completions](https://simonwillison.net/2025/Mar/11/responses-vs-chat-completions/) - Confirmed Chat Completions not deprecated
- [OpenAI Error Codes](https://platform.openai.com/docs/guides/error-codes) - Error response format (via search results)

### Tertiary (LOW confidence)
- None. All findings verified with at least one authoritative source.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - reqwest + serde are already in Cargo.toml, only need rustls feature addition
- Architecture: HIGH - Provider-specific serde types is established pattern (01-01 decision), OpenAI JSON format well-documented
- Pitfalls: HIGH - All pitfalls verified against API documentation and response format specs
- API format: HIGH - Verified against OpenAI cookbook, Azure docs, and async-openai crate source

**Research date:** 2026-02-10
**Valid until:** 2026-04-10 (Chat Completions API is stable; reqwest 0.13 is stable)
