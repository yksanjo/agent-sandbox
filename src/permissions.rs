use crate::error::{SandboxError, SandboxResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Permission level for a tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// No access
    Denied,
    /// Read-only access
    ReadOnly,
    /// Read and execute
    Execute,
    /// Full access
    Full,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        PermissionLevel::Denied
    }
}

/// Tool permission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    pub name: String,
    pub level: PermissionLevel,
    pub allowed_paths: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
    pub allowed_args: Vec<String>,
    pub requires_approval: bool,
}

impl ToolPermission {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            level: PermissionLevel::default(),
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
            allowed_args: Vec::new(),
            requires_approval: false,
        }
    }
    
    pub fn with_level(mut self, level: PermissionLevel) -> Self {
        self.level = level;
        self
    }
    
    pub fn allow_path(mut self, path: PathBuf) -> Self {
        self.allowed_paths.push(path);
        self
    }
    
    pub fn deny_path(mut self, path: PathBuf) -> Self {
        self.denied_paths.push(path);
        self
    }
    
    pub fn allow_arg(mut self, arg: &str) -> Self {
        self.allowed_args.push(arg.to_string());
        self
    }
    
    pub fn requires_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }
    
    /// Check if this tool is allowed to run with the given arguments
    pub fn check_args(&self, args: &[String]) -> bool {
        if self.allowed_args.is_empty() {
            return true;
        }
        
        args.iter().any(|arg| {
            self.allowed_args.iter().any(|allowed| {
                arg == allowed || arg.contains(allowed)
            })
        })
    }
    
    /// Check if a path is allowed
    pub fn check_path(&self, path: &std::path::Path) -> bool {
        // Check denied paths first
        for denied in &self.denied_paths {
            if path.starts_with(denied) {
                return false;
            }
        }
        
        // If no allowed paths specified, allow all
        if self.allowed_paths.is_empty() {
            return true;
        }
        
        // Check allowed paths
        self.allowed_paths.iter().any(|allowed| path.starts_with(allowed))
    }
}

/// Permission gate for tools
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionGate {
    tools: HashSet<String>,
    permissions: std::collections::HashMap<String, ToolPermission>,
    default_level: PermissionLevel,
    allow_unknown: bool,
}

impl PermissionGate {
    pub fn new() -> Self {
        Self {
            tools: HashSet::new(),
            permissions: std::collections::HashMap::new(),
            default_level: PermissionLevel::Execute,
            allow_unknown: false,
        }
    }
    
    /// Create a permission gate with default settings
    pub fn default_permissions() -> Self {
        let mut gate = Self::new();
        
        // Git permissions
        gate.register_tool(ToolPermission::new("git")
            .with_level(PermissionLevel::Full)
            .allow_path(PathBuf::from("/"))
            .allow_arg("status")
            .allow_arg("diff")
            .allow_arg("log")
            .allow_arg("add")
            .allow_arg("commit")
            .allow_arg("push")
            .allow_arg("pull")
            .requires_approval());
        
        // npm/yarn permissions
        gate.register_tool(ToolPermission::new("npm")
            .with_level(PermissionLevel::Execute)
            .allow_path(PathBuf::from("/"))
            .allow_arg("install")
            .allow_arg("run")
            .allow_arg("test")
            .allow_arg("build"));
        
        gate.register_tool(ToolPermission::new("yarn")
            .with_level(PermissionLevel::Execute)
            .allow_path(PathBuf::from("/"))
            .allow_arg("install")
            .allow_arg("run")
            .allow_arg("test")
            .allow_arg("build"));
        
        // File operations
        gate.register_tool(ToolPermission::new("file_read")
            .with_level(PermissionLevel::ReadOnly)
            .allow_path(PathBuf::from("/")));
        
        gate.register_tool(ToolPermission::new("file_write")
            .with_level(PermissionLevel::Execute)
            .allow_path(PathBuf::from("/")));
        
        // curl permissions
        gate.register_tool(ToolPermission::new("curl")
            .with_level(PermissionLevel::ReadOnly)
            .allow_arg("-X GET")
            .allow_arg("-X HEAD"));
        
        // Dangerous commands - require approval
        gate.register_tool(ToolPermission::new("rm")
            .with_level(PermissionLevel::Execute)
            .allow_path(PathBuf::from("/tmp"))
            .requires_approval());
        
        gate.register_tool(ToolPermission::new("sudo")
            .with_level(PermissionLevel::Denied));
        
        gate.register_tool(ToolPermission::new("chmod")
            .with_level(PermissionLevel::Execute)
            .requires_approval());
        
        gate
    }
    
    /// Register a tool with the permission gate
    pub fn register_tool(&mut self, permission: ToolPermission) {
        self.tools.insert(permission.name.clone());
        self.permissions.insert(permission.name.clone(), permission);
    }
    
    /// Set the default permission level for unknown tools
    pub fn set_default_level(&mut self, level: PermissionLevel) {
        self.default_level = level;
    }
    
    /// Allow unknown tools (not in the registry)
    pub fn allow_unknown(&mut self) {
        self.allow_unknown = true;
    }
    
    /// Check if a tool is allowed
    pub fn check_tool(&self, tool: &str) -> SandboxResult<PermissionLevel> {
        if let Some(permission) = self.permissions.get(tool) {
            Ok(permission.level)
        } else if self.allow_unknown {
            Ok(self.default_level)
        } else {
            Err(SandboxError::PermissionDenied(format!(
                "Tool '{}' is not registered in the permission gate",
                tool
            )))
        }
    }
    
    /// Check if a tool is allowed with specific arguments
    pub fn check_command(&self, tool: &str, args: &[String]) -> SandboxResult<PermissionLevel> {
        let level = self.check_tool(tool)?;
        
        if let Some(permission) = self.permissions.get(tool) {
            if !permission.check_args(args) {
                return Err(SandboxError::PermissionDenied(format!(
                    "Arguments not allowed for tool '{}'",
                    tool
                )));
            }
        }
        
        Ok(level)
    }
    
    /// Check if a tool can access a specific path
    pub fn check_path(&self, tool: &str, path: &std::path::Path) -> SandboxResult<bool> {
        let level = self.check_tool(tool)?;
        
        if level == PermissionLevel::Denied {
            return Ok(false);
        }
        
        if let Some(permission) = self.permissions.get(tool) {
            Ok(permission.check_path(path))
        } else {
            Ok(true)
        }
    }
    
    /// Check if a tool requires approval
    pub fn requires_approval(&self, tool: &str) -> bool {
        self.permissions
            .get(tool)
            .map(|p| p.requires_approval)
            .unwrap_or(false)
    }
    
    /// Get all registered tools
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.iter().cloned().collect()
    }
    
    /// Get permission for a specific tool
    pub fn get_permission(&self, tool: &str) -> Option<&ToolPermission> {
        self.permissions.get(tool)
    }
}
