use crate::diff_engine::DiffSummary;
use crate::error::{SandboxError, SandboxResult};
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::virtual_fs::{VirtualFilesystem, FileDiff};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

/// Sandbox execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Real execution (with safeguards)
    Live,
    /// Simulation mode - preview only, no actual execution
    Simulation,
    /// Diff mode - show what would change without executing
    Diff,
}

/// Result of a sandboxed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub id: String,
    pub command: String,
    pub tool: String,
    pub args: Vec<String>,
    pub mode: ExecutionMode,
    pub status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub file_changes: Vec<FileDiff>,
    pub diff_summary: Option<DiffSummary>,
    pub permission_level: PermissionLevel,
    pub approved: bool,
    pub executed_at: i64,
}

/// Status of execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Command executed successfully
    Success,
    /// Command failed
    Failed,
    /// Command was blocked by permissions
    Blocked,
    /// Command was simulated (no actual execution)
    Simulated,
    /// Command requires approval
    PendingApproval,
}

/// A sandbox session
#[derive(Debug)]
pub struct Sandbox {
    pub id: String,
    pub virtual_fs: VirtualFilesystem,
    pub permissions: PermissionGate,
    pub mode: ExecutionMode,
    pub execution_history: Vec<ExecutionResult>,
    pub pending_approvals: HashMap<String, ExecutionResult>,
    pub working_dir: PathBuf,
    pub allow_all: bool,
}

impl Sandbox {
    /// Create a new sandbox
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            virtual_fs: VirtualFilesystem::new(),
            permissions: PermissionGate::default_permissions(),
            mode: ExecutionMode::Live,
            execution_history: Vec::new(),
            pending_approvals: HashMap::new(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            allow_all: false,
        }
    }
    
    /// Create a sandbox with a specific working directory
    pub fn with_working_dir(path: PathBuf) -> SandboxResult<Self> {
        let mut sandbox = Self::new();
        sandbox.working_dir = path;
        
        // Mount the working directory
        if sandbox.working_dir.exists() {
            sandbox.virtual_fs.mount(&sandbox.working_dir)?;
        }
        
        Ok(sandbox)
    }
    
    /// Set the execution mode
    pub fn set_mode(&mut self, mode: ExecutionMode) {
        self.mode = mode;
    }
    
    /// Enable allow all mode (bypass permissions for testing)
    pub fn allow_all(&mut self) {
        self.allow_all = true;
    }
    
    /// Execute a command in the sandbox
    pub fn execute(&mut self, command: &str) -> SandboxResult<ExecutionResult> {
        // Parse command into tool and arguments
        let parts: Vec<String> = shell_words::split(command)
            .map_err(|e| SandboxError::InvalidCommand(e.to_string()))?;
        
        if parts.is_empty() {
            return Err(SandboxError::InvalidCommand("Empty command".to_string()));
        }
        
        let tool = &parts[0];
        let args = &parts[1..];
        
        self.execute_tool(tool, args)
    }
    
    /// Execute a specific tool with arguments
    pub fn execute_tool(&mut self, tool: &str, args: &[String]) -> SandboxResult<ExecutionResult> {
        // Check permissions
        let permission_level = if self.allow_all {
            PermissionLevel::Full
        } else {
            self.permissions.check_command(tool, args)?
        };
        
        // Check if approval is required
        if self.permissions.requires_approval(tool) && !self.allow_all {
            // Create a pending approval result
            let result = ExecutionResult {
                id: Uuid::new_v4().to_string(),
                command: format!("{} {}", tool, args.join(" ")),
                tool: tool.to_string(),
                args: args.to_vec(),
                mode: self.mode,
                status: ExecutionStatus::PendingApproval,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                file_changes: Vec::new(),
                diff_summary: None,
                permission_level,
                approved: false,
                executed_at: chrono::Utc::now().timestamp(),
            };
            
            // Store for approval
            self.pending_approvals.insert(result.id.clone(), result.clone());
            
            return Ok(result);
        }
        
        // Execute based on mode
        match self.mode {
            ExecutionMode::Simulation => self.simulate_execution(tool, args, permission_level),
            ExecutionMode::Diff => self.diff_execution(tool, args, permission_level),
            ExecutionMode::Live => self.live_execution(tool, args, permission_level),
        }
    }
    
    /// Execute in live mode (actual execution with safeguards)
    fn live_execution(
        &mut self,
        tool: &str,
        args: &[String],
        permission_level: PermissionLevel,
    ) -> SandboxResult<ExecutionResult> {
        // Build the command
        let mut cmd = Command::new(tool);
        cmd.args(args)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Execute
        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                return Ok(ExecutionResult {
                    id: Uuid::new_v4().to_string(),
                    command: format!("{} {}", tool, args.join(" ")),
                    tool: tool.to_string(),
                    args: args.to_vec(),
                    mode: self.mode,
                    status: ExecutionStatus::Failed,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    exit_code: Some(-1),
                    file_changes: Vec::new(),
                    diff_summary: None,
                    permission_level,
                    approved: true,
                    executed_at: chrono::Utc::now().timestamp(),
                });
            }
        };
        
        let status = if output.status.success() {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::Failed
        };
        
        let result = ExecutionResult {
            id: Uuid::new_v4().to_string(),
            command: format!("{} {}", tool, args.join(" ")),
            tool: tool.to_string(),
            args: args.to_vec(),
            mode: self.mode,
            status,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            file_changes: Vec::new(),
            diff_summary: None,
            permission_level,
            approved: true,
            executed_at: chrono::Utc::now().timestamp(),
        };
        
        self.execution_history.push(result.clone());
        Ok(result)
    }
    
    /// Execute in simulation mode (preview only)
    fn simulate_execution(
        &self,
        tool: &str,
        args: &[String],
        permission_level: PermissionLevel,
    ) -> SandboxResult<ExecutionResult> {
        let command = format!("{} {}", tool, args.join(" "));
        
        // Analyze what would happen
        let (stdout, stderr) = self.analyze_command(tool, args);
        
        Ok(ExecutionResult {
            id: Uuid::new_v4().to_string(),
            command,
            tool: tool.to_string(),
            args: args.to_vec(),
            mode: self.mode,
            status: ExecutionStatus::Simulated,
            stdout,
            stderr,
            exit_code: None,
            file_changes: Vec::new(),
            diff_summary: None,
            permission_level,
            approved: true,
            executed_at: chrono::Utc::now().timestamp(),
        })
    }
    
    /// Execute in diff mode (show what would change)
    fn diff_execution(
        &self,
        tool: &str,
        args: &[String],
        permission_level: PermissionLevel,
    ) -> SandboxResult<ExecutionResult> {
        let command = format!("{} {}", tool, args.join(" "));
        
        // Get file changes
        let file_changes = self.predict_file_changes(tool, args);
        
        // Generate diff summary
        let diff_summary = if !file_changes.is_empty() {
            let total_added: usize = file_changes
                .iter()
                .filter(|d| d.new_content.is_some())
                .map(|d| d.new_content.as_ref().unwrap().lines().count())
                .sum();
            let total_deleted: usize = file_changes
                .iter()
                .filter(|d| d.old_content.is_some())
                .map(|d| d.old_content.as_ref().unwrap().lines().count())
                .sum();
            
            Some(DiffSummary {
                added: total_added,
                deleted: total_deleted,
                unchanged: 0,
            })
        } else {
            None
        };
        
        Ok(ExecutionResult {
            id: Uuid::new_v4().to_string(),
            command,
            tool: tool.to_string(),
            args: args.to_vec(),
            mode: self.mode,
            status: ExecutionStatus::Simulated,
            stdout: String::new(),
            stderr: format!("Diff preview for {} file(s)", file_changes.len()),
            exit_code: None,
            file_changes,
            diff_summary,
            permission_level,
            approved: true,
            executed_at: chrono::Utc::now().timestamp(),
        })
    }
    
    /// Approve a pending execution
    pub fn approve(&mut self, execution_id: &str) -> SandboxResult<ExecutionResult> {
        let result = self.pending_approvals
            .remove(execution_id)
            .ok_or_else(|| SandboxError::InvalidCommand("Execution not found".to_string()))?;
        
        // Execute the command in live mode
        let live_result = self.live_execution(
            &result.tool,
            &result.args,
            result.permission_level,
        )?;
        
        Ok(live_result)
    }
    
    /// Analyze what a command would do
    fn analyze_command(&self, tool: &str, args: &[String]) -> (String, String) {
        let mut stdout = format!("[SIMULATION] Would execute: {} {}\n\n", tool, args.join(" "));
        let mut stderr = String::new();
        
        // Check what files would be affected
        let file_changes = self.predict_file_changes(tool, args);
        
        if file_changes.is_empty() {
            stdout.push_str("No file changes detected.\n");
        } else {
            stdout.push_str(&format!("Would affect {} file(s):\n", file_changes.len()));
            for diff in &file_changes {
                stdout.push_str(&format!("  - {}\n", diff.path.display()));
            }
        }
        
        // Add permission info
        if let Some(permission) = self.permissions.get_permission(tool) {
            stdout.push_str(&format!("\nPermission level: {:?}\n", permission.level));
            if permission.requires_approval {
                stdout.push_str("Requires approval: YES\n");
            }
        }
        
        (stdout, stderr)
    }
    
    /// Predict what files would be changed by a command
    fn predict_file_changes(&self, tool: &str, args: &[String]) -> Vec<FileDiff> {
        let mut changes = Vec::new();
        
        // Git commands
        if tool == "git" {
            if args.iter().any(|a| a == "add" || a == "commit") {
                // Would stage/commit files
                for file in self.virtual_fs.list_files() {
                    changes.push(FileDiff {
                        path: file,
                        operation: crate::virtual_fs::DiffOperation::Modified,
                        old_content: None,
                        new_content: Some("(staged)".to_string()),
                    });
                }
            }
        }
        
        // npm commands
        if tool == "npm" && args.iter().any(|a| a == "install") {
            changes.push(FileDiff {
                path: PathBuf::from("package-lock.json"),
                operation: crate::virtual_fs::DiffOperation::Modified,
                old_content: None,
                new_content: Some("(would be updated)".to_string()),
            });
            changes.push(FileDiff {
                path: PathBuf::from("node_modules/"),
                operation: crate::virtual_fs::DiffOperation::Modified,
                old_content: None,
                new_content: Some("(would be populated)".to_string()),
            });
        }
        
        // File write operations
        if tool == "echo" || tool == "tee" || tool == "cat" {
            for arg in args {
                if arg.starts_with('>') {
                    let path = arg.trim_start_matches(">").trim();
                    changes.push(FileDiff {
                        path: PathBuf::from(path),
                        operation: crate::virtual_fs::DiffOperation::Modified,
                        old_content: None,
                        new_content: Some("(would be written)".to_string()),
                    });
                }
            }
        }
        
        changes
    }
    
    /// Get execution history
    pub fn history(&self) -> &[ExecutionResult] {
        &self.execution_history
    }
    
    /// Get pending approvals
    pub fn pending_approvals(&self) -> HashMap<String, ExecutionResult> {
        self.pending_approvals.clone()
    }
    
    /// Reset the sandbox
    pub fn reset(&mut self) {
        self.virtual_fs.reset();
        self.execution_history.clear();
        self.pending_approvals.clear();
    }
    
    /// Get sandbox status
    pub fn status(&self) -> SandboxStatus {
        SandboxStatus {
            id: self.id.clone(),
            mode: self.mode,
            file_count: self.virtual_fs.list_files().len(),
            execution_count: self.execution_history.len(),
            pending_approval_count: self.pending_approvals.len(),
            working_dir: self.working_dir.clone(),
        }
    }
}

/// Sandbox status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxStatus {
    pub id: String,
    pub mode: ExecutionMode,
    pub file_count: usize,
    pub execution_count: usize,
    pub pending_approval_count: usize,
    pub working_dir: PathBuf,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

// Simple shell words parser
mod shell_words {
    use std::borrow::Cow;
    
    pub fn split(input: &str) -> Result<Vec<String>, Cow<'static, str>> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';
        let mut escaped = false;
        
        for c in input.chars() {
            if escaped {
                current.push(c);
                escaped = false;
                continue;
            }
            
            match c {
                '\\' if !in_quotes => {
                    escaped = true;
                }
                '\'' | '"' if in_quotes && c == quote_char => {
                    in_quotes = false;
                }
                '\'' | '"' if !in_quotes => {
                    in_quotes = true;
                    quote_char = c;
                }
                ' ' | '\t' | '\n' | '\r' if !in_quotes => {
                    if !current.is_empty() {
                        words.push(current.clone());
                        current.clear();
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }
        
        if !current.is_empty() {
            words.push(current);
        }
        
        if in_quotes {
            return Err("Unclosed quote".into());
        }
        
        Ok(words)
    }
}
