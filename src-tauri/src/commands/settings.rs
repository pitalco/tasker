use crate::db::{self, ApiKeys, AppSettings, UpdateSettingsRequest};

#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    db::get_settings()
        .await
        .map_err(|e| format!("Failed to get settings: {}", e))
}

#[tauri::command]
pub async fn update_settings(
    api_keys: Option<ApiKeys>,
    default_provider: Option<String>,
    default_model: Option<String>,
    default_max_steps: Option<i32>,
) -> Result<AppSettings, String> {
    let req = UpdateSettingsRequest {
        api_keys,
        default_provider,
        default_model,
        default_max_steps,
    };

    db::update_settings(req)
        .await
        .map_err(|e| format!("Failed to update settings: {}", e))
}
