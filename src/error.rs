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

/// Convenience Result alias for the agentic-framework library.
pub type Result<T> = std::result::Result<T, Error>;
