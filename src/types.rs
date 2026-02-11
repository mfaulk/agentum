/// A tool call requested by the model.
///
/// When the model decides to use a tool, it returns one or more `ToolCall` values.
/// Each carries a unique `id` (assigned by the API, e.g., `"call_abc123"`), the
/// tool `name`, and raw JSON `arguments` as a string.
///
/// The `id` must be echoed back in the corresponding tool result message so the
/// API can match results to requests.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// A tool definition describing a tool available to the model.
///
/// The `parameters` field holds a JSON Schema value describing the tool's
/// expected input. Using `serde_json::Value` keeps the schema flexible --
/// the framework passes it through without parsing or validating.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// The response from a model after processing a conversation.
///
/// Models return either text content or tool call requests, never both
/// (matching the OpenAI API's mutual exclusivity). This enum encodes
/// that constraint at the type level.
#[derive(Debug, Clone)]
pub enum ModelResponse {
    /// A text response from the model.
    Text(String),
    /// One or more tool calls requested by the model.
    ToolCalls(Vec<ToolCall>),
}

impl ModelResponse {
    /// Returns the text content if this is a `Text` response.
    pub fn text(&self) -> Option<&str> {
        match self {
            ModelResponse::Text(s) => Some(s),
            ModelResponse::ToolCalls(_) => None,
        }
    }

    /// Returns the tool calls if this is a `ToolCalls` response.
    pub fn tool_calls(&self) -> Option<&[ToolCall]> {
        match self {
            ModelResponse::Text(_) => None,
            ModelResponse::ToolCalls(calls) => Some(calls),
        }
    }
}

/// Options for controlling model behavior.
///
/// Uses a builder-lite pattern for ergonomic construction:
///
/// ```rust
/// use agentic_framework::types::ModelOptions;
///
/// let opts = ModelOptions::new()
///     .with_temperature(0.7)
///     .with_max_tokens(1024);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ModelOptions {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
}

impl ModelOptions {
    /// Create a new `ModelOptions` with all fields set to `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the temperature parameter for controlling randomness.
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum number of tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}
