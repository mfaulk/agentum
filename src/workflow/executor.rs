//! Workflow execution: runs steps in topological order with data flow.
//!
//! The executor walks the pre-computed topological order from [`Workflow`],
//! executing each step and collecting outputs. Each step receives the outputs
//! of its upstream dependencies as a [`StepInput`] HashMap, enabling data
//! to flow through the DAG.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::message::Message;
use crate::model::Model;
use crate::tool::ToolRegistry;
use crate::types::{ModelOptions, ModelResponse};

use super::step::{Step, StepInput};
use super::workflow::Workflow;

impl Workflow {
    /// Execute the workflow by running each step in topological order.
    ///
    /// For each step, the executor:
    /// 1. Gathers outputs from the step's upstream dependencies into a [`StepInput`].
    /// 2. Runs the step (LLM call or transform).
    /// 3. Stores the output for downstream steps.
    ///
    /// Returns a `HashMap<String, Value>` containing every step's output, keyed
    /// by step name. Callers can read whichever outputs they need (typically the
    /// final step's output).
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidWorkflow`] if an LLM model returns tool calls but no
    ///   tools are configured for that step.
    /// - Any error from a transform closure or model call propagates up.
    pub async fn execute(&self) -> Result<HashMap<String, Value>> {
        let mut outputs: HashMap<String, Value> = HashMap::new();

        for step_name in self.execution_order() {
            // 1. Gather inputs from upstream dependencies.
            let mut inputs: StepInput = HashMap::new();
            for dep_name in self.dependencies(step_name) {
                if let Some(dep_output) = outputs.get(&dep_name) {
                    inputs.insert(dep_name, dep_output.clone());
                }
            }

            // 2. Get the step and execute it based on its variant.
            let step = self
                .step(step_name)
                .expect("execution_order contains only valid step names");

            let output = match step {
                Step::Transform { transform } => transform(&inputs)?,
                Step::Llm {
                    model,
                    prompt_builder,
                    tools,
                    options,
                } => {
                    execute_llm_step(
                        model.as_ref(),
                        prompt_builder.as_ref(),
                        tools,
                        options,
                        &inputs,
                    )
                    .await?
                }
            };

            // 3. Store the output for downstream steps.
            outputs.insert(step_name.clone(), output);
        }

        Ok(outputs)
    }
}

/// Execute a single LLM step: build prompt, call model, handle response.
///
/// This is a private helper that keeps the main `execute` method focused on
/// orchestration. It handles both text responses and tool call responses.
async fn execute_llm_step(
    model: &dyn Model,
    prompt_builder: &(dyn Fn(&StepInput) -> String + Send + Sync),
    tools: &Option<ToolRegistry>,
    options: &ModelOptions,
    inputs: &StepInput,
) -> Result<Value> {
    // 1. Build the prompt from upstream step outputs.
    let prompt = prompt_builder(inputs);

    // 2. Create the message list for the model.
    let messages = vec![Message::user(prompt)];

    // 3. Call the model, optionally with tool definitions.
    let response = match tools {
        Some(registry) => {
            model
                .chat_with_tools(&messages, &registry.definitions(), options)
                .await?
        }
        None => model.chat(&messages, options).await?,
    };

    // 4. Handle the model's response.
    match response {
        ModelResponse::Text(text) => Ok(Value::String(text)),
        ModelResponse::ToolCalls(calls) => {
            // The model wants to call tools -- dispatch them via the registry.
            let registry = tools.as_ref().ok_or_else(|| {
                Error::InvalidWorkflow(
                    "model returned tool calls but no tools configured".into(),
                )
            })?;
            let results = registry.dispatch_all(&calls).await?;
            let result_values: Vec<Value> = results
                .iter()
                .map(|m| Value::String(m.content.clone()))
                .collect();
            Ok(Value::Array(result_values))
        }
    }
}
