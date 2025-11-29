use tauri::{Emitter, Manager, WebviewBuilder, WebviewUrl};
use tauri::LogicalPosition;
use tauri::LogicalSize;

/// Get window position info for debugging
#[tauri::command]
pub async fn get_window_position(window: tauri::Window) -> Result<serde_json::Value, String> {
    let outer_pos = window.outer_position().map_err(|e| e.to_string())?;
    let inner_pos = window.inner_position().map_err(|e| e.to_string())?;
    let outer_size = window.outer_size().map_err(|e| e.to_string())?;
    let inner_size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "outer_position": { "x": outer_pos.x, "y": outer_pos.y },
        "inner_position": { "x": inner_pos.x, "y": inner_pos.y },
        "outer_size": { "width": outer_size.width, "height": outer_size.height },
        "inner_size": { "width": inner_size.width, "height": inner_size.height },
        "scale_factor": scale
    }))
}

/// Create a new browser tab as a child webview
#[tauri::command]
pub async fn create_browser_tab(
    window: tauri::Window,
    url: String,
    label: String,
    bounds: (f64, f64, f64, f64), // x, y, width, height as f64 for precision
) -> Result<String, String> {
    log::info!("Creating browser tab: {} at {} with bounds {:?}", label, url, bounds);

    // Get window position info for debugging
    let window_outer = window.outer_position().ok();
    let window_inner = window.inner_position().ok();
    let scale = window.scale_factor().unwrap_or(1.0);

    log::info!("Window outer_position: {:?}, inner_position: {:?}, scale: {}",
               window_outer, window_inner, scale);

    // Parse the URL - WebviewUrl::External expects a url::Url
    let webview_url = WebviewUrl::External(url.parse().map_err(|e| format!("Invalid URL: {}", e))?);

    // On Linux/GTK, there's a known issue where child webview positions are sometimes
    // interpreted incorrectly. Try creating at origin first, then repositioning.
    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .initialization_script(include_str!("../scripts/recording.js"));

    let target_position = LogicalPosition::new(bounds.0, bounds.1);
    let target_size = LogicalSize::new(bounds.2, bounds.3);

    // Create at origin first (workaround for Linux positioning issues)
    let initial_position = LogicalPosition::new(0.0, 0.0);

    log::info!("Creating webview at origin, will reposition to: {:?}, size: {:?}",
               target_position, target_size);

    // add_child takes the builder directly (not a built webview)
    window
        .add_child(webview_builder, initial_position, target_size)
        .map_err(|e| e.to_string())?;

    // Now reposition the webview to the correct location
    if let Some(webview) = window.get_webview(&label) {
        log::info!("Repositioning webview to target position");

        // Try setting position with a small delay workaround
        webview.set_position(target_position).map_err(|e| e.to_string())?;
        webview.set_size(target_size).map_err(|e| e.to_string())?;

        // Log final position for debugging
        if let Ok(pos) = webview.position() {
            log::info!("Webview final position: {:?}", pos);
        }
    }

    log::info!("Browser tab created: {}", label);
    Ok(label)
}

/// Close a browser tab
#[tauri::command]
pub async fn close_browser_tab(window: tauri::Window, label: String) -> Result<bool, String> {
    log::info!("Closing browser tab: {}", label);

    if let Some(webview) = window.get_webview(&label) {
        webview.close().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Navigate a tab to a new URL
#[tauri::command]
pub async fn navigate_tab(window: tauri::Window, label: String, url: String) -> Result<bool, String> {
    log::info!("Navigating tab {} to {}", label, url);

    if let Some(webview) = window.get_webview(&label) {
        // Use JavaScript navigation to trigger the recording script
        let script = format!("window.location.href = '{}'", url.replace('\'', "\\'"));
        webview.eval(&script).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Resize a tab
#[tauri::command]
pub async fn resize_tab(
    window: tauri::Window,
    label: String,
    bounds: (f64, f64, f64, f64),
) -> Result<bool, String> {
    log::debug!("Resizing tab {} to bounds {:?}", label, bounds);
    if let Some(webview) = window.get_webview(&label) {
        webview
            .set_position(LogicalPosition::new(bounds.0, bounds.1))
            .map_err(|e| e.to_string())?;
        webview
            .set_size(LogicalSize::new(bounds.2, bounds.3))
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Set tab visibility (show/hide)
#[tauri::command]
pub async fn set_tab_visible(
    window: tauri::Window,
    label: String,
    visible: bool,
) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        if visible {
            webview.show().map_err(|e| e.to_string())?;
        } else {
            webview.hide().map_err(|e| e.to_string())?;
        }
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Execute JavaScript in a tab
#[tauri::command]
pub async fn eval_in_tab(
    window: tauri::Window,
    label: String,
    script: String,
) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview.eval(&script).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Handle recording events from the injected script
#[tauri::command]
pub async fn on_recording_event(
    app: tauri::AppHandle,
    action_type: String,
    data: serde_json::Value,
) -> Result<(), String> {
    log::debug!("Recording event: {} - {:?}", action_type, data);

    // Emit to frontend
    app.emit(
        "recording_event",
        serde_json::json!({
            "actionType": action_type,
            "data": data
        }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Pause recording in a tab
#[tauri::command]
pub async fn pause_recording(window: tauri::Window, label: String) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview
            .eval("window.__taskerPaused = true;")
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Resume recording in a tab
#[tauri::command]
pub async fn resume_recording(window: tauri::Window, label: String) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview
            .eval("window.__taskerPaused = false;")
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Go back in tab history
#[tauri::command]
pub async fn tab_go_back(window: tauri::Window, label: String) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview
            .eval("window.history.back();")
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Go forward in tab history
#[tauri::command]
pub async fn tab_go_forward(window: tauri::Window, label: String) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview
            .eval("window.history.forward();")
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}

/// Reload the tab
#[tauri::command]
pub async fn tab_reload(window: tauri::Window, label: String) -> Result<bool, String> {
    if let Some(webview) = window.get_webview(&label) {
        webview
            .eval("window.location.reload();")
            .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("Webview not found".to_string())
    }
}
