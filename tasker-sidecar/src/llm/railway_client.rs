use anyhow::{anyhow, Result};
use keyring::Entry;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SERVICE_NAME: &str = "com.tasker.app";
const AUTH_TOKEN_KEY: &str = "auth_token";
const RAILWAY_API_URL: &str = "https://api.tasker-app.com";

/// Request structure for Railway LLM proxy (OpenAI-compatible)
#[derive(Debug, Clone, Serialize)]
pub struct RailwayChatRequest {
    pub messages: Vec<RailwayMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
}

/// Message in OpenAI format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayMessage {
    pub role: String,
    pub content: Value, // Can be string or array for multimodal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Response from Railway LLM proxy
#[derive(Debug, Clone, Deserialize)]
pub struct RailwayChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<RailwayChoice>,
    pub usage: Option<RailwayUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RailwayChoice {
    pub index: i32,
    pub message: RailwayMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RailwayUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// Error response from Railway
#[derive(Debug, Clone, Deserialize)]
pub struct RailwayError {
    pub error: RailwayErrorDetail,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RailwayErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
}

/// Client for Tasker Fast model via Railway proxy
pub struct RailwayClient {
    client: Client,
    auth_token: String,
}

impl RailwayClient {
    /// Create a new Railway client
    /// Requires authentication token from keyring
    pub fn new() -> Result<Self> {
        let auth_token = get_auth_token()?
            .ok_or_else(|| anyhow!("Not authenticated. Please sign in to use Tasker Fast."))?;

        Ok(Self {
            client: Client::new(),
            auth_token,
        })
    }

    /// Create a new Railway client with a provided token
    pub fn with_token(auth_token: String) -> Self {
        Self {
            client: Client::new(),
            auth_token,
        }
    }

    /// Send a chat completion request to the Railway proxy
    pub async fn chat(&self, request: RailwayChatRequest) -> Result<RailwayChatResponse> {
        let response = self
            .client
            .post(format!("{}/llm/chat", RAILWAY_API_URL))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Railway request failed: {}", e))?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            // Try to parse error response
            if let Ok(error) = serde_json::from_str::<RailwayError>(&body) {
                match error.error.code.as_str() {
                    "missing_token" | "invalid_session" | "auth_failed" => {
                        return Err(anyhow!("Authentication failed. Please sign in again."));
                    }
                    "subscription_required" => {
                        return Err(anyhow!(
                            "Active subscription required to use Tasker Fast. Please subscribe at $15/month."
                        ));
                    }
                    "model_cold_start" => {
                        return Err(anyhow!(
                            "Model is warming up. Please try again in a few moments."
                        ));
                    }
                    _ => {
                        return Err(anyhow!("{}", error.error.message));
                    }
                }
            }

            return Err(anyhow!("Railway API error ({}): {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Railway response: {}", e))
    }

    /// Simple prompt helper
    pub async fn prompt(&self, user_message: &str) -> Result<String> {
        let request = RailwayChatRequest {
            messages: vec![RailwayMessage {
                role: "user".to_string(),
                content: Value::String(user_message.to_string()),
                tool_calls: None,
                tool_call_id: None,
            }],
            max_tokens: Some(4096),
            temperature: Some(0.7),
            tools: None,
            tool_choice: None,
        };

        let response = self.chat(request).await?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No text in response"))
    }
}

/// Get auth token from system keyring
pub fn get_auth_token() -> Result<Option<String>> {
    let entry = Entry::new(SERVICE_NAME, AUTH_TOKEN_KEY)
        .map_err(|e| anyhow!("Failed to access keyring: {}", e))?;

    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("Failed to get auth token: {}", e)),
    }
}

/// Check if user has an active subscription
pub async fn check_subscription_status() -> Result<bool> {
    let token = match get_auth_token()? {
        Some(t) => t,
        None => return Ok(false),
    };

    let client = Client::new();
    let response = client
        .get(format!("{}/subscription/status", RAILWAY_API_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    #[derive(Deserialize)]
    struct SubStatus {
        #[serde(rename = "hasSubscription")]
        has_subscription: bool,
    }

    response
        .json::<SubStatus>()
        .await
        .map(|s| s.has_subscription)
        .map_err(|e| anyhow!("Failed to parse subscription status: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_railway_message_serialization() {
        let msg = RailwayMessage {
            role: "user".to_string(),
            content: Value::String("Hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }
}
