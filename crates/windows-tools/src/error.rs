use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("operation is not supported: {0}")]
    Unsupported(String),
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ToolResult<T> = Result<T, ToolError>;
