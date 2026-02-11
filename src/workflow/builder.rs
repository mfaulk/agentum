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

use super::step::{Step, StepInput};
use super::workflow::Workflow;
use crate::error::{BuilderError, BuilderErrors, Result};
use crate::model::Model;
use crate::tool::ToolRegistry;
use crate::types::ModelOptions;
use serde_json::Value;

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
        self.step(
            name,
            Step::Transform {
                transform: Box::new(transform),
            },
        )
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
        self.step(
            name,
            Step::Llm {
                model,
                prompt_builder: Box::new(prompt_builder),
                tools: None,
                options: ModelOptions::default(),
            },
        )
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
        self.step(
            name,
            Step::Llm {
                model,
                prompt_builder: Box::new(prompt_builder),
                tools: Some(tools),
                options,
            },
        )
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
            self.edges
                .push((window[0].to_string(), window[1].to_string()));
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
        let mut reported_duplicates: HashSet<&str> = HashSet::new();
        for (name, _) in &self.steps {
            if !seen_names.insert(name.as_str()) {
                // Only report each duplicate name once.
                if reported_duplicates.insert(name.as_str()) {
                    errors.push(BuilderError::DuplicateStep(name.clone()));
                }
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
            errors.push(BuilderError::CycleDetected(graph[cycle.node_id()].clone()));
        }

        // 6. Disconnected step detection (single-step workflows are exempt).
        // Use the count of unique step names (graph nodes), not self.steps.len(),
        // because duplicates inflate the raw count.
        if node_map.len() > 1 {
            for (name, idx) in &node_map {
                let has_incoming = graph.neighbors_directed(*idx, Direction::Incoming).count() > 0;
                let has_outgoing = graph.neighbors_directed(*idx, Direction::Outgoing).count() > 0;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::BuilderError;
    use serde_json::json;

    /// Create a dummy Transform step for testing.
    fn dummy_transform() -> Step {
        Step::Transform {
            transform: Box::new(|_| Ok(serde_json::json!(null))),
        }
    }

    // -----------------------------------------------------------------------
    // Valid construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_builder_single_step() {
        let wf = Workflow::builder()
            .step("only", dummy_transform())
            .build()
            .expect("single step should build successfully");

        assert_eq!(wf.execution_order().len(), 1);
        assert_eq!(wf.execution_order()[0], "only");
    }

    #[test]
    fn test_builder_linear_chain() {
        let wf = Workflow::builder()
            .step("A", dummy_transform())
            .step("B", dummy_transform())
            .step("C", dummy_transform())
            .chain(&["A", "B", "C"])
            .build()
            .expect("linear chain should build successfully");

        assert_eq!(wf.execution_order(), &["A", "B", "C"]);
    }

    #[test]
    fn test_builder_explicit_edges() {
        let wf = Workflow::builder()
            .step("A", dummy_transform())
            .step("B", dummy_transform())
            .edge("A", "B")
            .build()
            .expect("explicit edges should build successfully");

        assert_eq!(wf.execution_order(), &["A", "B"]);
    }

    #[test]
    fn test_builder_diamond() {
        let wf = Workflow::builder()
            .step("A", dummy_transform())
            .step("B", dummy_transform())
            .step("C", dummy_transform())
            .step("D", dummy_transform())
            .edge("A", "B")
            .edge("A", "C")
            .edge("B", "D")
            .edge("C", "D")
            .build()
            .expect("diamond should build successfully");

        let order = wf.execution_order();
        assert_eq!(order[0], "A", "A must be first");
        assert_eq!(order[3], "D", "D must be last");
    }

    #[test]
    fn test_builder_typed_helpers() {
        let wf = Workflow::builder()
            .transform_step("step1", |_| Ok(json!({"msg": "hello"})))
            .transform_step("step2", |inputs| {
                Ok(json!({"echo": inputs["step1"]["msg"]}))
            })
            .edge("step1", "step2")
            .build()
            .expect("typed helpers should build successfully");

        assert_eq!(wf.execution_order(), &["step1", "step2"]);
    }

    // -----------------------------------------------------------------------
    // Error detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_builder_empty_workflow() {
        let result = Workflow::builder().build();
        let err = result.unwrap_err();
        assert_eq!(err.errors.len(), 1);
        assert!(
            matches!(err.errors[0], BuilderError::EmptyWorkflow),
            "expected EmptyWorkflow, got: {:?}",
            err.errors[0]
        );
    }

    #[test]
    fn test_builder_duplicate_step() {
        let result = Workflow::builder()
            .step("A", dummy_transform())
            .step("A", dummy_transform())
            .build();

        let err = result.unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, BuilderError::DuplicateStep(name) if name == "A")),
            "expected DuplicateStep(A), got: {:?}",
            err.errors
        );
    }

    #[test]
    fn test_builder_missing_step_in_edge() {
        let result = Workflow::builder()
            .step("A", dummy_transform())
            .edge("A", "nonexistent")
            .build();

        let err = result.unwrap_err();
        assert!(
            err.errors.iter().any(|e| matches!(
                e,
                BuilderError::MissingStep { edge_endpoint } if edge_endpoint == "nonexistent"
            )),
            "expected MissingStep for 'nonexistent', got: {:?}",
            err.errors
        );
    }

    #[test]
    fn test_builder_cycle_detection() {
        let result = Workflow::builder()
            .step("A", dummy_transform())
            .step("B", dummy_transform())
            .step("C", dummy_transform())
            .edge("A", "B")
            .edge("B", "C")
            .edge("C", "A")
            .build();

        let err = result.unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, BuilderError::CycleDetected(_))),
            "expected CycleDetected, got: {:?}",
            err.errors
        );
    }

    #[test]
    fn test_builder_disconnected_step() {
        let result = Workflow::builder()
            .step("A", dummy_transform())
            .step("B", dummy_transform())
            .step("orphan", dummy_transform())
            .edge("A", "B")
            .build();

        let err = result.unwrap_err();
        assert!(
            err.errors.iter().any(|e| matches!(
                e,
                BuilderError::DisconnectedStep(name) if name == "orphan"
            )),
            "expected DisconnectedStep(orphan), got: {:?}",
            err.errors
        );
    }

    #[test]
    fn test_builder_single_step_not_disconnected() {
        // Single-step workflows are exempt from disconnected step detection.
        let wf = Workflow::builder()
            .step("alone", dummy_transform())
            .build()
            .expect("single step should not be flagged as disconnected");

        assert_eq!(wf.execution_order().len(), 1);
    }

    // -----------------------------------------------------------------------
    // Error collection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_builder_collects_multiple_errors() {
        // Both a duplicate step AND a missing step reference.
        let result = Workflow::builder()
            .step("A", dummy_transform())
            .step("A", dummy_transform()) // duplicate
            .edge("A", "ghost") // missing step
            .build();

        let err = result.unwrap_err();
        assert!(
            err.errors.len() >= 2,
            "expected at least 2 errors (got {}): {:?}",
            err.errors.len(),
            err.errors
        );

        let has_duplicate = err
            .errors
            .iter()
            .any(|e| matches!(e, BuilderError::DuplicateStep(_)));
        let has_missing = err
            .errors
            .iter()
            .any(|e| matches!(e, BuilderError::MissingStep { .. }));
        assert!(has_duplicate, "expected a DuplicateStep error");
        assert!(has_missing, "expected a MissingStep error");
    }
}
