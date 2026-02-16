use crate::error::SandboxResult;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::path::PathBuf;

/// Represents a change in the diff
#[derive(Debug, Clone)]
pub struct DiffChange {
    pub line_number: usize,
    pub content: String,
    pub change_type: DiffChangeType,
}

/// Type of diff change
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffChangeType {
    Equal,
    Insert,
    Delete,
}

/// A unified diff representation
#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub hunks: Vec<DiffHunk>,
}

/// A diff hunk (group of changes)
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub changes: Vec<DiffChange>,
}

/// Diff engine for computing file differences
pub struct DiffEngine;

impl DiffEngine {
    /// Compute a unified diff between two strings
    pub fn unified_diff(
        old: &str,
        new: &str,
        old_path: &PathBuf,
        new_path: &PathBuf,
    ) -> UnifiedDiff {
        let diff = TextDiff::from_lines(old, new);
        let mut hunks = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;
        let mut old_line = 0;
        let mut new_line = 0;
        
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    if current_hunk.is_none() {
                        current_hunk = Some(DiffHunk {
                            old_start: old_line + 1,
                            old_lines: 0,
                            new_start: new_line + 1,
                            new_lines: 0,
                            changes: Vec::new(),
                        });
                    }
                    
                    let hunk = current_hunk.as_mut().unwrap();
                    hunk.old_lines += 1;
                    hunk.changes.push(DiffChange {
                        line_number: old_line,
                        content: change.to_string(),
                        change_type: DiffChangeType::Delete,
                    });
                    old_line += 1;
                }
                ChangeTag::Insert => {
                    if current_hunk.is_none() {
                        current_hunk = Some(DiffHunk {
                            old_start: old_line + 1,
                            old_lines: 0,
                            new_start: new_line + 1,
                            new_lines: 0,
                            changes: Vec::new(),
                        });
                    }
                    
                    let hunk = current_hunk.as_mut().unwrap();
                    hunk.new_lines += 1;
                    hunk.changes.push(DiffChange {
                        line_number: new_line,
                        content: change.to_string(),
                        change_type: DiffChangeType::Insert,
                    });
                    new_line += 1;
                }
                ChangeTag::Equal => {
                    if let Some(hunk) = current_hunk.take() {
                        hunks.push(hunk);
                    }
                    old_line += 1;
                    new_line += 1;
                }
            }
        }
        
        if let Some(hunk) = current_hunk {
            hunks.push(hunk);
        }
        
        UnifiedDiff {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
            hunks,
        }
    }
    
    /// Format a unified diff for display
    pub fn format_unified_diff(diff: &UnifiedDiff) -> String {
        let mut output = String::new();
        
        output.push_str(&format!(
            "--- a/{}\n+++ b/{}\n",
            diff.old_path.display(),
            diff.new_path.display()
        ));
        
        for hunk in &diff.hunks {
            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ));
            
            for change in &hunk.changes {
                let prefix = match change.change_type {
                    DiffChangeType::Equal => " ",
                    DiffChangeType::Insert => "+",
                    DiffChangeType::Delete => "-",
                };
                output.push_str(&format!("{}{}", prefix, change.content));
            }
        }
        
        output
    }
    
    /// Get a simple summary of changes
    pub fn diff_summary(old: &str, new: &str) -> DiffSummary {
        let diff = TextDiff::from_lines(old, new);
        let mut added = 0;
        let mut deleted = 0;
        let mut unchanged = 0;
        
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => deleted += 1,
                ChangeTag::Insert => added += 1,
                ChangeTag::Equal => unchanged += 1,
            }
        }
        
        DiffSummary {
            added,
            deleted,
            unchanged,
        }
    }
}

/// Summary of diff statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub deleted: usize,
    pub unchanged: usize,
}

impl DiffSummary {
    pub fn has_changes(&self) -> bool {
        self.added > 0 || self.deleted > 0
    }
    
    pub fn format(&self) -> String {
        format!(
            "+{} -{}\n",
            self.added, self.deleted
        )
    }
}

/// Compute diff between two files
pub fn compute_file_diff(old_path: &PathBuf, new_path: &PathBuf) -> SandboxResult<UnifiedDiff> {
    let old_content = if old_path.exists() {
        std::fs::read_to_string(old_path)?
    } else {
        String::new()
    };
    
    let new_content = if new_path.exists() {
        std::fs::read_to_string(new_path)?
    } else {
        String::new()
    };
    
    Ok(DiffEngine::unified_diff(&old_content, &new_content, old_path, new_path))
}

/// Generate a side-by-side diff view
pub fn side_by_side_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();
    
    output.push_str("┌─────────────────────────────────────┬─────────────────────────────────────┐\n");
    output.push_str("│ OLD                                │ NEW                                │\n");
    output.push_str("├─────────────────────────────────────┼─────────────────────────────────────┤\n");
    
    for change in diff.iter_all_changes() {
        let (left, right, marker) = match change.tag() {
            ChangeTag::Delete => (change.to_string(), String::new(), "-"),
            ChangeTag::Insert => (String::new(), change.to_string(), "+"),
            ChangeTag::Equal => (change.to_string(), change.to_string(), " "),
        };
        
        // Pad to 35 chars
        let left = format!("{:<35}", left.trim_end());
        let right = format!("{:<35}", right.trim_end());
        
        output.push_str(&format!("│{}│ {} │ {} │\n", marker, left, right));
    }
    
    output.push_str("└─────────────────────────────────────┴─────────────────────────────────────┘\n");
    
    output
}
