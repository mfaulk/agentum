use async_trait::async_trait;

use crate::error::Result;
use crate::message::Message;
use crate::types::{ModelOptions, ModelResponse, ToolDefinition};

/// A model that can generate chat completions.
///
/// Implementations handle the details of communicating with a specific
/// LLM API (OpenAI, Gemini, etc.). Consumer code programs against this
/// trait and uses `Box<dyn Model>` for runtime provider selection.
///
/// # Methods
///
/// - [`chat`](Model::chat) -- Send a conversation and get a text or tool-call response.
/// - [`chat_with_tools`](Model::chat_with_tools) -- Same, but with tool definitions
///   available for the model to call.
///
/// # Dynamic Dispatch
///
/// This trait is designed for use as `Box<dyn Model>`. The `Send + Sync`
/// supertraits and `#[async_trait]` attribute ensure trait objects can be
/// shared across async tasks.
#[async_trait]
pub trait Model: Send + Sync {
    /// Send a conversation and get a response.
    ///
    /// # Arguments
    /// * `messages` -- The conversation history as a slice of messages.
    /// * `options` -- Model parameters (temperature, max_tokens, etc.).
    ///
    /// # Returns
    /// A `ModelResponse` which is either text content or tool call requests.
    async fn chat(
        &self,
        messages: &[Message],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;

    /// Send a conversation with tool definitions available for the model to call.
    ///
    /// When tools are provided, the model may choose to return `ModelResponse::ToolCalls`
    /// instead of `ModelResponse::Text`. The caller is responsible for executing the
    /// requested tools and sending results back in a subsequent call.
    ///
    /// # Arguments
    /// * `messages` -- The conversation history as a slice of messages.
    /// * `tools` -- Tool definitions describing available functions.
    /// * `options` -- Model parameters (temperature, max_tokens, etc.).
    ///
    /// # Returns
    /// A `ModelResponse` which is either text content or tool call requests.
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ModelOptions,
    ) -> Result<ModelResponse>;
}
