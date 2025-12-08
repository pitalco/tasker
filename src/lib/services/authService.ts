import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface AuthState {
	is_authenticated: boolean;
	user_id: string | null;
	email: string | null;
	has_subscription: boolean;
}

export interface SubscriptionStatus {
	hasSubscription: boolean;
	status: string;
	currentPeriodEnd: string | null;
	cancelAtPeriodEnd: boolean;
}

// Check current auth status
export async function checkAuthStatus(): Promise<AuthState> {
	return invoke<AuthState>('check_auth_status');
}

// Start OAuth flow - opens browser to provider
export async function startOAuth(provider: 'google' | 'github'): Promise<void> {
	return invoke('start_oauth', { provider });
}

// Sign in with email/password
export async function signInEmail(email: string, password: string): Promise<AuthState> {
	return invoke<AuthState>('sign_in_email', { email, password });
}

// Sign up with email/password
export async function signUpEmail(
	email: string,
	password: string,
	name?: string
): Promise<AuthState> {
	return invoke<AuthState>('sign_up_email', { email, password, name });
}

// Verify OAuth callback token (called after deep link from OAuth)
export async function verifyOAuthCallback(token: string): Promise<AuthState> {
	return invoke<AuthState>('verify_oauth_callback', { token });
}

// Store token after deep link callback
export async function storeAuthToken(
	token: string,
	userId: string,
	email: string
): Promise<void> {
	return invoke('store_auth_token', { token, userId, email });
}

// Get stored token
export async function getAuthToken(): Promise<string | null> {
	return invoke<string | null>('get_auth_token');
}

// Clear auth (logout)
export async function logout(): Promise<void> {
	return invoke('clear_auth_token');
}

// Open Stripe checkout
export async function openCheckout(): Promise<void> {
	return invoke('open_checkout');
}

// Open customer portal
export async function openCustomerPortal(): Promise<void> {
	return invoke('open_customer_portal');
}

// Listen for deep link auth callbacks
export function onDeepLink(
	callback: (url: string) => void
): Promise<() => void> {
	return listen<string>('deep-link', (event) => {
		callback(event.payload);
	});
}

// Parse auth token from deep link URL
export function parseAuthToken(url: string): string | null {
	// Expected format: tasker://auth/callback?token=xxx
	if (!url.startsWith('tasker://auth')) {
		return null;
	}

	try {
		// Handle the URL - it might not be a valid URL format
		const queryStart = url.indexOf('?');
		if (queryStart === -1) {
			return null;
		}

		const params = new URLSearchParams(url.slice(queryStart + 1));
		return params.get('token');
	} catch {
		return null;
	}
}

// Handle subscription success/cancel deep links
export function parseSubscriptionResult(url: string): 'success' | 'cancel' | null {
	if (url.startsWith('tasker://subscription/success')) {
		return 'success';
	}
	if (url.startsWith('tasker://subscription/cancel')) {
		return 'cancel';
	}
	return null;
}
