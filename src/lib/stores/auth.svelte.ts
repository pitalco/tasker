import { getContext, setContext } from 'svelte';
import * as authService from '$lib/services/authService';
import type { AuthState } from '$lib/services/authService';

const AUTH_KEY = Symbol('auth');

class AuthStore {
	// Auth state
	isAuthenticated = $state(false);
	userId = $state<string | null>(null);
	email = $state<string | null>(null);
	hasSubscription = $state(false);

	// Loading states
	isLoading = $state(true);
	isSigningIn = $state(false);
	error = $state<string | null>(null);

	// Deep link cleanup function
	private unlistenDeepLink: (() => void) | null = null;

	constructor() {
		// Initialize on construction
		this.init();
	}

	private async init() {
		// Check initial auth status
		await this.refresh();

		// Set up deep link listener
		this.unlistenDeepLink = await authService.onDeepLink(async (url) => {
			await this.handleDeepLink(url);
		});
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
			this.hasSubscription = false;
		} finally {
			this.isLoading = false;
		}
	}

	private updateFromState(state: AuthState) {
		this.isAuthenticated = state.is_authenticated;
		this.userId = state.user_id;
		this.email = state.email;
		this.hasSubscription = state.has_subscription;
	}

	async sendMagicLink(email: string): Promise<boolean> {
		this.isSigningIn = true;
		this.error = null;

		try {
			await authService.sendMagicLink(email);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to send magic link';
			console.error('Failed to send magic link:', e);
			return false;
		} finally {
			this.isSigningIn = false;
		}
	}

	private async handleDeepLink(url: string) {
		// Check for auth callback
		const token = authService.parseAuthToken(url);
		if (token) {
			await this.verifyMagicLink(token);
			return;
		}

		// Check for subscription result
		const subscriptionResult = authService.parseSubscriptionResult(url);
		if (subscriptionResult === 'success') {
			// Refresh to get updated subscription status
			await this.refresh();
		}
	}

	private async verifyMagicLink(token: string) {
		this.isSigningIn = true;
		this.error = null;

		try {
			const state = await authService.verifyMagicLink(token);
			this.updateFromState(state);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to verify magic link';
			console.error('Failed to verify magic link:', e);
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
			this.hasSubscription = false;
		} catch (e) {
			console.error('Failed to logout:', e);
		}
	}

	async openCheckout() {
		if (!this.isAuthenticated) {
			this.error = 'Please sign in first';
			return;
		}

		try {
			await authService.openCheckout();
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to open checkout';
			console.error('Failed to open checkout:', e);
		}
	}

	async openCustomerPortal() {
		if (!this.isAuthenticated || !this.hasSubscription) {
			this.error = 'No active subscription';
			return;
		}

		try {
			await authService.openCustomerPortal();
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to open customer portal';
			console.error('Failed to open customer portal:', e);
		}
	}

	destroy() {
		if (this.unlistenDeepLink) {
			this.unlistenDeepLink();
			this.unlistenDeepLink = null;
		}
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
