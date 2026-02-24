use crate::sidecar::SidecarManager;

#[tauri::command]
pub async fn start_sidecar() -> Result<bool, String> {
    SidecarManager::start().await?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_sidecar() -> Result<bool, String> {
    SidecarManager::stop()?;
    Ok(true)
}

#[tauri::command]
pub async fn is_sidecar_running() -> Result<bool, String> {
    Ok(SidecarManager::is_running().await)
}

#[tauri::command]
pub async fn get_sidecar_urls() -> Result<(String, String), String> {
    let base_url = SidecarManager::base_url();
    let ws_url = SidecarManager::ws_url("tauri");
    Ok((base_url, ws_url))
}
