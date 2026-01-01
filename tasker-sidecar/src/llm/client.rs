use anyhow::{anyhow, Result};
use genai::chat::{ChatMessage, ChatRequest, ChatResponse};
use genai::Client;
use serde::{Deserialize, Serialize};

/// LLM Provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    Anthropic,
    OpenAI,
    Gemini,
}

impl LLMProvider {
    /// Get the genai model ID for this provider and model name
    /// genai 0.4+ auto-detects provider from model prefix (gemini-, gpt-, claude-)
    pub fn model_id(&self, model: &str) -> String {
        // Just return the model name - genai auto-detects provider
        model.to_string()
    }

    /// Get the environment variable name for the API key
    pub fn api_key_env_var(&self) -> &'static str {
        match self {
            LLMProvider::Anthropic => "ANTHROPIC_API_KEY",
            LLMProvider::OpenAI => "OPENAI_API_KEY",
            LLMProvider::Gemini => "GEMINI_API_KEY",
        }
    }
}

impl std::str::FromStr for LLMProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(LLMProvider::Anthropic),
            "openai" | "gpt" => Ok(LLMProvider::OpenAI),
            "gemini" | "google" => Ok(LLMProvider::Gemini),
            _ => Err(anyhow!("Unknown LLM provider: {}", s)),
        }
    }
}

/// LLM Configuration
#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl LLMConfig {
    pub fn new(provider: LLMProvider, model: String) -> Self {
        Self {
            provider,
            model,
            api_key: None,
            max_tokens: Some(4096),
            temperature: Some(0.7),
        }
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// LLM Client for making requests to various providers
pub struct LLMClient {
    client: Client,
    config: LLMConfig,
}

impl LLMClient {
    /// Create a new LLM client with the given configuration
    pub fn new(config: LLMConfig) -> Result<Self> {
        // Set API key in environment if provided (using thread-safe helper)
        if let Some(ref api_key) = config.api_key {
            crate::config::set_api_key_env(config.provider.api_key_env_var(), api_key);
        }

        let client = Client::default();

        Ok(Self { client, config })
    }

    /// Send a chat message and get a response
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let model_id = self.config.provider.model_id(&self.config.model);

        let request = ChatRequest::new(messages);

        // Note: genai 0.1.x has a simpler API without max_tokens/temperature setters
        // These will be configurable in future versions

        let response: ChatResponse = self
            .client
            .exec_chat(&model_id, request, None)
            .await
            .map_err(|e| anyhow!("LLM request failed: {}", e))?;

        // Extract text from response
        let text = response
            .first_text()
            .ok_or_else(|| anyhow!("No text in LLM response"))?
            .to_string();

        Ok(text)
    }

    /// Send a simple prompt and get a response
    pub async fn prompt(&self, prompt: &str) -> Result<String> {
        let messages = vec![ChatMessage::user(prompt)];
        self.chat(messages).await
    }

    /// Send a system prompt and user prompt
    pub async fn prompt_with_system(&self, system: &str, user: &str) -> Result<String> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
        self.chat(messages).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_model_id() {
        // genai 0.4+ auto-detects provider from model name prefix
        let claude = LLMProvider::Anthropic;
        assert_eq!(
            claude.model_id("claude-3-5-sonnet-20241022"),
            "claude-3-5-sonnet-20241022"
        );

        let openai = LLMProvider::OpenAI;
        assert_eq!(openai.model_id("gpt-4o"), "gpt-4o");

        let gemini = LLMProvider::Gemini;
        assert_eq!(gemini.model_id("gemini-2.5-flash"), "gemini-2.5-flash");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(
            "anthropic".parse::<LLMProvider>().unwrap(),
            LLMProvider::Anthropic
        );
        assert_eq!(
            "openai".parse::<LLMProvider>().unwrap(),
            LLMProvider::OpenAI
        );
        assert_eq!(
            "gemini".parse::<LLMProvider>().unwrap(),
            LLMProvider::Gemini
        );
    }
}
