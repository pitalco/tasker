<script lang="ts">
	import { getAuthStore } from '$lib/stores/auth.svelte';

	let { onClose }: { onClose: () => void } = $props();

	const auth = getAuthStore();

	let email = $state('');
	let password = $state('');
	let name = $state('');
	let isSignUp = $state(false);
	let oauthPending = $state(false);

	async function handleEmailSubmit(e: Event) {
		e.preventDefault();
		if (!email.trim() || !password.trim()) return;

		let success: boolean;
		if (isSignUp) {
			success = await auth.signUpEmail(email.trim(), password, name.trim() || undefined);
		} else {
			success = await auth.signInEmail(email.trim(), password);
		}

		if (success) {
			onClose();
		}
	}

	async function handleOAuth(provider: 'google' | 'github') {
		oauthPending = true;
		await auth.startOAuth(provider);
		// Modal stays open - will close when deep link callback arrives
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}

	function toggleMode() {
		isSignUp = !isSignUp;
		auth.error = null;
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
	onclick={handleBackdropClick}
>
	<div
		class="bg-white border-4 border-black p-8 max-w-md w-full mx-4"
		style="box-shadow: 8px 8px 0 0 #000;"
	>
		<h2 class="text-2xl font-black text-black mb-2">
			{isSignUp ? 'CREATE ACCOUNT' : 'SIGN IN'}
		</h2>
		<p class="text-black/60 font-medium mb-6">
			{isSignUp ? 'Create an account to access Tasker Fast' : 'Sign in to access Tasker Fast cloud models'}
		</p>

		{#if auth.error}
			<div class="bg-red-100 border-3 border-black p-3 mb-4">
				<span class="font-bold text-black text-sm">{auth.error}</span>
			</div>
		{/if}

		{#if oauthPending}
			<div
				class="bg-brutal-lime border-3 border-black p-4 text-center"
				style="box-shadow: 4px 4px 0 0 #000;"
			>
				<svg
					class="w-12 h-12 mx-auto mb-3 animate-spin"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					viewBox="0 0 24 24"
				>
					<circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
					<path d="M12 2a10 10 0 0 1 10 10" stroke-linecap="round" />
				</svg>
				<p class="font-bold text-black text-lg">Complete sign in</p>
				<p class="text-black/70 mt-2">
					Continue in your browser to authenticate.
				</p>
				<p class="text-sm text-black/50 mt-4">
					After signing in, you'll be redirected back to Tasker.
				</p>
			</div>

			<button
				type="button"
				onclick={() => {
					oauthPending = false;
					auth.isSigningIn = false;
				}}
				class="w-full mt-4 text-center font-bold text-black/60 hover:text-black"
			>
				Cancel
			</button>
		{:else}
			<!-- OAuth Buttons -->
			<div class="space-y-3 mb-4">
				<button
					type="button"
					onclick={() => handleOAuth('google')}
					disabled={auth.isSigningIn}
					class="w-full flex items-center justify-center gap-3 bg-white text-black font-bold py-3 px-4 border-3 border-black disabled:opacity-50 disabled:cursor-not-allowed hover:translate-x-1 hover:-translate-y-1 hover:shadow-[4px_4px_0_0_#000] transition-all"
				>
					<svg class="w-5 h-5" viewBox="0 0 24 24">
						<path
							fill="#4285F4"
							d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
						/>
						<path
							fill="#34A853"
							d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
						/>
						<path
							fill="#FBBC05"
							d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
						/>
						<path
							fill="#EA4335"
							d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
						/>
					</svg>
					Continue with Google
				</button>

				<button
					type="button"
					onclick={() => handleOAuth('github')}
					disabled={auth.isSigningIn}
					class="w-full flex items-center justify-center gap-3 bg-[#24292e] text-white font-bold py-3 px-4 border-3 border-black disabled:opacity-50 disabled:cursor-not-allowed hover:translate-x-1 hover:-translate-y-1 hover:shadow-[4px_4px_0_0_#000] transition-all"
				>
					<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
						<path
							d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"
						/>
					</svg>
					Continue with GitHub
				</button>
			</div>

			<!-- Divider -->
			<div class="flex items-center gap-4 my-6">
				<div class="flex-1 h-0.5 bg-black/20"></div>
				<span class="text-black/40 font-bold text-sm">OR</span>
				<div class="flex-1 h-0.5 bg-black/20"></div>
			</div>

			<!-- Email/Password Form -->
			<form onsubmit={handleEmailSubmit}>
				{#if isSignUp}
					<label class="block text-sm font-bold text-black uppercase mb-2" for="name">
						Name (optional)
					</label>
					<input
						id="name"
						type="text"
						bind:value={name}
						placeholder="Your name"
						class="w-full px-4 py-3 border-3 border-black font-medium focus:outline-none focus:ring-2 focus:ring-black/20 mb-4"
						disabled={auth.isSigningIn}
					/>
				{/if}

				<label class="block text-sm font-bold text-black uppercase mb-2" for="email">
					Email Address
				</label>
				<input
					id="email"
					type="email"
					bind:value={email}
					placeholder="you@example.com"
					class="w-full px-4 py-3 border-3 border-black font-medium focus:outline-none focus:ring-2 focus:ring-black/20 mb-4"
					disabled={auth.isSigningIn}
				/>

				<label class="block text-sm font-bold text-black uppercase mb-2" for="password">
					Password
				</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					placeholder="••••••••"
					class="w-full px-4 py-3 border-3 border-black font-medium focus:outline-none focus:ring-2 focus:ring-black/20 mb-4"
					disabled={auth.isSigningIn}
				/>

				<button
					type="submit"
					disabled={auth.isSigningIn || !email.trim() || !password.trim()}
					class="w-full bg-brutal-cyan text-black font-bold py-3 border-3 border-black disabled:opacity-50 disabled:cursor-not-allowed hover:translate-x-1 hover:-translate-y-1 hover:shadow-[4px_4px_0_0_#000] transition-all"
				>
					{#if auth.isSigningIn}
						{isSignUp ? 'CREATING ACCOUNT...' : 'SIGNING IN...'}
					{:else}
						{isSignUp ? 'CREATE ACCOUNT' : 'SIGN IN'}
					{/if}
				</button>
			</form>

			<p class="text-sm text-black/60 mt-4 text-center">
				{isSignUp ? 'Already have an account?' : "Don't have an account?"}
				<button
					type="button"
					onclick={toggleMode}
					class="font-bold text-black hover:underline ml-1"
				>
					{isSignUp ? 'Sign in' : 'Create one'}
				</button>
			</p>
		{/if}

		{#if !oauthPending}
			<button
				type="button"
				onclick={onClose}
				class="w-full mt-4 text-center font-bold text-black/60 hover:text-black"
			>
				Cancel
			</button>
		{/if}
	</div>
</div>
