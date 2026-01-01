import { invoke } from '@tauri-apps/api/core';

export interface AuthState {
	is_authenticated: boolean;
	user_id: string | null;
	email: string | null;
}

// Check current auth status
export async function checkAuthStatus(): Promise<AuthState> {
	return invoke<AuthState>('check_auth_status');
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
