//! Fluent builder for constructing validated [`Workflow`] instances.
//!
//! The [`WorkflowBuilder`] accumulates steps and edges via chainable methods,
//! then validates the entire graph at [`build()`](WorkflowBuilder::build) time.
//! All builder methods consume `self` and return `Self` (move semantics),
//! consistent with the [`ModelOptions`](crate::types::ModelOptions) pattern
//! used elsewhere in the crate. This is necessary because [`Step`] contains
//! non-Clone types (`Box<dyn Model>`, boxed closures).
//!
//! Forward references are allowed: you can call `.edge("A", "B")` before
//! adding step "A" or "B". Validation happens entirely at `build()` time,
//! so the order of builder calls does not matter.
//!
//! # Example
//!
//! ```ignore
//! use agentic_framework::Workflow;
//! use serde_json::json;
//!
//! let wf = Workflow::builder()
//!     .transform_step("format", |_| Ok(json!({"topic": "Rust"})))
//!     .transform_step("process", |inputs| {
//!         Ok(json!({"result": inputs["format"]["topic"]}))
//!     })
//!     .edge("format", "process")
//!     .build()?;
//! ```

use crate::error::{BuilderError, BuilderErrors, Result};
use crate::model::Model;
use crate::tool::ToolRegistry;
use crate::types::ModelOptions;
use serde_json::Value;
use super::step::{Step, StepInput};
use super::workflow::Workflow;

/// A fluent builder for constructing [`Workflow`] instances.
///
/// Accumulates steps and edges via chainable methods, then validates the
/// complete graph when [`build()`](WorkflowBuilder::build) is called.
///
/// All methods consume `self` and return `Self` (move semantics) to enable
/// method chaining. This is required because [`Step`] contains non-Clone
/// types (`Box<dyn Model>`, boxed closures).
///
/// # Chained usage (most common)
///
/// ```ignore
/// let wf = Workflow::builder()
///     .transform_step("A", |_| Ok(json!("a")))
///     .transform_step("B", |_| Ok(json!("b")))
///     .edge("A", "B")
///     .build()?;
/// ```
///
/// # Multi-statement usage (for conditional logic)
///
/// ```ignore
/// let mut builder = Workflow::builder()
///     .transform_step("A", |_| Ok(json!("a")));
///
/// if include_optional_step {
///     builder = builder.transform_step("B", |_| Ok(json!("b")))
///                      .edge("A", "B");
/// }
///
/// let wf = builder.build()?;
/// ```
pub struct WorkflowBuilder {
    steps: Vec<(String, Step)>,
    edges: Vec<(String, String)>,
}

impl WorkflowBuilder {
    /// Create a new, empty `WorkflowBuilder`.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a raw [`Step`] with the given name.
    ///
    /// This is the lowest-level step method. Prefer [`transform_step()`](Self::transform_step)
    /// or [`llm_step()`](Self::llm_step) for convenience.
    ///
    /// Duplicate step names are detected at [`build()`](Self::build) time.
    pub fn step(mut self, name: impl Into<String>, step: Step) -> Self {
        self.steps.push((name.into(), step));
        self
    }

    /// Add a [`Step::Transform`] with the given name and transform closure.
    ///
    /// This is a convenience method that wraps the closure in a `Step::Transform`
    /// and delegates to [`step()`](Self::step), avoiding manual `Box::new()`.
    pub fn transform_step(
        self,
        name: impl Into<String>,
        transform: impl Fn(&StepInput) -> Result<Value> + Send + Sync + 'static,
    ) -> Self {
        self.step(name, Step::Transform {
            transform: Box::new(transform),
        })
    }

    /// Add a [`Step::Llm`] with default options and no tools.
    ///
    /// The `prompt_builder` closure receives upstream step outputs and returns
    /// the prompt string to send to the model.
    ///
    /// For LLM steps with tools or custom options, use
    /// [`llm_step_with_tools()`](Self::llm_step_with_tools).
    pub fn llm_step(
        self,
        name: impl Into<String>,
        model: Box<dyn Model>,
        prompt_builder: impl Fn(&StepInput) -> String + Send + Sync + 'static,
    ) -> Self {
        self.step(name, Step::Llm {
            model,
            prompt_builder: Box::new(prompt_builder),
            tools: None,
            options: ModelOptions::default(),
        })
    }

    /// Add a [`Step::Llm`] with tools and custom options.
    ///
    /// This is the fully-specified LLM step method. For simpler cases without
    /// tools, use [`llm_step()`](Self::llm_step).
    pub fn llm_step_with_tools(
        self,
        name: impl Into<String>,
        model: Box<dyn Model>,
        prompt_builder: impl Fn(&StepInput) -> String + Send + Sync + 'static,
        tools: ToolRegistry,
        options: ModelOptions,
    ) -> Self {
        self.step(name, Step::Llm {
            model,
            prompt_builder: Box::new(prompt_builder),
            tools: Some(tools),
            options,
        })
    }

    /// Add a dependency edge: `from` must complete before `to`.
    ///
    /// This matches the convention used by [`Workflow::new`]: `(from, to)` means
    /// "from must complete before to can run".
    ///
    /// Forward references are allowed -- the step does not need to exist yet.
    /// All edges are validated at [`build()`](Self::build) time.
    pub fn edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.edges.push((from.into(), to.into()));
        self
    }

    /// Connect steps in a linear chain: A -> B -> C -> ...
    ///
    /// Equivalent to calling `.edge("A", "B").edge("B", "C")` for each
    /// adjacent pair. Uses [`windows(2)`](slice::windows) internally.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // These are equivalent:
    /// builder.edge("A", "B").edge("B", "C")
    /// builder.chain(&["A", "B", "C"])
    /// ```
    pub fn chain(mut self, steps: &[&str]) -> Self {
        for window in steps.windows(2) {
            self.edges.push((window[0].to_string(), window[1].to_string()));
        }
        self
    }

    /// Validate and build the [`Workflow`].
    ///
    /// This stub delegates to [`Workflow::new`] and wraps any error in a
    /// single-element [`BuilderErrors`]. Plan 02 replaces this with full
    /// validation logic (disconnected step detection, error collection).
    ///
    /// # Errors
    ///
    /// Returns [`BuilderErrors`] if the workflow is structurally invalid.
    pub fn build(self) -> std::result::Result<Workflow, BuilderErrors> {
        Workflow::new(self.steps, self.edges).map_err(|e| BuilderErrors {
            errors: vec![match e {
                crate::error::Error::InvalidWorkflow(msg) => {
                    if msg.contains("duplicate step name") {
                        let name = msg.strip_prefix("duplicate step name: ")
                            .unwrap_or(&msg)
                            .to_string();
                        BuilderError::DuplicateStep(name)
                    } else if msg.contains("cycle detected") {
                        let name = msg.strip_prefix("cycle detected involving step: ")
                            .unwrap_or(&msg)
                            .to_string();
                        BuilderError::CycleDetected(name)
                    } else {
                        // Fallback: wrap the message as a cycle error
                        BuilderError::CycleDetected(msg)
                    }
                }
                crate::error::Error::MissingDependency { step: _, dependency } => {
                    BuilderError::MissingStep { edge_endpoint: dependency }
                }
                other => {
                    // Unexpected error type -- wrap as a generic message
                    BuilderError::CycleDetected(other.to_string())
                }
            }],
        })
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
