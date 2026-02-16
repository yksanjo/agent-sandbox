//! Agent Sandbox - Deterministic Execution Firewall
//!
//! A WASI-based sandbox runtime for AI agents with file-system virtualization,
//! tool permission gating, side-effect simulation, and diff previews.
//!
//! # Features
//!
//! - **WASI-based Execution**: WebAssembly System Interface for sandboxed execution
//! - **File-system Virtualization**: Virtual filesystem with diff tracking
//! - **Tool Permission Gating**: Control which commands/tools agents can access
//! - **Side-effect Simulation**: Preview changes without executing (dry-run mode)
//! - **Diff Previews**: See exactly what will change before committing
//!
//! # Quick Start
//!
//! ```rust
//! use agent_sandbox::sandbox::{Sandbox, ExecutionMode};
//!
//! // Create a new sandbox
//! let mut sandbox = Sandbox::new();
//!
//! // Set to simulation mode
//! sandbox.set_mode(ExecutionMode::Simulation);
//!
//! // Execute a command (won't actually run)
//! let result = sandbox.execute("git commit -m 'fix: bug'")?;
//! ```

pub mod diff_engine;
pub mod error;
pub mod permissions;
pub mod sandbox;
pub mod virtual_fs;

// Re-export main types
pub use diff_engine::{DiffEngine, DiffSummary, UnifiedDiff};
pub use error::{SandboxError, SandboxResult};
pub use permissions::{PermissionGate, PermissionLevel, ToolPermission};
pub use sandbox::{ExecutionMode, ExecutionResult, ExecutionStatus, Sandbox, SandboxStatus};
pub use virtual_fs::{DiffOperation, FileDiff, VirtualFile, VirtualFilesystem};
