//! Tool trait and registry for LLM-callable tools.
//!
//! This module defines the developer-facing API for creating tools that an LLM
//! can call. Developers implement the [`Tool`] trait for each tool, then
//! register instances with a [`ToolRegistry`]. The registry collects tool
//! definitions for passing to [`Model::chat_with_tools`] and provides lookup
//! for dispatching tool calls back to the correct implementation.
//!
//! # Example
//!
//! ```rust,ignore
//! use async_trait::async_trait;
//! use agentic_framework::{Tool, ToolRegistry};
//! use agentic_framework::error::Result;
//! use serde_json::json;
//!
//! struct Calculator;
//!
//! #[async_trait]
//! impl Tool for Calculator {
//!     fn name(&self) -> &str { "calculator" }
//!     fn description(&self) -> &str { "Evaluates arithmetic expressions" }
//!     fn parameters(&self) -> serde_json::Value {
//!         json!({
//!             "type": "object",
//!             "properties": {
//!                 "expression": { "type": "string" }
//!             },
//!             "required": ["expression"]
//!         })
//!     }
//!     async fn execute(&self, args: serde_json::Value) -> Result<String> {
//!         let expr = args["expression"].as_str().unwrap_or("0");
//!         Ok(format!("Result: {expr}"))
//!     }
//! }
//! ```

use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::{Error, Result};
use crate::types::ToolDefinition;

/// A tool that can be called by an LLM during a conversation.
///
/// Implement this trait for each tool you want to make available. The trait
/// uses `#[async_trait]` with `Send + Sync` supertraits, matching the same
/// pattern as [`crate::model::Model`] -- this ensures tools can be shared
/// across async tasks and stored as trait objects (`Box<dyn Tool>`).
///
/// # Design Notes
///
/// - **Separate methods** (`name`, `description`, `parameters`) rather than a
///   single metadata struct -- reads naturally as educational code and allows
///   the default `definition()` method to assemble them.
/// - **`serde_json::Value` for parameters** -- the JSON Schema is built by the
///   developer using `serde_json::json!()`, keeping things explicit.
/// - **`String` return from `execute`** -- matches the OpenAI tool result
///   content field exactly. Tools stringify their output.
/// - **`Result<String>` for execute** -- uses the framework's [`Error`] type
///   so tool failures are handled uniformly with other errors.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool (e.g., "calculator", "get_weather").
    ///
    /// This name is sent to the model in the tool definition and is used
    /// by the registry for lookup. It must be unique within a registry.
    fn name(&self) -> &str;

    /// A human-readable description of what this tool does.
    ///
    /// Sent to the model to help it decide when to use this tool. Write
    /// this as you would a doc comment -- clear, concise, action-oriented.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    ///
    /// The developer builds this explicitly using `serde_json::json!()`.
    /// This schema is sent to the model so it knows what arguments to
    /// provide when calling the tool.
    fn parameters(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments.
    ///
    /// `args` is the parsed JSON arguments from the model's tool call.
    /// Returns a string result that will be sent back to the model as
    /// the tool's output.
    ///
    /// # Errors
    ///
    /// Return an error if the tool cannot produce a meaningful result.
    /// The framework will convert this into a
    /// [`ToolExecutionFailed`](Error::ToolExecutionFailed) error.
    async fn execute(&self, args: serde_json::Value) -> Result<String>;

    /// Build a [`ToolDefinition`] from this tool's metadata.
    ///
    /// Default implementation assembles a `ToolDefinition` from `name()`,
    /// `description()`, and `parameters()`. Override only if you need
    /// custom behavior (rare).
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters(),
        }
    }
}

/// A registry that owns tools and provides lookup by name.
///
/// `ToolRegistry` stores tools as `Box<dyn Tool>`, which gives a simple
/// lifetime story: tools live as long as the registry. This avoids the
/// complexity of lifetime parameters while still supporting dynamic dispatch.
///
/// The registry serves two purposes:
/// 1. **Definition export** -- `definitions()` produces a `Vec<ToolDefinition>`
///    for passing to [`Model::chat_with_tools()`](crate::model::Model::chat_with_tools).
/// 2. **Tool lookup** -- `get()` retrieves a tool by name so the dispatch
///    layer (Plan 03-02) can call `execute()` on the correct tool.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool with the registry.
    ///
    /// The tool is stored by its `name()`. If a tool with the same name is
    /// already registered, this returns [`Error::DuplicateTool`] instead of
    /// silently overwriting -- catching misconfiguration early.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateTool`] if a tool with the same name is
    /// already registered.
    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<()> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(Error::DuplicateTool(name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Look up a tool by name.
    ///
    /// Returns a reference to the trait object if found. The caller can then
    /// call `execute()` on the returned tool.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Produce tool definitions for all registered tools.
    ///
    /// This is the bridge between the tool registry and the model API:
    /// pass the returned `Vec<ToolDefinition>` to
    /// [`Model::chat_with_tools()`](crate::model::Model::chat_with_tools).
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
