<script lang="ts">
	import { getAuthStore } from '$lib/stores/auth.svelte';

	let { onClose }: { onClose: () => void } = $props();

	const auth = getAuthStore();

	let email = $state('');
	let password = $state('');
	let isSignUp = $state(false);

	async function handleEmailSubmit(e: Event) {
		e.preventDefault();
		if (!email.trim() || !password.trim()) return;

		let success: boolean;
		if (isSignUp) {
			success = await auth.signUpEmail(email.trim(), password);
		} else {
			success = await auth.signInEmail(email.trim(), password);
		}

		if (success) {
			onClose();
		}
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
<div
	class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 cursor-pointer"
	onclick={handleBackdropClick}
>
	<div
		class="bg-white border-4 border-black p-8 max-w-md w-full mx-4 cursor-default"
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

		<!-- Email/Password Form -->
		<form onsubmit={handleEmailSubmit}>
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
				class="w-full bg-brutal-cyan text-black font-bold py-3 border-3 border-black disabled:opacity-50 disabled:cursor-not-allowed hover:translate-x-1 hover:-translate-y-1 hover:shadow-[4px_4px_0_0_#000] transition-all cursor-pointer"
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
				class="font-bold text-black hover:underline ml-1 cursor-pointer"
			>
				{isSignUp ? 'Sign in' : 'Create one'}
			</button>
		</p>

		<button
			type="button"
			onclick={onClose}
			class="w-full mt-4 text-center font-bold text-black/60 hover:text-black cursor-pointer"
		>
			Cancel
		</button>
	</div>
</div>
