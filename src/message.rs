use serde::{Deserialize, Serialize};

use crate::types::ToolCall;

/// Represents the role of a participant in a conversation.
///
/// Maps directly to the OpenAI API's `role` field. Serializes to lowercase
/// strings (e.g., `Role::System` becomes `"system"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in a conversation with an LLM.
///
/// The core fields `role` and `content` are always present. The optional
/// metadata fields support tool call round-trips:
///
/// - `tool_calls`: Present on assistant messages when the model requests tool execution.
/// - `tool_call_id`: Present on tool result messages, echoing the ID from the original tool call.
/// - `name`: Present on tool result messages, identifying which tool produced the result.
///
/// Does not derive `Serialize`/`Deserialize` -- this is an internal library type.
/// Provider implementations (e.g., OpenAI) define their own API-specific serde structs.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

impl Message {
    /// Create a new message with the given role and content.
    /// All optional metadata fields are set to `None`.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Create a tool result message.
    ///
    /// This constructor sets the `tool_call_id` and `name` fields required by
    /// the OpenAI API for tool result messages. The `tool_call_id` must match
    /// the `id` from the original `ToolCall` that triggered this result.
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
