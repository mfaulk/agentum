//! Workflow struct: DAG validation, topological ordering, and accessors.
//!
//! The `Workflow` type owns a petgraph `DiGraph` representing the step
//! dependency graph. Construction validates for cycles, missing dependencies,
//! and duplicate step names, then pre-computes the topological execution order.

use std::collections::HashMap;
use std::fmt;

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

use crate::error::{Error, Result};
use super::step::Step;

/// A validated workflow DAG with pre-computed execution order.
///
/// Create a `Workflow` via [`Workflow::new`], which validates the graph and
/// rejects invalid configurations. Once constructed, the workflow is
/// guaranteed to be a valid DAG with all dependencies resolved.
pub struct Workflow {
    /// The underlying directed graph. Node weights are step names.
    graph: DiGraph<String, ()>,
    /// Step implementations, keyed by name.
    steps: HashMap<String, Step>,
    /// Maps step names to their graph node indices for efficient lookup.
    node_indices: HashMap<String, NodeIndex>,
    /// Pre-computed topological execution order (step names).
    execution_order: Vec<String>,
}

impl Workflow {
    /// Create a [`WorkflowBuilder`](super::builder::WorkflowBuilder) for fluent workflow construction.
    ///
    /// The builder provides a chainable API for adding steps and edges,
    /// with build-time validation including disconnected step detection.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let wf = Workflow::builder()
    ///     .transform_step("A", |_| Ok(json!("a")))
    ///     .transform_step("B", |_| Ok(json!("b")))
    ///     .edge("A", "B")
    ///     .build()?;
    /// ```
    pub fn builder() -> super::builder::WorkflowBuilder {
        super::builder::WorkflowBuilder::new()
    }

    /// Create a new workflow from steps and dependency edges.
    ///
    /// Validates the graph at construction time:
    /// 1. Rejects duplicate step names (`Error::InvalidWorkflow`)
    /// 2. Rejects edges referencing non-existent steps (`Error::MissingDependency`)
    /// 3. Rejects cyclic graphs (`Error::InvalidWorkflow`)
    ///
    /// Edge convention: `(from, to)` means "from must complete before to".
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidWorkflow`] if duplicate step names or cycles detected
    /// - [`Error::MissingDependency`] if an edge references a non-existent step
    pub fn new(steps: Vec<(String, Step)>, edges: Vec<(String, String)>) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut step_map = HashMap::new();
        let mut node_indices = HashMap::new();

        // 1. Add all steps as graph nodes, checking for duplicates.
        for (name, step) in steps {
            if step_map.contains_key(&name) {
                return Err(Error::InvalidWorkflow(format!(
                    "duplicate step name: {}",
                    name
                )));
            }
            let idx = graph.add_node(name.clone());
            node_indices.insert(name.clone(), idx);
            step_map.insert(name, step);
        }

        // 2. Add edges, validating that both endpoints exist.
        for (from, to) in edges {
            let from_idx = node_indices.get(&from).ok_or_else(|| Error::MissingDependency {
                step: to.clone(),
                dependency: from.clone(),
            })?;
            let to_idx = node_indices.get(&to).ok_or_else(|| Error::MissingDependency {
                step: from.clone(),
                dependency: to.clone(),
            })?;
            graph.add_edge(*from_idx, *to_idx, ());
        }

        // 3. Topological sort -- detects cycles.
        let sorted = toposort(&graph, None).map_err(|cycle| {
            let name = &graph[cycle.node_id()];
            Error::InvalidWorkflow(format!("cycle detected involving step: {}", name))
        })?;

        // 4. Convert node indices to step names for the execution order.
        let execution_order: Vec<String> = sorted
            .iter()
            .map(|idx| graph[*idx].clone())
            .collect();

        Ok(Self {
            graph,
            steps: step_map,
            node_indices,
            execution_order,
        })
    }

    /// Returns the pre-computed topological execution order.
    ///
    /// Steps should be executed in this order to satisfy all dependencies.
    pub fn execution_order(&self) -> &[String] {
        &self.execution_order
    }

    /// Look up a step by name.
    pub fn step(&self, name: &str) -> Option<&Step> {
        self.steps.get(name)
    }

    /// Returns the names of a step's upstream dependencies.
    ///
    /// These are the steps that must complete before the given step can run.
    /// Returns an empty vec if the step has no dependencies or does not exist.
    pub fn dependencies(&self, name: &str) -> Vec<String> {
        match self.node_indices.get(name) {
            Some(&idx) => self
                .graph
                .neighbors_directed(idx, Direction::Incoming)
                .map(|dep_idx| self.graph[dep_idx].clone())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Returns all step names in the workflow.
    pub fn step_names(&self) -> Vec<&str> {
        self.steps.keys().map(|s| s.as_str()).collect()
    }
}

impl fmt::Debug for Workflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workflow")
            .field("execution_order", &self.execution_order)
            .field("step_count", &self.steps.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    /// Create a dummy Transform step for testing.
    fn dummy_step() -> Step {
        Step::Transform {
            transform: Box::new(|_| Ok(serde_json::Value::Null)),
        }
    }

    /// Helper to create a named step tuple.
    fn named(name: &str) -> (String, Step) {
        (name.to_string(), dummy_step())
    }

    /// Helper to create an edge tuple.
    fn edge(from: &str, to: &str) -> (String, String) {
        (from.to_string(), to.to_string())
    }

    #[test]
    fn test_linear_workflow_order() {
        // A -> B -> C
        let wf = Workflow::new(
            vec![named("A"), named("B"), named("C")],
            vec![edge("A", "B"), edge("B", "C")],
        )
        .unwrap();

        assert_eq!(wf.execution_order(), &["A", "B", "C"]);
    }

    #[test]
    fn test_diamond_workflow() {
        // A -> B, A -> C, B -> D, C -> D
        let wf = Workflow::new(
            vec![named("A"), named("B"), named("C"), named("D")],
            vec![
                edge("A", "B"),
                edge("A", "C"),
                edge("B", "D"),
                edge("C", "D"),
            ],
        )
        .unwrap();

        let order = wf.execution_order();

        // A must be first, D must be last.
        assert_eq!(order[0], "A");
        assert_eq!(order[3], "D");

        // B and C must both appear between A and D.
        let b_pos = order.iter().position(|s| s == "B").unwrap();
        let c_pos = order.iter().position(|s| s == "C").unwrap();
        assert!(b_pos > 0 && b_pos < 3);
        assert!(c_pos > 0 && c_pos < 3);
    }

    #[test]
    fn test_cycle_detection() {
        // A -> B -> C -> A (cycle)
        let result = Workflow::new(
            vec![named("A"), named("B"), named("C")],
            vec![edge("A", "B"), edge("B", "C"), edge("C", "A")],
        );

        let err = result.unwrap_err();
        match &err {
            Error::InvalidWorkflow(msg) => {
                assert!(msg.contains("cycle detected"), "expected cycle message, got: {msg}");
            }
            other => panic!("expected InvalidWorkflow, got: {other:?}"),
        }
    }

    #[test]
    fn test_missing_dependency() {
        // Edge references "X" which does not exist as a step.
        let result = Workflow::new(
            vec![named("A"), named("B")],
            vec![edge("X", "B")],
        );

        let err = result.unwrap_err();
        match &err {
            Error::MissingDependency { step, dependency } => {
                assert_eq!(dependency, "X");
                assert_eq!(step, "B");
            }
            other => panic!("expected MissingDependency, got: {other:?}"),
        }
    }

    #[test]
    fn test_duplicate_step_names() {
        let result = Workflow::new(
            vec![named("A"), named("A")],
            vec![],
        );

        let err = result.unwrap_err();
        match &err {
            Error::InvalidWorkflow(msg) => {
                assert!(msg.contains("duplicate step name: A"), "got: {msg}");
            }
            other => panic!("expected InvalidWorkflow, got: {other:?}"),
        }
    }

    #[test]
    fn test_single_step_no_edges() {
        let wf = Workflow::new(
            vec![named("only")],
            vec![],
        )
        .unwrap();

        assert_eq!(wf.execution_order(), &["only"]);
        assert!(wf.step("only").is_some());
        assert!(wf.step("missing").is_none());
        assert!(wf.dependencies("only").is_empty());
    }

    #[test]
    fn test_dependencies_accessor() {
        // A -> B -> C
        let wf = Workflow::new(
            vec![named("A"), named("B"), named("C")],
            vec![edge("A", "B"), edge("B", "C")],
        )
        .unwrap();

        // A has no dependencies.
        assert!(wf.dependencies("A").is_empty());

        // B depends on A.
        let b_deps = wf.dependencies("B");
        assert_eq!(b_deps.len(), 1);
        assert_eq!(b_deps[0], "A");

        // C depends on B.
        let c_deps = wf.dependencies("C");
        assert_eq!(c_deps.len(), 1);
        assert_eq!(c_deps[0], "B");

        // Non-existent step returns empty.
        assert!(wf.dependencies("Z").is_empty());
    }
}
