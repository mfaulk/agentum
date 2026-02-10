//! OpenAI Chat Completions API provider.
//!
//! Implements the [`Model`] trait for the OpenAI Chat Completions API,
//! bridging the library's internal types to OpenAI's wire format via HTTP.
//!
//! # Usage
//!
//! ```rust,no_run
//! use agentic_framework::{OpenAiProvider, Model, Message, ModelOptions};
//!
//! # async fn example() -> agentic_framework::Result<()> {
//! let provider = OpenAiProvider::from_env("gpt-4o")?;
//! let messages = vec![Message::user("Hello!")];
//! let response = provider.chat(&messages, &ModelOptions::default()).await?;
//! # Ok(())
//! # }
//! ```

pub(crate) mod types;

use async_trait::async_trait;
use reqwest::Client;

use crate::error::{Error, Result};
use crate::message::{Message, Role};
use crate::model::Model;
use crate::types::{ModelOptions, ModelResponse, ToolCall, ToolDefinition};

use types::{
    ApiErrorResponse, ChatCompletionRequest, ChatCompletionResponse, ChatFunction, ChatMessage,
    ChatTool, ToolCallWire,
};

/// An OpenAI Chat Completions API provider.
///
/// Implements [`Model`] by sending HTTP requests to the OpenAI (or compatible)
/// API and converting between internal library types and the wire format.
///
/// # Construction
///
/// Use [`OpenAiProvider::new`] with an explicit API key, or
/// [`OpenAiProvider::from_env`] to read the key from `OPENAI_API_KEY`:
///
/// ```rust,no_run
/// use agentic_framework::OpenAiProvider;
///
/// // From environment variable:
/// let provider = OpenAiProvider::from_env("gpt-4o").unwrap();
///
/// // With explicit key:
/// let provider = OpenAiProvider::new("sk-...", "gpt-4o");
///
/// // With custom base URL (e.g., Azure OpenAI):
/// let provider = OpenAiProvider::new("sk-...", "gpt-4o")
///     .with_base_url("https://my-proxy.example.com/v1");
/// ```
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiProvider {
    /// Create a new provider with the given API key and model name.
    ///
    /// Uses the default OpenAI base URL (`https://api.openai.com/v1`).
    /// A single `reqwest::Client` is created and reused for connection pooling.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: model.into(),
        }
    }

    /// Create a new provider by reading the API key from the `OPENAI_API_KEY`
    /// environment variable.
    ///
    /// Returns `Error::Config` if the variable is not set.
    pub fn from_env(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            Error::Config("OPENAI_API_KEY environment variable not set".into())
        })?;
        Ok(Self::new(api_key, model))
    }

    /// Override the base URL for custom endpoints (Azure OpenAI, proxies, etc.).
    ///
    /// Returns `self` for builder-style chaining.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Send a chat completion request and return the parsed response.
    ///
    /// Constructs the full endpoint URL from `base_url`, sets authorization
    /// headers, and handles error responses by parsing the API error body.
    async fn send_request(&self, request: &ChatCompletionRequest) -> Result<ModelResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ApiErrorResponse>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            return Err(Error::ApiResponse {
                status: status.as_u16(),
                message,
            });
        }

        let completion: ChatCompletionResponse = response.json().await?;
        parse_response(completion)
    }
}

/// Convert an internal [`Message`] to the OpenAI wire-format [`ChatMessage`].
fn to_chat_message(msg: &Message) -> ChatMessage {
    match msg.role {
        Role::System => ChatMessage::System {
            content: msg.content.clone(),
        },
        Role::User => ChatMessage::User {
            content: msg.content.clone(),
        },
        Role::Assistant => {
            let content = if msg.content.is_empty() {
                None
            } else {
                Some(msg.content.clone())
            };

            let tool_calls = msg.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|tc| ToolCallWire {
                        id: tc.id.clone(),
                        call_type: "function".to_string(),
                        function: types::FunctionCallWire {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect()
            });

            ChatMessage::Assistant {
                content,
                tool_calls,
            }
        }
        Role::Tool => ChatMessage::Tool {
            content: msg.content.clone(),
            tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
        },
    }
}

/// Wrap a [`ToolDefinition`] into OpenAI's nested `{"type": "function", "function": {...}}` format.
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

/// Parse a [`ChatCompletionResponse`] into a [`ModelResponse`].
///
/// Extracts the first choice and converts tool calls or text content into the
/// internal representation. Returns `Error::UnexpectedResponse` if the response
/// has no choices, or if the first choice contains neither content nor tool calls.
fn parse_response(response: ChatCompletionResponse) -> Result<ModelResponse> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| Error::UnexpectedResponse("response contained no choices".into()))?;

    let message = choice.message;

    // Check for tool calls first (they take priority when both are present).
    if let Some(wire_calls) = message.tool_calls {
        if !wire_calls.is_empty() {
            let tool_calls: Vec<ToolCall> = wire_calls
                .into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                })
                .collect();
            return Ok(ModelResponse::ToolCalls(tool_calls));
        }
    }

    // Fall back to text content.
    if let Some(content) = message.content {
        return Ok(ModelResponse::Text(content));
    }

    Err(Error::UnexpectedResponse(
        "response contained neither content nor tool calls".into(),
    ))
}

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
