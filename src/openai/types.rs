//! OpenAI Chat Completions API wire-format types.
//!
//! These types define exactly how Rust data serializes to / deserializes from
//! the JSON that OpenAI's API expects and returns. They are intentionally
//! separate from the library's public types (`Message`, `ModelResponse`, etc.)
//! so that wire-format concerns (field names, null handling, optional fields)
//! stay isolated from the ergonomic API surface.
//!
//! The core library `Message` type does NOT derive `Serialize`/`Deserialize`
//! (per 01-01 decision). The provider is responsible for converting between
//! `Message` and `ChatMessage` at the boundary.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Request types (Serialize only -- sent to OpenAI)
// ---------------------------------------------------------------------------

/// Top-level request body for `POST /v1/chat/completions`.
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

/// A single message in the conversation, using OpenAI's role-based tagging.
///
/// Serialized with `#[serde(tag = "role")]` so each variant becomes a JSON
/// object with `"role": "<variant>"` plus variant-specific fields.
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

/// A tool the model may call, wrapping a function definition.
#[derive(Debug, Serialize)]
pub(crate) struct ChatTool {
    /// Always `"function"` for the current API.
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatFunction,
}

/// Function metadata sent in a tool definition.
#[derive(Debug, Serialize)]
pub(crate) struct ChatFunction {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the function's parameters.
    pub parameters: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Response types (Deserialize only -- received from OpenAI)
// ---------------------------------------------------------------------------

/// Top-level response from `POST /v1/chat/completions`.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

/// A single completion choice.
#[derive(Debug, Deserialize)]
pub(crate) struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

/// The assistant message inside a response choice.
///
/// `content` is `Option<String>` because OpenAI returns `"content": null`
/// when the response consists entirely of tool calls.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallWire>>,
}

/// Token usage statistics for the request.
#[derive(Debug, Deserialize)]
pub(crate) struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Shared types (both Serialize and Deserialize)
// ---------------------------------------------------------------------------

/// A tool call as represented in OpenAI's wire format.
///
/// Used in both requests (assistant messages echoing previous tool calls)
/// and responses (new tool calls from the model).
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ToolCallWire {
    pub id: String,
    /// Always `"function"` for the current API.
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCallWire,
}

/// The function name and arguments inside a tool call.
///
/// `arguments` is a raw JSON string (not parsed) -- OpenAI sends it as a
/// string that the caller is expected to parse with `serde_json::from_str`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FunctionCallWire {
    pub name: String,
    pub arguments: String,
}

// ---------------------------------------------------------------------------
// Error response types (Deserialize only)
// ---------------------------------------------------------------------------

/// Wrapper for OpenAI API error responses.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

/// The error body inside an API error response.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorBody {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}
