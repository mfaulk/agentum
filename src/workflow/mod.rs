//! Workflow engine: DAG-based step orchestration.
//!
//! A workflow is a directed acyclic graph (DAG) of steps. Each step is either
//! an LLM call ([`Step::Llm`]) or a pure data transformation ([`Step::Transform`]).
//! The [`Workflow`] struct validates the graph at construction time and
//! pre-computes a topological execution order.

pub mod builder;
pub mod executor;
pub mod step;
pub mod workflow;

pub use builder::WorkflowBuilder;
pub use step::{Step, StepInput, StepOutput};
pub use workflow::Workflow;
