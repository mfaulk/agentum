use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    // --- Runtime errors (things that go wrong at call time) ---
    /// HTTP request to the LLM API failed (network error, timeout, DNS failure).
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

    /// Failed to parse/deserialize the API response JSON.
    #[error("failed to parse API response: {0}")]
    ResponseParse(#[from] serde_json::Error),

    /// The API returned a non-success HTTP status with an error message body.
    #[error("API error ({status}): {message}")]
    ApiResponse { status: u16, message: String },

    /// The API response was structurally valid but missing expected content.
    #[error("unexpected API response: {0}")]
    UnexpectedResponse(String),

    // --- Tool errors ---
    /// A tool requested by the model was not registered.
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    /// A registered tool failed during execution.
    #[error("tool execution failed: {name}: {message}")]
    ToolExecutionFailed { name: String, message: String },

    /// A tool with this name is already registered.
    #[error("duplicate tool: {0}")]
    DuplicateTool(String),

    // --- Framework errors (structural/configuration problems) ---
    /// The workflow definition is structurally invalid (e.g., contains cycles).
    #[error("invalid workflow: {0}")]
    InvalidWorkflow(String),

    /// A workflow step referenced a dependency that does not exist.
    #[error("missing dependency: step '{step}' depends on '{dependency}' which does not exist")]
    MissingDependency { step: String, dependency: String },

    /// Configuration is missing or invalid (e.g., no API key).
    #[error("configuration error: {0}")]
    Config(String),
}

/// Convenience Result alias for the agentum library.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during workflow construction via [`WorkflowBuilder`].
///
/// Each variant represents a specific structural problem found at `build()` time.
/// The builder collects all errors rather than failing at the first one, so
/// developers see every problem at once.
///
/// [`WorkflowBuilder`]: crate::workflow::builder::WorkflowBuilder
#[derive(Debug)]
pub enum BuilderError {
    /// A step name was used more than once.
    DuplicateStep(String),
    /// An edge references a step that was not added to the builder.
    MissingStep { edge_endpoint: String },
    /// The workflow graph contains a cycle involving the named step.
    CycleDetected(String),
    /// A step has no connections to any other step in a multi-step workflow.
    DisconnectedStep(String),
    /// No steps were added to the builder.
    EmptyWorkflow,
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuilderError::DuplicateStep(name) => write!(f, "duplicate step name: {name}"),
            BuilderError::MissingStep { edge_endpoint } => {
                write!(f, "edge references undefined step: {edge_endpoint}")
            }
            BuilderError::CycleDetected(step) => {
                write!(f, "cycle detected involving step: {step}")
            }
            BuilderError::DisconnectedStep(name) => {
                write!(f, "step '{name}' has no edges to any other step")
            }
            BuilderError::EmptyWorkflow => write!(f, "workflow has no steps"),
        }
    }
}

impl std::error::Error for BuilderError {}

/// A collection of [`BuilderError`]s returned by [`WorkflowBuilder::build`].
///
/// The builder validates the entire workflow and collects all errors, so
/// developers can fix every problem in one pass.
///
/// [`WorkflowBuilder::build`]: crate::workflow::builder::WorkflowBuilder::build
#[derive(Debug)]
pub struct BuilderErrors {
    pub errors: Vec<BuilderError>,
}

impl std::fmt::Display for BuilderErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let messages: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
        write!(f, "{}", messages.join("; "))
    }
}

impl std::error::Error for BuilderErrors {}

impl From<BuilderErrors> for Error {
    fn from(errors: BuilderErrors) -> Self {
        Error::InvalidWorkflow(errors.to_string())
    }
}
