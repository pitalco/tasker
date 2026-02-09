mod commands;
mod db;
mod sidecar;
mod taskfile;

use sidecar::SidecarManager;
use tauri::{Emitter, RunEvent};
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file (if exists)
    // Try current directory first, then parent (for dev where .env is in project root)
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_filename("../.env");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // Initialize logging - always output to stdout in dev mode
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .target(tauri_plugin_log::Target::new(
                        tauri_plugin_log::TargetKind::Stdout,
                    ))
                    .build(),
            )?;

            // Initialize database
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = db::init(&app_handle).await {
                    log::error!("Failed to initialize database: {}", e);
                }
            });

            // Register deep link handler for auth callbacks
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                if let Some(url) = event.urls().first() {
                    let url_str = url.to_string();
                    // Emit to frontend for handling
                    if let Err(e) = handle.emit("deep-link", url_str) {
                        log::error!("Failed to emit deep link event: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Workflow commands
            commands::workflow::get_workflows,
            commands::workflow::get_workflow,
            commands::workflow::create_workflow,
            commands::workflow::update_workflow,
            commands::workflow::delete_workflow,
            // Sidecar commands
            commands::recording::start_sidecar,
            commands::recording::stop_sidecar,
            commands::recording::is_sidecar_running,
            commands::recording::get_sidecar_urls,
            // Recording commands
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::cancel_recording,
            commands::recording::get_recording_status,
            // Replay commands
            commands::replay::get_llm_providers,
            commands::replay::start_replay,
            commands::replay::stop_replay,
            commands::replay::get_replay_status,
            // Runs commands
            commands::runs::list_runs,
            commands::runs::get_run,
            commands::runs::start_run,
            commands::runs::cancel_run,
            commands::runs::delete_run,
            commands::runs::get_run_steps,
            commands::runs::get_run_logs,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Taskfile commands
            commands::taskfile::parse_taskfile,
            commands::taskfile::validate_taskfile,
            commands::taskfile::import_taskfile,
            commands::taskfile::export_taskfile,
            commands::taskfile::suggest_taskfile_filename,
            commands::taskfile::save_taskfile,
            // Webview commands (embedded browser)
            commands::webview::get_window_position,
            commands::webview::create_browser_tab,
            commands::webview::close_browser_tab,
            commands::webview::navigate_tab,
            commands::webview::resize_tab,
            commands::webview::set_tab_visible,
            commands::webview::eval_in_tab,
            commands::webview::on_recording_event,
            commands::webview::pause_recording,
            commands::webview::resume_recording,
            commands::webview::tab_go_back,
            commands::webview::tab_go_forward,
            commands::webview::tab_reload,
            // Files commands
            commands::files::get_all_files,
            commands::files::get_files_for_run,
            commands::files::get_file_content,
            commands::files::delete_file,
            commands::files::download_file,
            // Auth commands
            commands::auth::store_auth_token,
            commands::auth::get_auth_token,
            commands::auth::clear_auth_token,
            commands::auth::check_auth_status,
            // Email/password auth
            commands::auth::sign_up_email,
            commands::auth::sign_in_email,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::Exit = event {
                let _ = SidecarManager::stop();
            }
        });
}
