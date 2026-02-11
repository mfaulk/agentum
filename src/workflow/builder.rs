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

use std::collections::{HashMap, HashSet};

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

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
    /// Runs all validation checks and collects every error before returning,
    /// so developers see all problems at once rather than fixing them one at
    /// a time.
    ///
    /// # Validation order
    ///
    /// 1. **Empty workflow** -- no steps added (returns immediately)
    /// 2. **Duplicate step names** -- same name added more than once
    /// 3. **Missing step references** -- edges referencing undefined steps
    /// 4. **Cycles** -- circular dependencies via petgraph toposort
    /// 5. **Disconnected steps** -- steps with no edges in a multi-step workflow
    ///    (single-step workflows are exempt)
    ///
    /// # Errors
    ///
    /// Returns [`BuilderErrors`] containing one or more [`BuilderError`] variants
    /// if any validation check fails.
    pub fn build(self) -> std::result::Result<Workflow, BuilderErrors> {
        let mut errors: Vec<BuilderError> = Vec::new();

        // 1. Empty workflow check -- return immediately since there is nothing
        //    else to validate.
        if self.steps.is_empty() {
            return Err(BuilderErrors {
                errors: vec![BuilderError::EmptyWorkflow],
            });
        }

        // 2. Duplicate step detection.
        let mut seen_names: HashSet<&str> = HashSet::new();
        let mut unique_names: HashSet<&str> = HashSet::new();
        for (name, _) in &self.steps {
            if !seen_names.insert(name.as_str()) {
                // Only report each duplicate name once.
                if unique_names.insert(name.as_str()) {
                    errors.push(BuilderError::DuplicateStep(name.clone()));
                }
            } else {
                unique_names.insert(name.as_str());
            }
        }

        // Collect the set of all defined step names (using first occurrence).
        let step_names: HashSet<&str> = self.steps.iter().map(|(n, _)| n.as_str()).collect();

        // 3. Build the petgraph DiGraph from first occurrences.
        let mut graph: DiGraph<String, ()> = DiGraph::new();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        // Add each unique step name as a node (skip duplicates so the graph
        // has exactly one node per name).
        for (name, _) in &self.steps {
            if !node_map.contains_key(name) {
                let idx = graph.add_node(name.clone());
                node_map.insert(name.clone(), idx);
            }
        }

        // 4. Missing step detection on edges.
        // Track already-reported missing names to avoid duplicate errors.
        let mut reported_missing: HashSet<String> = HashSet::new();

        for (from, to) in &self.edges {
            let from_exists = step_names.contains(from.as_str());
            let to_exists = step_names.contains(to.as_str());

            if !from_exists && reported_missing.insert(from.clone()) {
                errors.push(BuilderError::MissingStep {
                    edge_endpoint: from.clone(),
                });
            }
            if !to_exists && reported_missing.insert(to.clone()) {
                errors.push(BuilderError::MissingStep {
                    edge_endpoint: to.clone(),
                });
            }

            // Only add the edge to the graph if both endpoints are valid
            // (avoids panics from missing node indices).
            if from_exists && to_exists {
                let from_idx = node_map[from];
                let to_idx = node_map[to];
                graph.add_edge(from_idx, to_idx, ());
            }
        }

        // 5. Cycle detection via petgraph toposort.
        if let Err(cycle) = toposort(&graph, None) {
            errors.push(BuilderError::CycleDetected(
                graph[cycle.node_id()].clone(),
            ));
        }

        // 6. Disconnected step detection (single-step workflows are exempt).
        if self.steps.len() > 1 {
            for (name, idx) in &node_map {
                let has_incoming = graph
                    .neighbors_directed(*idx, Direction::Incoming)
                    .count()
                    > 0;
                let has_outgoing = graph
                    .neighbors_directed(*idx, Direction::Outgoing)
                    .count()
                    > 0;

                if !has_incoming && !has_outgoing {
                    errors.push(BuilderError::DisconnectedStep(name.clone()));
                }
            }
        }

        // 7. Return result.
        if !errors.is_empty() {
            return Err(BuilderErrors { errors });
        }

        // All checks passed -- construct the workflow via Workflow::new.
        // Since we already validated everything, this should not fail.
        Workflow::new(self.steps, self.edges).map_err(|e| BuilderErrors {
            errors: vec![BuilderError::CycleDetected(e.to_string())],
        })
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
