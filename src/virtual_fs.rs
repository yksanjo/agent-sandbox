use crate::error::{SandboxError, SandboxResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a file in the virtual filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFile {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub permissions: u16,
    pub is_executable: bool,
    pub hash: String,
    pub created_at: i64,
    pub modified_at: i64,
}

impl VirtualFile {
    pub fn new(path: PathBuf, content: Vec<u8>) -> Self {
        let hash = Self::compute_hash(&content);
        let now = chrono::Utc::now().timestamp();
        
        Self {
            path,
            content,
            permissions: 0o644,
            is_executable: false,
            hash,
            created_at: now,
            modified_at: now,
        }
    }
    
    pub fn new_executable(path: PathBuf, content: Vec<u8>) -> Self {
        let mut file = Self::new(path, content);
        file.is_executable = true;
        file.permissions = 0o755;
        file
    }
    
    fn compute_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
    
    pub fn update_content(&mut self, content: Vec<u8>) {
        self.hash = Self::compute_hash(&content);
        self.content = content;
        self.modified_at = chrono::Utc::now().timestamp();
    }
}

/// Virtual filesystem with diff tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VirtualFilesystem {
    files: HashMap<PathBuf, VirtualFile>,
    deleted_files: HashMap<PathBuf, VirtualFile>,
    mount_points: Vec<PathBuf>,
}

impl VirtualFilesystem {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            deleted_files: HashMap::new(),
            mount_points: Vec::new(),
        }
    }
    
    /// Create a new virtual filesystem from a real directory
    pub fn from_directory(path: &Path) -> SandboxResult<Self> {
        let mut vfs = Self::new();
        vfs.mount(path)?;
        Ok(vfs)
    }
    
    /// Mount a real directory into the virtual filesystem
    pub fn mount(&mut self, path: &Path) -> SandboxResult<()> {
        if !path.exists() {
            return Err(SandboxError::FileSystemError(format!(
                "Directory does not exist: {}",
                path.display()
            )));
        }
        
        self.mount_points.push(path.to_path_buf());
        
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let relative_path = entry
                    .path()
                    .strip_prefix(path)
                    .unwrap()
                    .to_path_buf();
                
                let content = std::fs::read(entry.path())?;
                let is_executable = {
                    use std::os::unix::fs::PermissionsExt;
                    entry.metadata()
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
                };
                
                let file = if is_executable {
                    VirtualFile::new_executable(relative_path.clone(), content)
                } else {
                    VirtualFile::new(relative_path.clone(), content)
                };
                
                self.files.insert(relative_path, file);
            }
        }
        
        Ok(())
    }
    
    /// Read a file from the virtual filesystem
    pub fn read(&self, path: &Path) -> SandboxResult<Vec<u8>> {
        self.files
            .get(path)
            .map(|f| f.content.clone())
            .ok_or_else(|| SandboxError::VirtualFileNotFound(path.display().to_string()))
    }
    
    /// Write a file to the virtual filesystem
    pub fn write(&mut self, path: PathBuf, content: Vec<u8>) {
        let file = VirtualFile::new(path.clone(), content);
        self.files.insert(path, file);
    }
    
    /// Delete a file from the virtual filesystem
    pub fn delete(&mut self, path: &Path) -> SandboxResult<()> {
        if let Some(file) = self.files.remove(path) {
            self.deleted_files.insert(path.to_path_buf(), file);
            Ok(())
        } else {
            Err(SandboxError::VirtualFileNotFound(path.display().to_string()))
        }
    }
    
    /// Check if a file exists
    pub fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path) || self.deleted_files.contains_key(path)
    }
    
    /// Get a file's metadata
    pub fn get_metadata(&self, path: &Path) -> SandboxResult<VirtualFile> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| SandboxError::VirtualFileNotFound(path.display().to_string()))
    }
    
    /// List all files in the virtual filesystem
    pub fn list_files(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }
    
    /// Get the diff between current state and original state
    pub fn get_diff(&self) -> Vec<FileDiff> {
        let mut diffs = Vec::new();
        
        // New and modified files
        for (path, file) in &self.files {
            diffs.push(FileDiff {
                path: path.clone(),
                operation: DiffOperation::Modified,
                old_content: None,
                new_content: Some(String::from_utf8_lossy(&file.content).to_string()),
            });
        }
        
        // Deleted files
        for (path, file) in &self.deleted_files {
            diffs.push(FileDiff {
                path: path.clone(),
                operation: DiffOperation::Deleted,
                old_content: Some(String::from_utf8_lossy(&file.content).to_string()),
                new_content: None,
            });
        }
        
        diffs
    }
    
    /// Reset the virtual filesystem to its original state
    pub fn reset(&mut self) {
        // Restore deleted files
        for (path, file) in self.deleted_files.drain() {
            self.files.insert(path, file);
        }
    }
    
    /// Commit changes (apply deletions)
    pub fn commit(&mut self) {
        self.deleted_files.clear();
    }
}

/// Represents a diff operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffOperation {
    Added,
    Modified,
    Deleted,
}

/// Represents a file diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub operation: DiffOperation,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

impl FileDiff {
    /// Format the diff for display
    pub fn format(&self) -> String {
        match self.operation {
            DiffOperation::Added => {
                format!(
                    "+++ {}\n{}\n",
                    self.path.display(),
                    self.new_content.as_deref().unwrap_or("")
                )
            }
            DiffOperation::Modified => {
                format!(
                    "M  {}\n--- a/{}\n+++ b/{}\n{}\n",
                    self.path.display(),
                    self.path.display(),
                    self.path.display(),
                    self.new_content.as_deref().unwrap_or("")
                )
            }
            DiffOperation::Deleted => {
                format!(
                    "D  {}\n{}\n",
                    self.path.display(),
                    self.old_content.as_deref().unwrap_or("")
                )
            }
        }
    }
}
