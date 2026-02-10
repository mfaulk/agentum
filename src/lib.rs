//! # Agentic Framework
//!
//! A minimal Rust library for building LLM-powered applications through
//! DAG-based workflows. Provides core abstractions for model interaction,
//! tool calling, and structured error handling.
//!
//! ## Core Types
//!
//! - [`Model`] -- Async trait for LLM chat completions (use as `Box<dyn Model>`)
//! - [`Message`] -- Conversation messages with role-based constructors
//! - [`ModelResponse`] -- Either text content or tool call requests
//! - [`OpenAiProvider`] -- OpenAI Chat Completions API provider
//! - [`Error`] -- Structured error types for all failure modes

pub mod error;
pub mod message;
pub mod model;
pub mod types;

pub mod openai;

// Re-export primary types for convenience.
// Users can `use agentic_framework::{Model, Message, ...}` instead of
// reaching into submodules.
pub use error::{Error, Result};
pub use message::{Message, Role};
pub use model::Model;
pub use openai::OpenAiProvider;
pub use types::{ModelOptions, ModelResponse, ToolCall, ToolDefinition};

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time verification that Model is dyn-safe.
    // This function never runs -- it just needs to compile.
    fn _assert_model_is_dyn_safe(_: Box<dyn Model>) {}
}
