use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::registry::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Validate that a path is within the allowed directories (security-critical).
/// Returns the canonicalized path if valid.
pub fn validate_path(path: &str, allowed_dirs: &[PathBuf]) -> std::result::Result<PathBuf, String> {
    if allowed_dirs.is_empty() {
        return Err(
            "No filesystem access configured. Add allowed directories in Settings > Filesystem Access."
                .to_string(),
        );
    }

    let requested = PathBuf::from(path);

    // For existing paths, canonicalize directly
    if requested.exists() {
        let canonical = std::fs::canonicalize(&requested)
            .map_err(|e| format!("Cannot resolve path '{}': {}", path, e))?;

        for dir in allowed_dirs {
            if let Ok(allowed_canonical) = std::fs::canonicalize(dir) {
                if canonical.starts_with(&allowed_canonical) {
                    return Ok(canonical);
                }
            }
        }

        return Err(format!(
            "Access denied: '{}' is outside allowed directories",
            path
        ));
    }

    // For new files: canonicalize the parent directory and append the filename
    if let Some(parent) = requested.parent() {
        let parent_path = if parent.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            parent.to_path_buf()
        };

        if parent_path.exists() {
            let canonical_parent = std::fs::canonicalize(&parent_path)
                .map_err(|e| format!("Cannot resolve parent directory: {}", e))?;

            if let Some(filename) = requested.file_name() {
                let full_path = canonical_parent.join(filename);

                for dir in allowed_dirs {
                    if let Ok(allowed_canonical) = std::fs::canonicalize(dir) {
                        if full_path.starts_with(&allowed_canonical) {
                            return Ok(full_path);
                        }
                    }
                }
            }
        }
    }

    Err(format!(
        "Access denied: '{}' is outside allowed directories or parent directory does not exist",
        path
    ))
}

/// Validate that both source and destination paths are within allowed directories.
fn validate_path_pair(
    source: &str,
    dest: &str,
    allowed_dirs: &[PathBuf],
) -> std::result::Result<(PathBuf, PathBuf), String> {
    let src = validate_path(source, allowed_dirs)?;
    let dst = validate_path(dest, allowed_dirs)?;
    Ok((src, dst))
}

// ============================================================================
// Filesystem Tools
// ============================================================================

/// Read a real file from disk
pub struct FsReadFileTool;

#[async_trait]
impl Tool for FsReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read a real file from disk. Path must be within allowed directories configured in Settings.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read (default: 1048576 = 1MB)",
                        "default": 1048576
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path = match params["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'path' parameter")),
        };
        let max_bytes = params["max_bytes"].as_i64().unwrap_or(1_048_576) as usize;

        let canonical = match validate_path(path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };

        if !canonical.is_file() {
            return Ok(ToolResult::error(format!("Not a file: {}", path)));
        }

        match tokio::fs::read(&canonical).await {
            Ok(bytes) => {
                let truncated = bytes.len() > max_bytes;
                let data = if truncated {
                    &bytes[..max_bytes]
                } else {
                    &bytes
                };

                match String::from_utf8(data.to_vec()) {
                    Ok(text) => {
                        let msg = if truncated {
                            format!(
                                "Read {} bytes (truncated from {} bytes): {}",
                                max_bytes,
                                bytes.len(),
                                path
                            )
                        } else {
                            format!("Read {} bytes: {}", bytes.len(), path)
                        };
                        Ok(ToolResult::success_with_data(msg, json!({ "content": text })))
                    }
                    Err(_) => {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                        Ok(ToolResult::success_with_data(
                            format!("Read binary file ({} bytes): {}", bytes.len(), path),
                            json!({ "content_base64": b64, "is_binary": true }),
                        ))
                    }
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to read '{}': {}", path, e))),
        }
    }
}

/// Write content to a real file on disk
pub struct FsWriteFileTool;

#[async_trait]
impl Tool for FsWriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_write_file".to_string(),
            description: "Write content to a real file on disk. Creates parent directories if needed. Path must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "If true, append to existing file instead of overwriting (default: false)",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path = match params["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'path' parameter")),
        };
        let content = match params["content"].as_str() {
            Some(c) => c,
            None => return Ok(ToolResult::error("Missing 'content' parameter")),
        };
        let append = params["append"].as_bool().unwrap_or(false);

        let canonical = match validate_path(path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };

        // Create parent directories if needed
        if let Some(parent) = canonical.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Ok(ToolResult::error(format!(
                        "Failed to create directories: {}",
                        e
                    )));
                }
            }
        }

        let result = if append {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&canonical)
                .await;
            match file {
                Ok(ref mut f) => f.write_all(content.as_bytes()).await,
                Err(e) => Err(e),
            }
        } else {
            tokio::fs::write(&canonical, content.as_bytes()).await
        };

        match result {
            Ok(()) => {
                let action = if append { "Appended" } else { "Wrote" };
                Ok(ToolResult::success(format!(
                    "{} {} bytes to '{}'",
                    action,
                    content.len(),
                    path
                )))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to write '{}': {}",
                path, e
            ))),
        }
    }
}

/// List directory contents
pub struct FsListDirectoryTool;

#[async_trait]
impl Tool for FsListDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_list_directory".to_string(),
            description: "List contents of a directory with file size and type info. Path must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the directory"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "If true, list recursively (default: false, max depth 3)",
                        "default": false
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter entries (e.g., '*.rs', '*.txt')"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path = match params["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'path' parameter")),
        };
        let recursive = params["recursive"].as_bool().unwrap_or(false);
        let pattern = params["pattern"].as_str();

        let canonical = match validate_path(path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };

        if !canonical.is_dir() {
            return Ok(ToolResult::error(format!("Not a directory: {}", path)));
        }

        let mut entries = Vec::new();
        let max_entries = 500;

        if recursive {
            list_recursive(&canonical, &canonical, pattern, &mut entries, 0, 3, max_entries);
        } else {
            list_flat(&canonical, pattern, &mut entries, max_entries);
        }

        let entries_json: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "path": e.path,
                    "is_dir": e.is_dir,
                    "size": e.size,
                })
            })
            .collect();

        let truncated = entries.len() >= max_entries;
        let msg = if truncated {
            format!(
                "Listed {} entries (truncated at {}) in '{}'",
                entries.len(),
                max_entries,
                path
            )
        } else {
            format!("Listed {} entries in '{}'", entries.len(), path)
        };

        Ok(ToolResult::success_with_data(
            msg,
            json!({ "entries": entries_json }),
        ))
    }
}

struct DirEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

fn matches_glob(name: &str, pattern: &str) -> bool {
    // Simple glob matching: supports * and ?
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    glob_match(&pattern_chars, &name_chars, 0, 0)
}

fn glob_match(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    if pattern[pi] == '*' {
        // Match zero or more characters
        for i in ti..=text.len() {
            if glob_match(pattern, text, pi + 1, i) {
                return true;
            }
        }
        return false;
    }
    if ti == text.len() {
        return false;
    }
    if pattern[pi] == '?' || pattern[pi].to_lowercase().eq(text[ti].to_lowercase()) {
        return glob_match(pattern, text, pi + 1, ti + 1);
    }
    false
}

fn list_flat(dir: &Path, pattern: Option<&str>, entries: &mut Vec<DirEntry>, max: usize) {
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if entries.len() >= max {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(pat) = pattern {
                if !matches_glob(&name, pat) {
                    continue;
                }
            }
            let metadata = entry.metadata();
            let (is_dir, size) = metadata
                .map(|m| (m.is_dir(), m.len()))
                .unwrap_or((false, 0));

            entries.push(DirEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
                size,
            });
        }
    }
}

fn list_recursive(
    base: &Path,
    dir: &Path,
    pattern: Option<&str>,
    entries: &mut Vec<DirEntry>,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
) {
    if depth > max_depth || entries.len() >= max_entries {
        return;
    }
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if entries.len() >= max_entries {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata();
            let (is_dir, size) = metadata
                .map(|m| (m.is_dir(), m.len()))
                .unwrap_or((false, 0));

            let matches = pattern
                .map(|pat| matches_glob(&name, pat) || is_dir)
                .unwrap_or(true);

            if matches || is_dir {
                if pattern.is_none() || matches_glob(&name, pattern.unwrap_or("*")) {
                    entries.push(DirEntry {
                        name: name.clone(),
                        path: entry.path().to_string_lossy().to_string(),
                        is_dir,
                        size,
                    });
                }
            }

            if is_dir {
                list_recursive(
                    base,
                    &entry.path(),
                    pattern,
                    entries,
                    depth + 1,
                    max_depth,
                    max_entries,
                );
            }
        }
    }
}

/// Delete a file (not directories for safety)
pub struct FsDeleteFileTool;

#[async_trait]
impl Tool for FsDeleteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_delete_file".to_string(),
            description: "Delete a file from disk. Cannot delete directories (for safety). Path must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to delete"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path = match params["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'path' parameter")),
        };

        let canonical = match validate_path(path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };

        if canonical.is_dir() {
            return Ok(ToolResult::error(
                "Cannot delete directories. Only files can be deleted for safety.",
            ));
        }

        if !canonical.exists() {
            return Ok(ToolResult::error(format!("File not found: {}", path)));
        }

        match tokio::fs::remove_file(&canonical).await {
            Ok(()) => Ok(ToolResult::success(format!("Deleted file: {}", path))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to delete '{}': {}",
                path, e
            ))),
        }
    }
}

/// Move/rename a file
pub struct FsMoveFileTool;

#[async_trait]
impl Tool for FsMoveFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_move_file".to_string(),
            description: "Move or rename a file. Both source and destination must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Absolute path of the file to move"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Absolute path of the destination"
                    }
                },
                "required": ["source", "destination"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let source = match params["source"].as_str() {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing 'source' parameter")),
        };
        let dest = match params["destination"].as_str() {
            Some(d) => d,
            None => return Ok(ToolResult::error("Missing 'destination' parameter")),
        };

        let (src_path, dst_path) =
            match validate_path_pair(source, dest, &ctx.allowed_directories) {
                Ok(p) => p,
                Err(e) => return Ok(ToolResult::error(e)),
            };

        if !src_path.exists() {
            return Ok(ToolResult::error(format!("Source not found: {}", source)));
        }

        // Create parent dir for destination if needed
        if let Some(parent) = dst_path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Ok(ToolResult::error(format!(
                        "Failed to create destination directories: {}",
                        e
                    )));
                }
            }
        }

        match tokio::fs::rename(&src_path, &dst_path).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Moved '{}' to '{}'",
                source, dest
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to move '{}' to '{}': {}",
                source, dest, e
            ))),
        }
    }
}

/// Get file info (size, modified date, permissions, is_dir)
pub struct FsFileInfoTool;

#[async_trait]
impl Tool for FsFileInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_file_info".to_string(),
            description: "Get information about a file or directory: size, modification date, type. Path must be within allowed directories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file or directory"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let path = match params["path"].as_str() {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing 'path' parameter")),
        };

        let canonical = match validate_path(path, &ctx.allowed_directories) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };

        match tokio::fs::metadata(&canonical).await {
            Ok(metadata) => {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                    });

                let file_type = if metadata.is_dir() {
                    "directory"
                } else if metadata.is_file() {
                    "file"
                } else {
                    "other"
                };

                let info = json!({
                    "path": path,
                    "type": file_type,
                    "size_bytes": metadata.len(),
                    "modified_unix": modified,
                    "readonly": metadata.permissions().readonly(),
                });

                Ok(ToolResult::success_with_data(
                    format!(
                        "{}: {} ({} bytes)",
                        file_type,
                        path,
                        metadata.len()
                    ),
                    info,
                ))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to get info for '{}': {}",
                path, e
            ))),
        }
    }
}
