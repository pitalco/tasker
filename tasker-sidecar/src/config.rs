use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use rusqlite::Connection;
use serde::Deserialize;

// SECURITY: Mutex to synchronize environment variable modifications
// This prevents race conditions when multiple threads try to set API keys
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Safely set an environment variable with synchronization
/// This is needed because std::env::set_var is not thread-safe
pub fn set_api_key_env(env_var: &str, value: &str) {
    // Acquire mutex to ensure only one thread modifies env at a time
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: We're holding a mutex to prevent concurrent modifications
    unsafe {
        std::env::set_var(env_var, value);
    }
}

/// Helper to get the environment variable name for a provider
pub fn get_env_var_for_provider(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => "ANTHROPIC_API_KEY",
        "openai" | "gpt" => "OPENAI_API_KEY",
        "gemini" | "google" => "GEMINI_API_KEY",
        _ => "ANTHROPIC_API_KEY",
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8765),
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8765,
            host: "127.0.0.1".to_string(),
        }
    }
}

// API Key management - reads from local Tauri database

#[derive(Debug, Deserialize)]
struct LLMConfig {
    api_keys: ApiKeys,
    default_provider: Option<String>,
    default_model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiKeys {
    gemini: Option<String>,
    openai: Option<String>,
    anthropic: Option<String>,
}

/// Get the path to the Tauri app's database
fn get_db_path() -> Option<PathBuf> {
    // Standard Tauri data directory location
    let data_dir = dirs::data_dir()?;
    let db_path = data_dir.join("com.tasker.app").join("tasker.db");

    if db_path.exists() {
        Some(db_path)
    } else {
        tracing::warn!("Database not found at {:?}", db_path);
        None
    }
}

/// Get API key for a provider from the local database
pub fn get_api_key(provider: &str) -> Option<String> {
    let db_path = get_db_path()?;

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open settings database: {}", e);
            return None;
        }
    };

    let llm_config_json: String = match conn.query_row(
        "SELECT llm_config_json FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    ) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("Failed to query settings: {}", e);
            return None;
        }
    };

    let config: LLMConfig = match serde_json::from_str(&llm_config_json) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to parse LLM config: {}", e);
            return None;
        }
    };

    match provider.to_lowercase().as_str() {
        "gemini" | "google" => config.api_keys.gemini.filter(|k| !k.is_empty()),
        "openai" | "gpt" => config.api_keys.openai.filter(|k| !k.is_empty()),
        "anthropic" | "claude" => config.api_keys.anthropic.filter(|k| !k.is_empty()),
        _ => {
            tracing::warn!("Unknown provider: {}", provider);
            None
        }
    }
}

/// Get the default LLM provider from settings
pub fn get_default_provider() -> Option<String> {
    let db_path = get_db_path()?;

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open settings database: {}", e);
            return None;
        }
    };

    let llm_config_json: String = match conn.query_row(
        "SELECT llm_config_json FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    ) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("Failed to query settings: {}", e);
            return None;
        }
    };

    let config: LLMConfig = match serde_json::from_str(&llm_config_json) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to parse LLM config: {}", e);
            return None;
        }
    };

    config.default_provider
}

/// Get the default LLM model from settings
pub fn get_default_model() -> Option<String> {
    let db_path = get_db_path()?;

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open settings database: {}", e);
            return None;
        }
    };

    let llm_config_json: String = match conn.query_row(
        "SELECT llm_config_json FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    ) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("Failed to query settings: {}", e);
            return None;
        }
    };

    let config: LLMConfig = match serde_json::from_str(&llm_config_json) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to parse LLM config: {}", e);
            return None;
        }
    };

    config.default_model
}
