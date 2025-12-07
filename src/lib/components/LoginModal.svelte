<script lang="ts">
	import { getAuthStore } from '$lib/stores/auth.svelte';

	let { onClose }: { onClose: () => void } = $props();

	const auth = getAuthStore();

	let email = $state('');
	let emailSent = $state(false);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!email.trim()) return;

		const success = await auth.sendMagicLink(email.trim());
		if (success) {
			emailSent = true;
		}
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
	onclick={handleBackdropClick}
>
	<div
		class="bg-white border-4 border-black p-8 max-w-md w-full mx-4"
		style="box-shadow: 8px 8px 0 0 #000;"
	>
		<h2 class="text-2xl font-black text-black mb-2">SIGN IN</h2>
		<p class="text-black/60 font-medium mb-6">Sign in to access Tasker Fast cloud models</p>

		{#if auth.error}
			<div class="bg-red-100 border-3 border-black p-3 mb-4">
				<span class="font-bold text-black text-sm">{auth.error}</span>
			</div>
		{/if}

		{#if emailSent}
			<div
				class="bg-brutal-lime border-3 border-black p-4 text-center"
				style="box-shadow: 4px 4px 0 0 #000;"
			>
				<svg
					class="w-12 h-12 mx-auto mb-3"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					viewBox="0 0 24 24"
				>
					<path
						d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
					/>
				</svg>
				<p class="font-bold text-black text-lg">Check your email!</p>
				<p class="text-black/70 mt-2">
					We sent a magic link to <strong>{email}</strong>
				</p>
				<p class="text-sm text-black/50 mt-4">
					Click the link in the email to sign in. The link will open Tasker automatically.
				</p>
			</div>

			<button
				type="button"
				onclick={() => {
					emailSent = false;
					email = '';
				}}
				class="w-full mt-4 text-center font-bold text-black/60 hover:text-black"
			>
				Try a different email
			</button>
		{:else}
			<form onsubmit={handleSubmit}>
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

				<button
					type="submit"
					disabled={auth.isSigningIn || !email.trim()}
					class="w-full bg-brutal-cyan text-black font-bold py-3 border-3 border-black disabled:opacity-50 disabled:cursor-not-allowed hover:translate-x-1 hover:-translate-y-1 hover:shadow-[4px_4px_0_0_#000] transition-all"
				>
					{auth.isSigningIn ? 'SENDING...' : 'SEND MAGIC LINK'}
				</button>
			</form>

			<p class="text-xs text-black/50 mt-4 text-center">
				No password needed! We'll email you a secure link.
			</p>
		{/if}

		<button
			type="button"
			onclick={onClose}
			class="w-full mt-4 text-center font-bold text-black/60 hover:text-black"
		>
			Cancel
		</button>
	</div>
</div>
