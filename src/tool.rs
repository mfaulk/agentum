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
//! use agentum::{Tool, ToolRegistry};
//! use agentum::error::Result;
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
use crate::message::Message;
use crate::types::{ToolCall, ToolDefinition};

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

    /// Execute a single tool call and return the result as a Message.
    ///
    /// Looks up the tool by name, parses the JSON arguments string into
    /// a `serde_json::Value`, calls the tool's execute method, and wraps
    /// the result in a [`Message::tool_result`] with the correct tool_call_id.
    ///
    /// # Errors
    ///
    /// - [`Error::ToolNotFound`] if no tool with the given name is registered
    /// - [`Error::ResponseParse`] if the arguments string is not valid JSON
    /// - [`Error::ToolExecutionFailed`] if the tool's execute method fails
    pub async fn dispatch(&self, tool_call: &ToolCall) -> Result<Message> {
        // 1. Look up the tool by name.
        let tool = self
            .tools
            .get(&tool_call.name)
            .ok_or_else(|| Error::ToolNotFound(tool_call.name.clone()))?;

        // 2. Parse the JSON arguments string into a serde_json::Value.
        //    The `?` operator uses the `#[from] serde_json::Error` on
        //    Error::ResponseParse for automatic conversion.
        let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)?;

        // 3. Execute the tool, wrapping any error in ToolExecutionFailed
        //    to preserve the tool name for diagnostic context.
        let result = tool
            .execute(args)
            .await
            .map_err(|e| Error::ToolExecutionFailed {
                name: tool_call.name.clone(),
                message: e.to_string(),
            })?;

        // 4. Build and return the tool result message.
        Ok(Message::tool_result(
            tool_call.id.clone(),
            tool_call.name.clone(),
            result,
        ))
    }

    /// Execute multiple tool calls sequentially and return results as Messages.
    ///
    /// Processes tool calls one at a time, in order. Stops immediately on the
    /// first error (fail-fast behavior). Does not execute remaining tool calls
    /// after a failure.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered. Any tool calls after the failing
    /// one are not executed.
    pub async fn dispatch_all(&self, tool_calls: &[ToolCall]) -> Result<Vec<Message>> {
        let mut results = Vec::with_capacity(tool_calls.len());
        for tool_call in tool_calls {
            // The `?` operator provides fail-fast: on first error, we return
            // immediately without executing remaining tool calls.
            results.push(self.dispatch(tool_call).await?);
        }
        Ok(results)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Role;
    use crate::types::ToolCall;

    // Compile-time verification that Tool is dyn-safe.
    // This function never runs -- it just needs to compile.
    fn _assert_tool_is_dyn_safe(_: Box<dyn Tool>) {}

    /// A simple test tool that echoes its input back.
    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes the input message back"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })
        }
        async fn execute(&self, args: serde_json::Value) -> Result<String> {
            let message = args["message"].as_str().unwrap_or("no message");
            Ok(format!("echo: {}", message))
        }
    }

    /// A tool that always fails, for testing error handling.
    struct FailingTool;

    #[async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &str {
            "failing"
        }
        fn description(&self) -> &str {
            "Always fails"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _args: serde_json::Value) -> Result<String> {
            Err(Error::ToolExecutionFailed {
                name: "failing".into(),
                message: "intentional failure".into(),
            })
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        assert!(registry.get("echo").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        let err = registry.register(Box::new(EchoTool)).unwrap_err();
        assert!(matches!(err, Error::DuplicateTool(_)));
    }

    #[test]
    fn test_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        let defs = registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "echo");
        assert_eq!(defs[0].description, "Echoes the input message back");
        assert_eq!(
            defs[0].parameters,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })
        );
    }

    #[tokio::test]
    async fn test_dispatch_success() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "echo".to_string(),
            arguments: r#"{"message":"hello"}"#.to_string(),
        };

        let msg = registry.dispatch(&tool_call).await.unwrap();
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
        assert_eq!(msg.name, Some("echo".to_string()));
        assert_eq!(msg.content, "echo: hello");
    }

    #[tokio::test]
    async fn test_dispatch_tool_not_found() {
        let registry = ToolRegistry::new();

        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "nonexistent".to_string(),
            arguments: "{}".to_string(),
        };

        let err = registry.dispatch(&tool_call).await.unwrap_err();
        assert!(matches!(err, Error::ToolNotFound(ref name) if name == "nonexistent"));
    }

    #[tokio::test]
    async fn test_dispatch_invalid_json() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        let tool_call = ToolCall {
            id: "call_789".to_string(),
            name: "echo".to_string(),
            arguments: "not json".to_string(),
        };

        let err = registry.dispatch(&tool_call).await.unwrap_err();
        assert!(matches!(err, Error::ResponseParse(_)));
    }

    #[tokio::test]
    async fn test_dispatch_tool_execution_failure() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(FailingTool)).unwrap();

        let tool_call = ToolCall {
            id: "call_fail".to_string(),
            name: "failing".to_string(),
            arguments: "{}".to_string(),
        };

        let err = registry.dispatch(&tool_call).await.unwrap_err();
        assert!(matches!(err, Error::ToolExecutionFailed { ref name, .. } if name == "failing"));
    }

    #[tokio::test]
    async fn test_dispatch_all_success() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool)).unwrap();

        let tool_calls = vec![
            ToolCall {
                id: "call_a".to_string(),
                name: "echo".to_string(),
                arguments: r#"{"message":"first"}"#.to_string(),
            },
            ToolCall {
                id: "call_b".to_string(),
                name: "echo".to_string(),
                arguments: r#"{"message":"second"}"#.to_string(),
            },
        ];

        let msgs = registry.dispatch_all(&tool_calls).await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "echo: first");
        assert_eq!(msgs[0].tool_call_id, Some("call_a".to_string()));
        assert_eq!(msgs[1].content, "echo: second");
        assert_eq!(msgs[1].tool_call_id, Some("call_b".to_string()));
    }

    #[tokio::test]
    async fn test_dispatch_all_fail_fast() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(FailingTool)).unwrap();
        registry.register(Box::new(EchoTool)).unwrap();

        // FailingTool's call first, then EchoTool's call.
        // dispatch_all should fail on the first call and never reach the second.
        let tool_calls = vec![
            ToolCall {
                id: "call_fail".to_string(),
                name: "failing".to_string(),
                arguments: "{}".to_string(),
            },
            ToolCall {
                id: "call_echo".to_string(),
                name: "echo".to_string(),
                arguments: r#"{"message":"should not run"}"#.to_string(),
            },
        ];

        let err = registry.dispatch_all(&tool_calls).await.unwrap_err();
        assert!(matches!(err, Error::ToolExecutionFailed { ref name, .. } if name == "failing"));
    }
}
