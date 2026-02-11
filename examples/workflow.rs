//! # Workflow Example
//!
//! Demonstrates a multi-step pipeline using the [`WorkflowBuilder`] API.
//!
//! **Use case:** Summarize an article, then translate the summary to Spanish.
//!
//! The pipeline has three steps chained together:
//!
//! 1. **inject_article** ([`transform_step`]) -- Injects the article text into
//!    the workflow's data flow as a JSON value.
//! 2. **summarize** ([`llm_step`]) -- Sends the article to an LLM with
//!    instructions to produce a concise summary.
//! 3. **translate** ([`llm_step`]) -- Takes the summary and asks a second LLM
//!    call to translate it into Spanish.
//!
//! Data flows between steps via [`StepInput`] -- each step receives the outputs
//! of its upstream dependencies as a `HashMap<String, Value>`, keyed by step name.
//!
//! ## Running
//!
//! ```bash
//! export OPENAI_API_KEY=sk-...
//! cargo run --example workflow
//! ```
//!
//! Requires the `OPENAI_API_KEY` environment variable to be set.

use agentic_framework::{Model, OpenAiProvider, Workflow};
use serde_json::json;

/// A short article to summarize and translate.
/// In a real application, this could come from a file, database, or API.
const ARTICLE: &str = "\
Rust is a systems programming language focused on safety, speed, and concurrency. \
It achieves memory safety without a garbage collector through its ownership system, \
which tracks how data is used at compile time. Rust's type system and borrow checker \
prevent data races and null pointer dereferences, making it suitable for systems \
where reliability is critical. The language has seen growing adoption in areas like \
web servers, operating systems, game engines, and embedded devices. Major companies \
including Mozilla, Google, Microsoft, and Amazon use Rust in production systems.";

#[tokio::main]
async fn main() {
    println!("--- Workflow Example: Summarize & Translate ---");
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

    // 2. Create two provider instances -- one for each LLM step.
    //    Each llm_step takes ownership of a Box<dyn Model>, so we need
    //    separate instances. Both use gpt-4o-mini for cost efficiency.
    let summarizer: Box<dyn Model> =
        Box::new(OpenAiProvider::new(api_key.clone(), "gpt-4o-mini"));
    let translator: Box<dyn Model> =
        Box::new(OpenAiProvider::new(api_key, "gpt-4o-mini"));

    // 3. Build the workflow using the WorkflowBuilder API.
    //
    //    The builder uses move semantics (consuming self) for all methods,
    //    enabling a fluent chaining style. Steps and edges can be added in
    //    any order -- validation happens entirely at build() time.
    //
    //    Step types:
    //    - transform_step: Pure data transformation (no LLM call)
    //    - llm_step: Calls an LLM with a dynamically built prompt
    //
    //    Note: llm_step() uses ModelOptions::default() internally (no
    //    temperature or max_tokens set). For custom options, use
    //    llm_step_with_tools() which accepts a ModelOptions parameter.
    println!("Building workflow...");

    let workflow = Workflow::builder()
        // Step 1: Inject the article text into the data flow.
        // Transform steps are useful for preparing data before LLM calls.
        .transform_step("inject_article", |_inputs| {
            Ok(json!({ "text": ARTICLE }))
        })
        // Step 2: Summarize the article.
        // The prompt_builder closure receives upstream outputs via StepInput.
        // Here, it reads the article text injected by the previous step.
        .llm_step("summarize", summarizer, |inputs| {
            // Access upstream output by step name. The StepInput is a
            // HashMap<String, Value> where keys are dependency step names.
            let article = inputs
                .get("inject_article")
                .and_then(|v| v["text"].as_str())
                .unwrap_or("No article provided");

            format!(
                "Summarize the following article in 2-3 sentences. \
                 Be concise and capture the key points.\n\n\
                 Article:\n{article}"
            )
        })
        // Step 3: Translate the summary to Spanish.
        // This step depends on the summarize step -- it reads the summary
        // text from the upstream output.
        .llm_step("translate", translator, |inputs| {
            // The LLM step output is stored as a JSON string value.
            let summary = inputs
                .get("summarize")
                .and_then(|v| v.as_str())
                .unwrap_or("No summary available");

            format!(
                "Translate the following text to Spanish. \
                 Output only the translation, nothing else.\n\n\
                 Text:\n{summary}"
            )
        })
        // Chain connects steps in linear order: inject -> summarize -> translate.
        // This is equivalent to calling .edge("inject_article", "summarize")
        // followed by .edge("summarize", "translate").
        .chain(&["inject_article", "summarize", "translate"])
        // Build validates the DAG: checks for cycles, missing steps,
        // disconnected steps, and duplicate names. All errors are collected
        // and reported at once.
        .build();

    let workflow = match workflow {
        Ok(wf) => wf,
        Err(errors) => {
            eprintln!("Workflow build failed with {} error(s):", errors.errors.len());
            for err in &errors.errors {
                eprintln!("  - {err}");
            }
            std::process::exit(1);
        }
    };

    // 4. Execute the workflow.
    //    The executor runs steps in topological order. Each step receives
    //    the outputs of its upstream dependencies as a StepInput HashMap.
    //    The result is a HashMap<String, Value> containing every step's output.
    println!("Executing workflow ({} steps)...", workflow.execution_order().len());
    println!();

    let outputs = match workflow.execute().await {
        Ok(results) => results,
        Err(err) => {
            eprintln!("Workflow execution failed: {err}");
            std::process::exit(1);
        }
    };

    // 5. Display results from each step.
    println!("=== Original Article ===");
    println!("{ARTICLE}");
    println!();

    if let Some(summary) = outputs.get("summarize").and_then(|v| v.as_str()) {
        println!("=== Summary (English) ===");
        println!("{summary}");
        println!();
    }

    if let Some(translation) = outputs.get("translate").and_then(|v| v.as_str()) {
        println!("=== Translation (Spanish) ===");
        println!("{translation}");
        println!();
    }

    // Show execution order for educational purposes.
    println!("Execution order: {:?}", workflow.execution_order());

    println!();
    println!("--- Done ---");
}
