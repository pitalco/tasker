import { getContext, setContext } from 'svelte';
import * as authService from '$lib/services/authService';
import type { AuthState } from '$lib/services/authService';

const AUTH_KEY = Symbol('auth');

class AuthStore {
	// Auth state
	isAuthenticated = $state(false);
	userId = $state<string | null>(null);
	email = $state<string | null>(null);

	// Loading states
	isLoading = $state(true);
	isSigningIn = $state(false);
	error = $state<string | null>(null);

	constructor() {
		// Initialize on construction
		this.init();
	}

	private async init() {
		// Check initial auth status
		await this.refresh();
	}

	async refresh() {
		this.isLoading = true;
		this.error = null;

		try {
			const state = await authService.checkAuthStatus();
			this.updateFromState(state);
		} catch (e) {
			console.error('Failed to check auth status:', e);
			this.isAuthenticated = false;
			this.userId = null;
			this.email = null;
		} finally {
			this.isLoading = false;
		}
	}

	private updateFromState(state: AuthState) {
		this.isAuthenticated = state.is_authenticated;
		this.userId = state.user_id;
		this.email = state.email;
	}

	// Sign in with email/password
	async signInEmail(email: string, password: string): Promise<boolean> {
		this.isSigningIn = true;
		this.error = null;

		try {
			const state = await authService.signInEmail(email, password);
			this.updateFromState(state);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Invalid email or password';
			console.error('Failed to sign in with email:', e);
			return false;
		} finally {
			this.isSigningIn = false;
		}
	}

	// Sign up with email/password
	async signUpEmail(email: string, password: string, name?: string): Promise<boolean> {
		this.isSigningIn = true;
		this.error = null;

		try {
			const state = await authService.signUpEmail(email, password, name);
			this.updateFromState(state);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to create account';
			console.error('Failed to sign up with email:', e);
			return false;
		} finally {
			this.isSigningIn = false;
		}
	}

	async logout() {
		try {
			await authService.logout();
			this.isAuthenticated = false;
			this.userId = null;
			this.email = null;
		} catch (e) {
			console.error('Failed to logout:', e);
		}
	}

	destroy() {
		// Cleanup if needed
	}
}

// Create and set context
export function createAuthStore(): AuthStore {
	const store = new AuthStore();
	setContext(AUTH_KEY, store);
	return store;
}

// Get store from context
export function getAuthStore(): AuthStore {
	const store = getContext<AuthStore>(AUTH_KEY);
	if (!store) {
		throw new Error('Auth store not found. Did you forget to call createAuthStore()?');
	}
	return store;
}

export type { AuthStore };
