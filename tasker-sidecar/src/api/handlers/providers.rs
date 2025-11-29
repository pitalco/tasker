use axum::Json;

use crate::models::ProvidersResponse;

/// List available LLM providers and their supported models
pub async fn list_providers() -> Json<ProvidersResponse> {
    // Return available LLM providers and their models
    let mut providers = std::collections::HashMap::new();

    providers.insert(
        "gemini".to_string(),
        vec![
            "gemini-2.5-flash".to_string(),
            "gemini-2.5-pro".to_string(),
            "gemini-2.0-flash".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-1.5-pro".to_string(),
        ],
    );

    providers.insert(
        "openai".to_string(),
        vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
        ],
    );

    providers.insert(
        "anthropic".to_string(),
        vec![
            "claude-sonnet-4-5-20250929".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
        ],
    );

    Json(ProvidersResponse { providers })
}
