//! Workflow struct: DAG validation, topological ordering, and accessors.
//!
//! The `Workflow` type owns a petgraph `DiGraph` representing the step
//! dependency graph. Construction validates for cycles, missing dependencies,
//! and duplicate step names, then pre-computes the topological execution order.

use std::collections::HashMap;

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
