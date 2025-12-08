use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "auth.json";
const AUTH_TOKEN_KEY: &str = "auth_token";
const USER_ID_KEY: &str = "user_id";
const USER_EMAIL_KEY: &str = "user_email";

// Default production backend URL
const DEFAULT_BACKEND_URL: &str = "https://api.automatewithtasker.com";

/// Get the backend URL from environment variable or use default
fn get_backend_url() -> String {
    let url = std::env::var("TASKER_BACKEND_URL").unwrap_or_else(|_| DEFAULT_BACKEND_URL.to_string());
    log::info!("Using backend URL: {}", url);
    url
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

/// Store auth token and user info in persistent store
#[tauri::command]
pub async fn store_auth_token(
    app: AppHandle,
    token: String,
    user_id: String,
    email: String,
) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.set(AUTH_TOKEN_KEY, serde_json::json!(token));
    store.set(USER_ID_KEY, serde_json::json!(user_id));
    store.set(USER_EMAIL_KEY, serde_json::json!(email));

    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

/// Get stored auth token
#[tauri::command]
pub async fn get_auth_token(app: AppHandle) -> Result<Option<String>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    match store.get(AUTH_TOKEN_KEY) {
        Some(value) => Ok(value.as_str().map(|s| s.to_string())),
        None => Ok(None),
    }
}

/// Clear auth token (logout)
#[tauri::command]
pub async fn clear_auth_token(app: AppHandle) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    store.delete(AUTH_TOKEN_KEY);
    store.delete(USER_ID_KEY);
    store.delete(USER_EMAIL_KEY);

    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

/// Check auth status - verifies token with backend and clears if expired
#[tauri::command]
pub async fn check_auth_status(app: AppHandle) -> Result<AuthState, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    // Check if we have a stored token
    let token = match store.get(AUTH_TOKEN_KEY) {
        Some(val) => val.as_str().map(|s| s.to_string()),
        None => None,
    };

    let not_authenticated = AuthState {
        is_authenticated: false,
        user_id: None,
        email: None,
        has_subscription: false,
    };

    if token.is_none() {
        return Ok(not_authenticated);
    }

    let token = token.unwrap();

    // Verify token with backend
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();

    let session_response = client
        .get(format!("{}/api/auth/get-session", backend_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await;

    // If request fails or returns non-success, token is invalid/expired
    match session_response {
        Ok(resp) if resp.status().is_success() => {
            // Token is valid, get stored user info
            let user_id = store
                .get(USER_ID_KEY)
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let email = store
                .get(USER_EMAIL_KEY)
                .and_then(|v| v.as_str().map(|s| s.to_string()));

            // Check subscription status
            let has_subscription = check_subscription(&client, &backend_url, &token).await;

            Ok(AuthState {
                is_authenticated: true,
                user_id,
                email,
                has_subscription,
            })
        }
        _ => {
            // Token invalid/expired - clear stored credentials
            store.delete(AUTH_TOKEN_KEY);
            store.delete(USER_ID_KEY);
            store.delete(USER_EMAIL_KEY);
            let _ = store.save();

            Ok(not_authenticated)
        }
    }
}

/// Open Stripe checkout in default browser
#[tauri::command]
pub async fn open_checkout(app: AppHandle) -> Result<(), String> {
    let token = get_auth_token(app).await?.ok_or("Not authenticated")?;

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

    Ok(())
}

/// Open Stripe customer portal in default browser
#[tauri::command]
pub async fn open_customer_portal(app: AppHandle) -> Result<(), String> {
    let token = get_auth_token(app).await?.ok_or("Not authenticated")?;

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

    Ok(())
}

// Helper function to check subscription status
async fn check_subscription(client: &reqwest::Client, backend_url: &str, token: &str) -> bool {
    let url = format!("{}/subscription/status", backend_url);
    log::info!("Checking subscription at: {}", url);

    match client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                match resp.json::<SubscriptionStatus>().await {
                    Ok(s) => {
                        log::info!("Subscription status: has_subscription={}", s.has_subscription);
                        s.has_subscription
                    }
                    Err(e) => {
                        log::error!("Failed to parse subscription response: {}", e);
                        false
                    }
                }
            } else {
                log::error!("Subscription check failed with status: {}", status);
                false
            }
        }
        Err(e) => {
            log::error!("Subscription check request failed: {}", e);
            false
        }
    }
}

/// Sign up with email and password
#[tauri::command]
pub async fn sign_up_email(
    app: AppHandle,
    email: String,
    password: String,
    name: Option<String>,
) -> Result<AuthState, String> {
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();

    let mut body = serde_json::json!({
        "email": email,
        "password": password,
    });

    if let Some(n) = name {
        body["name"] = serde_json::Value::String(n);
    }

    let response = client
        .post(format!("{}/api/auth/sign-up/email", backend_url))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to sign up: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Sign up failed: {}", error));
    }

    // Parse response and store credentials
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let session_token = body
        .get("session")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .or_else(|| body.get("token").and_then(|t| t.as_str()))
        .ok_or("Missing session token in response")?;

    let user_id = body
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Missing user ID in response")?;

    let user_email = body
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .ok_or("Missing email in response")?;

    // Store credentials
    store_auth_token(
        app,
        session_token.to_string(),
        user_id.to_string(),
        user_email.to_string(),
    )
    .await?;

    // Check subscription status
    let has_subscription = check_subscription(&client, &backend_url, session_token).await;

    Ok(AuthState {
        is_authenticated: true,
        user_id: Some(user_id.to_string()),
        email: Some(user_email.to_string()),
        has_subscription,
    })
}

/// Sign in with email and password
#[tauri::command]
pub async fn sign_in_email(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<AuthState, String> {
    let client = reqwest::Client::new();
    let backend_url = get_backend_url();

    let response = client
        .post(format!("{}/api/auth/sign-in/email", backend_url))
        .json(&serde_json::json!({
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to sign in: {}", e))?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(format!("Sign in failed: {}", error));
    }

    // Parse response and store credentials
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let session_token = body
        .get("session")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .or_else(|| body.get("token").and_then(|t| t.as_str()))
        .ok_or("Missing session token in response")?;

    let user_id = body
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Missing user ID in response")?;

    let user_email = body
        .get("user")
        .and_then(|u| u.get("email"))
        .and_then(|e| e.as_str())
        .ok_or("Missing email in response")?;

    store_auth_token(
        app,
        session_token.to_string(),
        user_id.to_string(),
        user_email.to_string(),
    )
    .await?;

    let has_subscription = check_subscription(&client, &backend_url, session_token).await;

    Ok(AuthState {
        is_authenticated: true,
        user_id: Some(user_id.to_string()),
        email: Some(user_email.to_string()),
        has_subscription,
    })
}
