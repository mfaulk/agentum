//! # Simple Chat Example
//!
//! The simplest possible framework usage: a single LLM call.
//!
//! Demonstrates:
//! - Creating an [`OpenAiProvider`] with an explicit API key
//! - Building a conversation with [`Message::system`] and [`Message::user`]
//! - Configuring model behavior with [`ModelOptions`]
//! - Calling [`Model::chat`] and handling the [`ModelResponse`] enum
//!
//! ## Running
//!
//! ```bash
//! export OPENAI_API_KEY=sk-...
//! cargo run --example simple_chat
//! ```
//!
//! Requires the `OPENAI_API_KEY` environment variable to be set.

use agentic_framework::{Message, Model, ModelOptions, ModelResponse, OpenAiProvider};

#[tokio::main]
async fn main() {
    println!("--- Simple Chat Example ---");
    println!();

    // 1. Check for API key before creating the provider.
    //    We do this manually (instead of using OpenAiProvider::from_env) so we
    //    can print a clear, example-quality error message when the key is missing.
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

    // 2. Create the provider.
    //    We use gpt-4o-mini because it's cost-efficient for examples while
    //    still demonstrating the full API surface.
    let provider = OpenAiProvider::new(api_key, "gpt-4o-mini");

    // 3. Build the conversation.
    //    Every conversation starts with a system message that sets the model's
    //    behavior, followed by one or more user messages. The Message type
    //    provides role-based constructors for ergonomic construction.
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is the capital of France? Answer in one sentence."),
    ];

    // 4. Configure model options.
    //    Temperature controls randomness (0.0 = deterministic, 1.0 = creative).
    //    max_tokens limits the response length.
    let options = ModelOptions::new()
        .with_temperature(0.7)
        .with_max_tokens(256);

    // 5. Call the model.
    //    The Model trait's chat() method is async and returns a Result. We handle
    //    errors explicitly rather than using .unwrap() to show good practice.
    println!("Sending message to model...");
    println!();

    let response = match provider.chat(&messages, &options).await {
        Ok(resp) => resp,
        Err(err) => {
            eprintln!("Error calling model: {err}");
            std::process::exit(1);
        }
    };

    // 6. Handle the response.
    //    ModelResponse is an enum with two variants: Text and ToolCalls.
    //    When calling chat() (without tools), the model always returns Text,
    //    but we match exhaustively because the enum requires it -- this is a
    //    strength of Rust's type system enforcing correct handling.
    match response {
        ModelResponse::Text(text) => {
            println!("Response: {text}");
        }
        ModelResponse::ToolCalls(tool_calls) => {
            // This branch won't be reached when using chat() without tools,
            // but the match must be exhaustive. If it somehow occurs, we
            // print a diagnostic message.
            println!("Unexpected tool calls received ({} calls)", tool_calls.len());
            println!("This should not happen when calling chat() without tools.");
        }
    }

    println!();
    println!("--- Done ---");
}
