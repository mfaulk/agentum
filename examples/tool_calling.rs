//! # Tool Calling Example
//!
//! Demonstrates the full tool-calling round trip:
//!
//! 1. **Define tools** by implementing the [`Tool`] trait with `name()`,
//!    `description()`, `parameters()` (hand-written JSON Schema), and `execute()`.
//! 2. **Register tools** in a [`ToolRegistry`] so the framework can look them up.
//! 3. **Send tool definitions** to the model via [`Model::chat_with_tools`].
//! 4. **Handle both response paths**: [`ModelResponse::Text`] (model answered
//!    directly) and [`ModelResponse::ToolCalls`] (model wants to call a tool).
//! 5. **Dispatch tool calls** back through the registry and send results to the model.
//!
//! ## Running
//!
//! ```bash
//! export OPENAI_API_KEY=sk-...
//! cargo run --example tool_calling
//! ```
//!
//! Requires the `OPENAI_API_KEY` environment variable to be set.

use agentic_framework::error::Result;
use agentic_framework::{
    Message, Model, ModelOptions, ModelResponse, OpenAiProvider, Tool, ToolRegistry,
};
use async_trait::async_trait;
use serde_json::json;

// ---------------------------------------------------------------------------
// Tool 1: Calculator
// ---------------------------------------------------------------------------

/// A simple arithmetic calculator tool.
///
/// Demonstrates implementing the `Tool` trait with a hand-written JSON Schema
/// for parameters. The model sends an operation name and two numbers; this
/// tool performs the computation and returns the result as a string.
struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Performs basic arithmetic operations (add, subtract, multiply, divide)"
    }

    /// JSON Schema is hand-written -- the framework passes it to the model as-is.
    /// This gives you full control over how the tool's interface is described.
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "The first operand"
                },
                "b": {
                    "type": "number",
                    "description": "The second operand"
                }
            },
            "required": ["operation", "a", "b"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let operation = args["operation"].as_str().unwrap_or("unknown");
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);

        let result = match operation {
            "add" => format!("{}", a + b),
            "subtract" => format!("{}", a - b),
            "multiply" => format!("{}", a * b),
            "divide" => {
                if b == 0.0 {
                    "Error: division by zero".to_string()
                } else {
                    format!("{}", a / b)
                }
            }
            other => format!("Unknown operation: {other}"),
        };

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tool 2: GetWeather
// ---------------------------------------------------------------------------

/// A mock weather tool that returns fabricated weather data.
///
/// In a real application, this would call a weather API. Here we return
/// static data to demonstrate the tool-calling pattern without external
/// dependencies.
struct GetWeather;

#[async_trait]
impl Tool for GetWeather {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "Gets the current weather for a given city (mock data)"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name to get weather for"
                }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String> {
        let city = args["city"].as_str().unwrap_or("Unknown");

        // Mock weather data -- a real tool would call an external API here.
        let weather = match city.to_lowercase().as_str() {
            "paris" => "22C, partly cloudy",
            "tokyo" => "28C, sunny",
            "london" => "15C, rainy",
            "new york" => "25C, clear skies",
            _ => "18C, conditions unknown",
        };

        Ok(format!("Weather in {city}: {weather}"))
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("--- Tool Calling Example ---");
    println!();

    // 1. Check for API key.
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: OPENAI_API_KEY environment variable is not set.");
            eprintln!();
            eprintln!("To run this example, set your OpenAI API key:");
            eprintln!();
            eprintln!("  export OPENAI_API_KEY=sk-...");
            eprintln!();
            eprintln!("You can get an API key at https://platform.openai.com/api-keys");
            std::process::exit(1);
        }
    };

    // 2. Create provider. gpt-4o-mini supports tool calling at low cost.
    let provider = OpenAiProvider::new(api_key, "gpt-4o-mini");

    // 3. Create a ToolRegistry and register both tools.
    //    The registry owns the tools (via Box<dyn Tool>) and provides:
    //    - definitions() for passing to chat_with_tools
    //    - dispatch() for executing tool calls the model requests
    let mut registry = ToolRegistry::new();
    registry
        .register(Box::new(Calculator))
        .expect("calculator registration should succeed (hardcoded name)");
    registry
        .register(Box::new(GetWeather))
        .expect("get_weather registration should succeed (hardcoded name)");

    // 4. Build the conversation with a prompt that should trigger tool use.
    let messages = vec![
        Message::system(
            "You are a helpful assistant with access to a calculator and weather tools. \
             Use the tools when appropriate to answer questions accurately.",
        ),
        Message::user("What is 42 multiplied by 17? Also, what is the weather like in Paris?"),
    ];

    let options = ModelOptions::new()
        .with_temperature(0.0) // Low temperature for deterministic tool selection
        .with_max_tokens(512);

    // 5. Send the message with tool definitions.
    //    The definitions() method produces the Vec<ToolDefinition> that the
    //    model needs to know what tools are available and how to call them.
    println!(
        "Sending message with {} tool definitions...",
        registry.definitions().len()
    );
    println!();

    let response = match provider
        .chat_with_tools(&messages, &registry.definitions(), &options)
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            eprintln!("Error calling model: {err}");
            std::process::exit(1);
        }
    };

    // 6. Handle the response.
    //    ModelResponse has two variants. When tools are provided, the model
    //    may return either one -- you must handle both paths.
    match response {
        // Path A: The model answered directly without calling any tools.
        // This can happen if the model decides it doesn't need tools to answer.
        ModelResponse::Text(text) => {
            println!("[Model responded with text (no tool calls)]");
            println!();
            println!("Response: {text}");
        }

        // Path B: The model wants to call one or more tools.
        // We dispatch each call through the registry, which looks up the tool
        // by name, parses arguments, calls execute(), and wraps the result in
        // a Message::tool_result.
        ModelResponse::ToolCalls(tool_calls) => {
            println!("[Model requested {} tool call(s)]", tool_calls.len());
            println!();

            for (i, call) in tool_calls.iter().enumerate() {
                println!("  Tool call {}: {}({})", i + 1, call.name, call.arguments);
            }
            println!();

            // Dispatch all tool calls through the registry.
            // dispatch_all executes them sequentially and returns Message results.
            let tool_results = match registry.dispatch_all(&tool_calls).await {
                Ok(results) => results,
                Err(err) => {
                    eprintln!("Error dispatching tool calls: {err}");
                    std::process::exit(1);
                }
            };

            println!("Tool results:");
            for result in &tool_results {
                let tool_name = result.name.as_deref().unwrap_or("unknown");
                println!("  {tool_name}: {}", result.content);
            }
            println!();

            // In a real application, you would now send the tool results back
            // to the model in a follow-up chat_with_tools call. The conversation
            // would include:
            // 1. The original messages
            // 2. An assistant message with the tool_calls
            // 3. The tool result messages
            //
            // This creates a multi-turn conversation where the model can
            // incorporate the tool outputs into its final answer.
            println!("[In a full application, these results would be sent back to");
            println!(" the model for a final synthesized response.]");
        }
    }

    println!();
    println!("--- Done ---");
}
