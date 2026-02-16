use thiserror::Error;

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Command not found: {0}")]
    CommandNotFound(String),
    
    #[error("File system error: {0}")]
    FileSystemError(String),
    
    #[error("Virtual file not found: {0}")]
    VirtualFileNotFound(String),
    
    #[error("WASI error: {0}")]
    WasiError(String),
    
    #[error("Simulation mode: {0}")]
    SimulationError(String),
    
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

pub type SandboxResult<T> = Result<T, SandboxError>;
