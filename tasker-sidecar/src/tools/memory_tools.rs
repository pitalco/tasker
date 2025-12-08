use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::registry::{Memory, Tool, ToolContext, ToolDefinition, ToolResult};

// ============================================================================
// Save Memory Tool (Upsert)
// ============================================================================

pub struct SaveMemoryTool;

#[async_trait]
impl Tool for SaveMemoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "save_memory".to_string(),
            description: "Save a note/memory for later recall during this run. Use this to remember important information, observations, extracted data, or intermediate results. If a key is provided and already exists, the memory will be updated.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to remember (e.g., 'Found 3 search results', 'User email is test@example.com')"
                    },
                    "key": {
                        "type": "string",
                        "description": "Optional unique key for easy recall (e.g., 'user_email', 'search_count'). If provided and exists, updates the existing memory."
                    },
                    "category": {
                        "type": "string",
                        "description": "Optional category (e.g., 'observation', 'extracted_data', 'todo', 'result')"
                    }
                },
                "required": ["content"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;
        let key = params["key"].as_str().map(|s| s.to_string());
        let category = params["category"].as_str().map(|s| s.to_string());

        let mut memories = ctx.memories.write().await;

        // If key provided, check if it already exists (upsert)
        if let Some(ref k) = key {
            if let Some(existing) = memories.iter_mut().find(|m| m.key.as_ref() == Some(k)) {
                existing.content = content.to_string();
                existing.category = category;
                return Ok(ToolResult::success(format!(
                    "Updated memory [{}]: {}",
                    k,
                    truncate(content, 100)
                )));
            }
        }

        // Create new memory
        let memory = Memory::new(content, key.clone(), category);
        memories.push(memory);

        let msg = if let Some(k) = key {
            format!("Saved memory [{}]: {}", k, truncate(content, 100))
        } else {
            format!("Saved memory: {}", truncate(content, 100))
        };

        Ok(ToolResult::success(msg))
    }
}

// ============================================================================
// Recall Memories Tool
// ============================================================================

pub struct RecallMemoriesTool;

#[async_trait]
impl Tool for RecallMemoriesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "recall_memories".to_string(),
            description: "Recall saved memories from this run. Can retrieve a specific memory by key, filter by category, or list all memories.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Optional specific key to retrieve a single memory"
                    },
                    "category": {
                        "type": "string",
                        "description": "Optional category filter (e.g., 'observation', 'extracted_data')"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let memories = ctx.memories.read().await;

        // If specific key requested, get that one memory
        if let Some(key) = params["key"].as_str() {
            return match memories.iter().find(|m| m.key.as_ref().map(|k| k.as_str()) == Some(key)) {
                Some(memory) => Ok(ToolResult::success_with_data(
                    format!("Found memory [{}]: {}", key, truncate(&memory.content, 200)),
                    json!({
                        "memory": {
                            "key": memory.key,
                            "content": memory.content,
                            "category": memory.category
                        }
                    }),
                )),
                None => Ok(ToolResult::success(format!("No memory found with key '{}'", key))),
            };
        }

        // Filter by category if provided
        let category_filter = params["category"].as_str();
        let filtered: Vec<&Memory> = memories
            .iter()
            .filter(|m| {
                if let Some(cat) = category_filter {
                    m.category.as_ref().map(|c| c.as_str()) == Some(cat)
                } else {
                    true
                }
            })
            .collect();

        if filtered.is_empty() {
            return Ok(ToolResult::success("No memories found"));
        }

        let memories_json: Vec<Value> = filtered
            .iter()
            .map(|m| {
                json!({
                    "key": m.key,
                    "content": m.content,
                    "category": m.category
                })
            })
            .collect();

        Ok(ToolResult::success_with_data(
            format!("Found {} memories", filtered.len()),
            json!({ "memories": memories_json }),
        ))
    }
}

// ============================================================================
// Delete Memory Tool
// ============================================================================

pub struct DeleteMemoryTool;

#[async_trait]
impl Tool for DeleteMemoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delete_memory".to_string(),
            description: "Delete a memory by its key.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The key of the memory to delete"
                    }
                },
                "required": ["key"]
            }),
        }
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let key = params["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?;

        let mut memories = ctx.memories.write().await;
        let initial_len = memories.len();

        memories.retain(|m| m.key.as_ref().map(|k| k.as_str()) != Some(key));

        if memories.len() < initial_len {
            Ok(ToolResult::success(format!("Deleted memory [{}]", key)))
        } else {
            Ok(ToolResult::error(format!("No memory found with key '{}'", key)))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}
