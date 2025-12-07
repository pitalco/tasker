use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "com.tasker.app";
const AUTH_TOKEN_KEY: &str = "auth_token";
const USER_ID_KEY: &str = "user_id";
const USER_EMAIL_KEY: &str = "user_email";

// Default production backend URL
const DEFAULT_BACKEND_URL: &str = "https://api.automatewithtasker.com";

/// Get the backend URL from environment variable or use default
fn get_backend_url() -> String {
    std::env::var("TASKER_BACKEND_URL").unwrap_or_else(|_| DEFAULT_BACKEND_URL.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub is_authenticated: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub has_subscription: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStatus {
    #[serde(rename = "hasSubscription")]
    pub has_subscription: bool,
    pub status: String,
    #[serde(rename = "currentPeriodEnd")]
    pub current_period_end: Option<String>,
    #[serde(rename = "cancelAtPeriodEnd")]
    pub cancel_at_period_end: bool,
}

/// Store auth token and user info securely in system keyring
#[tauri::command]
pub async fn store_auth_token(
    token: String,
    user_id: String,
    email: String,
) -> Result<(), String> {
    // Store token
    let token_entry = Entry::new(SERVICE_NAME, AUTH_TOKEN_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    token_entry
        .set_password(&token)
        .map_err(|e| format!("Failed to store token: {}", e))?;

    // Store user_id
    let user_entry = Entry::new(SERVICE_NAME, USER_ID_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    user_entry
        .set_password(&user_id)
        .map_err(|e| format!("Failed to store user_id: {}", e))?;

    // Store email
    let email_entry = Entry::new(SERVICE_NAME, USER_EMAIL_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    email_entry
        .set_password(&email)
        .map_err(|e| format!("Failed to store email: {}", e))?;

    log::info!("Auth credentials stored for user: {}", email);
    Ok(())
}

/// Get stored auth token
#[tauri::command]
pub async fn get_auth_token() -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE_NAME, AUTH_TOKEN_KEY)
        .map_err(|e| format!("Failed to access keyring: {}", e))?;

    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to get token: {}", e)),
    }
}

/// Clear auth token (logout)
#[tauri::command]
pub async fn clear_auth_token() -> Result<(), String> {
    // Clear token
    if let Ok(entry) = Entry::new(SERVICE_NAME, AUTH_TOKEN_KEY) {
        let _ = entry.delete_credential();
    }

    // Clear user_id
    if let Ok(entry) = Entry::new(SERVICE_NAME, USER_ID_KEY) {
        let _ = entry.delete_credential();
    }

    // Clear email
    if let Ok(entry) = Entry::new(SERVICE_NAME, USER_EMAIL_KEY) {
        let _ = entry.delete_credential();
    }

    log::info!("Auth credentials cleared");
    Ok(())
}

/// Check auth status with backend
#[tauri::command]
pub async fn check_auth_status() -> Result<AuthState, String> {
    // Get stored token
    let token = match get_auth_token().await? {
        Some(t) => t,
        None => {
            return Ok(AuthState {
                is_authenticated: false,
                user_id: None,
                email: None,
                has_subscription: false,
            })
        }
    };

    // Verify session with backend
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();
    let response = client
        .get(format!("{}/api/auth/session", backend_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Failed to check auth: {}", e))?;

    if !response.status().is_success() {
        // Token invalid, clear it
        let _ = clear_auth_token().await;
        return Ok(AuthState {
            is_authenticated: false,
            user_id: None,
            email: None,
            has_subscription: false,
        });
    }

    // Parse session response
    let session: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let user_id = session
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string());

    let email = session
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .map(|s| s.to_string());

    // Get subscription status
    let sub_response = client
        .get(format!("{}/subscription/status", backend_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    let has_subscription = if let Ok(resp) = sub_response {
        if resp.status().is_success() {
            resp.json::<SubscriptionStatus>()
                .await
                .map(|s| s.has_subscription)
                .unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    Ok(AuthState {
        is_authenticated: true,
        user_id,
        email,
        has_subscription,
    })
}

/// Send magic link email
#[tauri::command]
pub async fn send_magic_link(email: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();
    let response = client
        .post(format!("{}/api/auth/sign-in/magic-link", backend_url))
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(|e| format!("Failed to send magic link: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Failed to send magic link: {}", error));
    }

    log::info!("Magic link sent to: {}", email);
    Ok(())
}

/// Verify magic link token and get session
#[tauri::command]
pub async fn verify_magic_link(token: String) -> Result<AuthState, String> {
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();
    let response = client
        .post(format!("{}/api/auth/magic-link/verify", backend_url))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .map_err(|e| format!("Failed to verify magic link: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Invalid or expired magic link: {}", error));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let session_token = body
        .get("session")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .ok_or("Missing session token in response")?;

    let user_id = body
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Missing user ID in response")?;

    let email = body
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .ok_or("Missing email in response")?;

    // Store credentials
    store_auth_token(
        session_token.to_string(),
        user_id.to_string(),
        email.to_string(),
    )
    .await?;

    // Check subscription status
    let sub_client = reqwest::Client::new();
    let sub_response = sub_client
        .get(format!("{}/subscription/status", backend_url))
        .header("Authorization", format!("Bearer {}", session_token))
        .send()
        .await;

    let has_subscription = if let Ok(resp) = sub_response {
        if resp.status().is_success() {
            resp.json::<SubscriptionStatus>()
                .await
                .map(|s| s.has_subscription)
                .unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    Ok(AuthState {
        is_authenticated: true,
        user_id: Some(user_id.to_string()),
        email: Some(email.to_string()),
        has_subscription,
    })
}

/// Open Stripe checkout in default browser
#[tauri::command]
pub async fn open_checkout() -> Result<(), String> {
    let token = get_auth_token()
        .await?
        .ok_or("Not authenticated")?;

    let client = reqwest::Client::new();
    let backend_url = get_backend_url();
    let response = client
        .post(format!("{}/subscription/checkout", backend_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Failed to create checkout: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Failed to create checkout session: {}", error));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let url = body
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or("No checkout URL in response")?;

    // Open in default browser
    open::that(url).map_err(|e| format!("Failed to open browser: {}", e))?;

    log::info!("Opened checkout in browser");
    Ok(())
}

/// Open Stripe customer portal in default browser
#[tauri::command]
pub async fn open_customer_portal() -> Result<(), String> {
    let token = get_auth_token()
        .await?
        .ok_or("Not authenticated")?;

    let client = reqwest::Client::new();
    let backend_url = get_backend_url();
    let response = client
        .post(format!("{}/subscription/portal", backend_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Failed to create portal session: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Failed to create portal session: {}", error));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let url = body
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or("No portal URL in response")?;

    // Open in default browser
    open::that(url).map_err(|e| format!("Failed to open browser: {}", e))?;

    log::info!("Opened customer portal in browser");
    Ok(())
}
