use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

static SIDECAR_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
static SIDECAR_PORT: u16 = 8765;

pub struct SidecarManager;

impl SidecarManager {
    /// Get the base URL for the sidecar API
    pub fn base_url() -> String {
        format!("http://127.0.0.1:{}", SIDECAR_PORT)
    }

    /// Get the WebSocket URL for real-time updates
    pub fn ws_url(client_id: &str) -> String {
        format!("ws://127.0.0.1:{}/ws/{}", SIDECAR_PORT, client_id)
    }

    /// Start the Rust sidecar process
    pub async fn start() -> Result<(), String> {
        // Check if already running
        if Self::is_running().await {
            return Ok(());
        }

        let sidecar_path = Self::get_sidecar_binary()?;

        // Spawn the sidecar process
        // Set RUST_LOG to filter out noisy chromiumoxide errors
        let mut cmd = Command::new(&sidecar_path);

        // Prevent a console window from appearing on Windows
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        cmd.env(
            "RUST_LOG",
            "info,chromiumoxide::conn=warn,chromiumoxide::handler=warn",
        );

        // Pass backend URL if set
        if let Ok(backend_url) = std::env::var("TASKER_BACKEND_URL") {
            cmd.env("TASKER_BACKEND_URL", backend_url);
        }

        let mut child = cmd
            .stdout(Stdio::null()) // Sidecar uses tracing which writes to stderr only
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start sidecar: {}", e))?;

        // Capture stderr in a separate thread (tracing writes here)
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        // Skip noisy chromiumoxide deserialization errors
                        if line.contains("Failed to deserialize WS response")
                            || line
                                .contains("data did not match any variant of untagged enum Message")
                        {
                            continue;
                        }
                        // Only log errors from sidecar
                        if line.contains("ERROR") {
                            log::error!("[sidecar] {}", line);
                        }
                    }
                }
            });
        }

        // Store the process handle
        {
            let mut process = SIDECAR_PROCESS.lock().unwrap();
            *process = Some(child);
        }

        // Wait for the sidecar to be ready
        Self::wait_for_ready().await?;

        Ok(())
    }

    /// Stop the sidecar process
    pub fn stop() -> Result<(), String> {
        let mut process = SIDECAR_PROCESS.lock().unwrap();

        if let Some(mut child) = process.take() {
            child
                .kill()
                .map_err(|e| format!("Failed to kill sidecar: {}", e))?;
        }

        Ok(())
    }

    /// Check if the sidecar is running by hitting the health endpoint
    pub async fn is_running() -> bool {
        let client = reqwest::Client::new();
        let url = format!("{}/health", Self::base_url());

        match client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Wait for the sidecar to be ready
    async fn wait_for_ready() -> Result<(), String> {
        let max_attempts = 30;
        let mut attempts = 0;

        while attempts < max_attempts {
            if Self::is_running().await {
                return Ok(());
            }

            sleep(Duration::from_millis(500)).await;
            attempts += 1;
        }

        log::error!("Sidecar failed to start after {} attempts", max_attempts);
        Err(
            "Sidecar failed to start within timeout (15s). Make sure tasker-sidecar is built."
                .to_string(),
        )
    }

    /// Get the path to the Rust sidecar binary
    fn get_sidecar_binary() -> Result<String, String> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| format!("Failed to get exe path: {}", e))?
            .parent()
            .ok_or("Failed to get exe parent dir")?
            .to_path_buf();

        // Binary name varies by platform
        #[cfg(target_os = "windows")]
        let binary_name = "tasker-sidecar.exe";
        #[cfg(not(target_os = "windows"))]
        let binary_name = "tasker-sidecar";

        // Tauri appends target triple for bundled binaries
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let bundled_name = "tasker-sidecar-x86_64-unknown-linux-gnu";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let bundled_name = "tasker-sidecar-aarch64-unknown-linux-gnu";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let bundled_name = "tasker-sidecar-x86_64-apple-darwin";
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let bundled_name = "tasker-sidecar-aarch64-apple-darwin";
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        let bundled_name = "tasker-sidecar-x86_64-pc-windows-msvc.exe";
        #[cfg(not(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "windows", target_arch = "x86_64")
        )))]
        let bundled_name = binary_name;

        // Try paths in order of preference:

        // 1. Same directory as main exe with target triple (Tauri bundled)
        let tauri_bundled_path = exe_dir.join(bundled_name);
        if tauri_bundled_path.exists() {
            return Ok(tauri_bundled_path.to_string_lossy().to_string());
        }

        // 2. Same directory as main exe (simple bundled)
        let bundled_path = exe_dir.join(binary_name);
        if bundled_path.exists() {
            return Ok(bundled_path.to_string_lossy().to_string());
        }

        // 3. tasker-sidecar/target/debug (development - exe is in src-tauri/target/debug)
        // From target/debug/, we need ../../../tasker-sidecar/target/debug/
        let dev_debug_path = exe_dir
            .join("../../../tasker-sidecar/target/debug")
            .join(binary_name);
        if dev_debug_path.exists() {
            let canonical = dev_debug_path
                .canonicalize()
                .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
            return Ok(canonical.to_string_lossy().to_string());
        }

        // 4. tasker-sidecar/target/release (development - release build)
        let dev_release_path = exe_dir
            .join("../../../tasker-sidecar/target/release")
            .join(binary_name);
        if dev_release_path.exists() {
            let canonical = dev_release_path
                .canonicalize()
                .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
            return Ok(canonical.to_string_lossy().to_string());
        }

        // 5. Try relative to current directory
        let current_dir =
            std::env::current_dir().map_err(|e| format!("Failed to get current dir: {}", e))?;

        // If running from project root
        let from_root = current_dir
            .join("tasker-sidecar/target/debug")
            .join(binary_name);
        if from_root.exists() {
            return Ok(from_root.to_string_lossy().to_string());
        }

        // If running from src-tauri
        let from_tauri = current_dir
            .join("../tasker-sidecar/target/debug")
            .join(binary_name);
        if from_tauri.exists() {
            let canonical = from_tauri
                .canonicalize()
                .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
            return Ok(canonical.to_string_lossy().to_string());
        }

        Err(format!(
            "Sidecar binary not found. Build it with: cd tasker-sidecar && cargo build\nTried: {:?}, {:?}, {:?}, {:?}, {:?}, {:?}",
            tauri_bundled_path, bundled_path, dev_debug_path, dev_release_path, from_root, from_tauri
        ))
    }
}
