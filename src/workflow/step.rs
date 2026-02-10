//! Step types for workflow DAG nodes.
//!
//! Each step in a workflow is either an LLM call or a pure data transformation.
//! Steps communicate through JSON values: each step receives the outputs of its
//! upstream dependencies (keyed by step name) and produces a single JSON value.

use crate::error::Result;
use crate::model::Model;
use crate::tool::ToolRegistry;
use crate::types::ModelOptions;
use serde_json::Value;
use std::collections::HashMap;

/// Output of a workflow step -- always a JSON Value for uniform data flow.
pub type StepOutput = Value;

/// Input available to a step: outputs of all completed dependencies,
/// keyed by dependency step name.
pub type StepInput = HashMap<String, Value>;

/// A single step in a workflow DAG.
///
/// Steps come in two variants:
///
/// - **Llm** -- Calls an LLM model with a dynamically built prompt. Optionally
///   provides tools for the model to call. The `prompt_builder` closure receives
///   the outputs of upstream dependencies so prompts can incorporate earlier results.
///
/// - **Transform** -- A pure data transformation that takes upstream outputs and
///   produces a new JSON value. No LLM call involved. Useful for reformatting,
///   filtering, merging, or any deterministic computation between LLM calls.
pub enum Step {
    /// An LLM call step.
    Llm {
        /// The model to call.
        model: Box<dyn Model>,
        /// Builds the prompt string from upstream step outputs.
        prompt_builder: Box<dyn Fn(&StepInput) -> String + Send + Sync>,
        /// Optional tools available for the model to call.
        tools: Option<ToolRegistry>,
        /// Model options (temperature, max_tokens, etc.).
        options: ModelOptions,
    },
    /// A pure data transformation step.
    Transform {
        /// The transformation function: takes upstream outputs, returns a JSON value.
        transform: Box<dyn Fn(&StepInput) -> Result<Value> + Send + Sync>,
    },
}
