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
                Error::InvalidWorkflow("model returned tool calls but no tools configured".into())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::error::Result;
    use crate::message::Message;
    use crate::types::{ModelOptions, ModelResponse, ToolDefinition};

    use super::super::step::Step;
    use super::super::workflow::Workflow;

    /// A mock model that returns a fixed text response.
    ///
    /// No HTTP calls are made. This enables integration testing of the
    /// executor's data flow without a real LLM API.
    struct MockModel {
        response: String,
    }

    impl MockModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::model::Model for MockModel {
        async fn chat(
            &self,
            _messages: &[Message],
            _options: &ModelOptions,
        ) -> Result<ModelResponse> {
            Ok(ModelResponse::Text(self.response.clone()))
        }

        async fn chat_with_tools(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _options: &ModelOptions,
        ) -> Result<ModelResponse> {
            Ok(ModelResponse::Text(self.response.clone()))
        }
    }

    /// Helper: create an edge tuple from string slices.
    fn edge(from: &str, to: &str) -> (String, String) {
        (from.to_string(), to.to_string())
    }

    // -----------------------------------------------------------------------
    // Test 1: Three-step linear workflow (THE critical integration test)
    //
    //   format_input (Transform) -> generate (Llm) -> format_output (Transform)
    //
    // Verifies: topological execution, data flow through StepInput, LLM step
    // prompt building from upstream output, and transform consuming LLM output.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_three_step_linear_workflow() {
        let wf = Workflow::new(
            vec![
                (
                    "format_input".to_string(),
                    Step::Transform {
                        transform: Box::new(|_inputs| Ok(json!({"topic": "Rust"}))),
                    },
                ),
                (
                    "generate".to_string(),
                    Step::Llm {
                        model: Box::new(MockModel::new("Rust is great")),
                        prompt_builder: Box::new(|inputs| {
                            let topic = inputs["format_input"]["topic"]
                                .as_str()
                                .unwrap_or("unknown");
                            format!("Write about {topic}")
                        }),
                        tools: None,
                        options: ModelOptions::new(),
                    },
                ),
                (
                    "format_output".to_string(),
                    Step::Transform {
                        transform: Box::new(|inputs| {
                            let text = inputs["generate"]
                                .as_str()
                                .unwrap_or("no output")
                                .to_string();
                            Ok(json!({"result": text, "formatted": true}))
                        }),
                    },
                ),
            ],
            vec![
                edge("format_input", "generate"),
                edge("generate", "format_output"),
            ],
        )
        .unwrap();

        let outputs = wf.execute().await.unwrap();

        // All three steps produced output.
        assert_eq!(outputs.len(), 3);

        // Transform step 1: produced the topic object.
        assert_eq!(outputs["format_input"], json!({"topic": "Rust"}));

        // LLM step: MockModel returned the fixed response.
        assert_eq!(outputs["generate"], json!("Rust is great"));

        // Transform step 2: wrapped the LLM output.
        assert_eq!(outputs["format_output"]["result"], "Rust is great");
        assert_eq!(outputs["format_output"]["formatted"], true);
    }

    // -----------------------------------------------------------------------
    // Test 2: Transform-only workflow
    //
    //   produce (Transform) -> double (Transform)
    //
    // Verifies: pure transform chains work without any LLM steps.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_transform_only_workflow() {
        let wf = Workflow::new(
            vec![
                (
                    "produce".to_string(),
                    Step::Transform {
                        transform: Box::new(|_inputs| Ok(json!(42))),
                    },
                ),
                (
                    "double".to_string(),
                    Step::Transform {
                        transform: Box::new(|inputs| {
                            let value = inputs["produce"].as_i64().unwrap_or(0);
                            Ok(json!(value * 2))
                        }),
                    },
                ),
            ],
            vec![edge("produce", "double")],
        )
        .unwrap();

        let outputs = wf.execute().await.unwrap();

        assert_eq!(outputs["produce"], json!(42));
        assert_eq!(outputs["double"], json!(84));
    }

    // -----------------------------------------------------------------------
    // Test 3: Independent steps (no edges)
    //
    //   a, b, c -- all independent, no dependencies between them.
    //
    // Verifies: steps with no dependencies all execute and appear in outputs.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_independent_steps_all_execute() {
        let wf = Workflow::new(
            vec![
                (
                    "a".to_string(),
                    Step::Transform {
                        transform: Box::new(|_| Ok(json!("alpha"))),
                    },
                ),
                (
                    "b".to_string(),
                    Step::Transform {
                        transform: Box::new(|_| Ok(json!("beta"))),
                    },
                ),
                (
                    "c".to_string(),
                    Step::Transform {
                        transform: Box::new(|_| Ok(json!("gamma"))),
                    },
                ),
            ],
            vec![],
        )
        .unwrap();

        let outputs = wf.execute().await.unwrap();

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs["a"], json!("alpha"));
        assert_eq!(outputs["b"], json!("beta"));
        assert_eq!(outputs["c"], json!("gamma"));
    }

    // -----------------------------------------------------------------------
    // Test 4: Diamond DAG data flow
    //
    //        source
    //        /    \
    //     left   right
    //        \    /
    //         sink
    //
    // Verifies: a step receiving outputs from multiple upstream dependencies.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_diamond_data_flow() {
        let wf = Workflow::new(
            vec![
                (
                    "source".to_string(),
                    Step::Transform {
                        transform: Box::new(|_| Ok(json!({"value": 10}))),
                    },
                ),
                (
                    "left".to_string(),
                    Step::Transform {
                        transform: Box::new(|inputs| {
                            let v = inputs["source"]["value"].as_i64().unwrap_or(0);
                            Ok(json!(v + 1))
                        }),
                    },
                ),
                (
                    "right".to_string(),
                    Step::Transform {
                        transform: Box::new(|inputs| {
                            let v = inputs["source"]["value"].as_i64().unwrap_or(0);
                            Ok(json!(v + 2))
                        }),
                    },
                ),
                (
                    "sink".to_string(),
                    Step::Transform {
                        transform: Box::new(|inputs| {
                            let left_val = inputs["left"].as_i64().unwrap_or(0);
                            let right_val = inputs["right"].as_i64().unwrap_or(0);
                            Ok(json!(left_val + right_val))
                        }),
                    },
                ),
            ],
            vec![
                edge("source", "left"),
                edge("source", "right"),
                edge("left", "sink"),
                edge("right", "sink"),
            ],
        )
        .unwrap();

        let outputs = wf.execute().await.unwrap();

        assert_eq!(outputs["source"], json!({"value": 10}));
        assert_eq!(outputs["left"], json!(11));
        assert_eq!(outputs["right"], json!(12));
        assert_eq!(outputs["sink"], json!(23));
    }
}
