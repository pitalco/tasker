<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getSettings,
		updateSettings,
		getAvailableModels,
		PROVIDERS,
		PROVIDER_MODELS,
		type ApiKeys
	} from '$lib/services/settingsService';
	import { createAuthStore, getAuthStore } from '$lib/stores/auth.svelte';
	import LoginModal from '$lib/components/LoginModal.svelte';

	// Create auth store for this component tree
	const auth = createAuthStore();

	let isLoading = $state(true);
	let isSaving = $state(false);
	let error = $state<string | null>(null);
	let successMessage = $state<string | null>(null);
	let showLoginModal = $state(false);

	// API keys for each provider
	let apiKeys = $state<ApiKeys>({
		gemini: '',
		openai: '',
		anthropic: ''
	});

	// Default model selection
	let defaultProvider = $state('gemini');
	let defaultModel = $state('gemini-2.5-flash');

	// Default max steps
	let defaultMaxSteps = $state(50);

	// Providers that require API keys (not subscription)
	const apiKeyProviders = $derived(PROVIDERS.filter((p) => !p.requiresSubscription));

	onMount(async () => {
		try {
			const settings = await getSettings();
			apiKeys = {
				gemini: settings.llm_config.api_keys.gemini || '',
				openai: settings.llm_config.api_keys.openai || '',
				anthropic: settings.llm_config.api_keys.anthropic || ''
			};
			defaultProvider = settings.llm_config.default_provider;
			defaultModel = settings.llm_config.default_model;
			defaultMaxSteps = settings.default_max_steps || 50;
		} catch (e) {
			console.warn('Failed to load settings, using defaults');
		} finally {
			isLoading = false;
		}
	});

	// Get available models based on configured API keys AND subscription status
	const availableModels = $derived(getAvailableModels(apiKeys, auth.hasSubscription));

	// When default model changes, update provider
	function handleModelChange(e: Event) {
		const select = e.target as HTMLSelectElement;
		const [provider, model] = select.value.split('|');
		defaultProvider = provider;
		defaultModel = model;
	}

	// Check if a provider has a key configured
	function hasKey(providerId: string): boolean {
		return (apiKeys[providerId as keyof ApiKeys] ?? '').length > 0;
	}

	async function saveSettings() {
		isSaving = true;
		error = null;
		successMessage = null;

		try {
			// Only send non-empty keys
			const keysToSave: ApiKeys = {};
			if (apiKeys.gemini) keysToSave.gemini = apiKeys.gemini;
			if (apiKeys.openai) keysToSave.openai = apiKeys.openai;
			if (apiKeys.anthropic) keysToSave.anthropic = apiKeys.anthropic;

			await updateSettings({
				api_keys: keysToSave,
				default_provider: defaultProvider,
				default_model: defaultModel,
				default_max_steps: defaultMaxSteps
			});
			successMessage = 'Settings saved successfully!';
			setTimeout(() => (successMessage = null), 3000);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save settings';
		} finally {
			isSaving = false;
		}
	}
</script>

<div class="max-w-2xl mx-auto space-y-8">
	<div>
		<h1 class="text-4xl font-bold text-black tracking-tight">Settings</h1>
		<p class="text-lg text-black/60 font-medium mt-1">Configure your LLM providers and API keys</p>
	</div>

	{#if error}
		<div
			class="bg-brutal-magenta border-3 border-black p-4 flex items-center justify-between"
			style="box-shadow: 4px 4px 0 0 #000;"
		>
			<span class="font-bold text-black">{error}</span>
			<button onclick={() => (error = null)} class="font-bold underline">DISMISS</button>
		</div>
	{/if}

	{#if successMessage}
		<div
			class="bg-brutal-lime border-3 border-black p-4 flex items-center gap-3"
			style="box-shadow: 4px 4px 0 0 #000;"
		>
			<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
				<path d="M5 13l4 4L19 7" />
			</svg>
			<span class="font-bold text-black">{successMessage}</span>
		</div>
	{/if}

	{#if isLoading}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else}
		<!-- API Keys Section -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-cyan h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-6">
				<div>
					<h2 class="text-xl font-bold text-black">API KEYS</h2>
					<p class="text-sm text-black/60 font-medium mt-1">
						Enter API keys for the providers you want to use. Keys are stored locally.
					</p>
				</div>

				<div class="space-y-4">
					{#each apiKeyProviders as provider}
						<div
							class="border-3 border-black p-4 {hasKey(provider.id)
								? 'bg-brutal-lime/20'
								: 'bg-white'}"
							style="box-shadow: 3px 3px 0 0 #000;"
						>
							<div class="flex items-center justify-between mb-2">
								<div class="flex items-center gap-2">
									<span class="font-bold text-black">{provider.name}</span>
									{#if hasKey(provider.id)}
										<svg
											class="w-5 h-5 text-green-600"
											fill="currentColor"
											viewBox="0 0 20 20"
										>
											<path
												fill-rule="evenodd"
												d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
												clip-rule="evenodd"
											/>
										</svg>
									{/if}
								</div>
								<span class="text-xs font-medium text-black/60">
									{PROVIDER_MODELS[provider.id]?.length || 0} models
								</span>
							</div>

							<input
								type="password"
								bind:value={apiKeys[provider.id as keyof ApiKeys]}
								placeholder="Enter your {provider.name} API key"
								class="input-brutal text-sm"
							/>
						</div>
					{/each}
				</div>
			</div>
		</div>

		<!-- Account & Subscription Section -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-yellow h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-4">
				<div>
					<h2 class="text-xl font-bold text-black">ACCOUNT & SUBSCRIPTION</h2>
					<p class="text-sm text-black/60 font-medium mt-1">
						Sign in to access Tasker Fast cloud models without API keys
					</p>
				</div>

				{#if auth.isLoading}
					<div class="flex items-center gap-2">
						<div class="w-4 h-4 border-2 border-black border-t-transparent animate-spin"></div>
						<span class="font-medium">Loading...</span>
					</div>
				{:else if auth.isAuthenticated}
					<div
						class="border-3 border-black p-4 bg-brutal-lime/20"
						style="box-shadow: 3px 3px 0 0 #000;"
					>
						<div class="flex items-center justify-between">
							<div>
								<p class="font-bold text-black">{auth.email}</p>
								<p class="text-sm text-black/60">
									{auth.hasSubscription ? 'Pro Subscriber' : 'Free Account'}
								</p>
							</div>
							<button
								onclick={() => auth.logout()}
								class="btn-brutal bg-white text-black text-sm"
							>
								Sign Out
							</button>
						</div>
					</div>

					{#if auth.hasSubscription}
						<div class="flex gap-3">
							<button
								onclick={() => auth.openCustomerPortal()}
								class="btn-brutal bg-white text-black text-sm flex-1"
							>
								Manage Subscription
							</button>
						</div>
						<div class="bg-brutal-cyan/20 border-3 border-black p-3">
							<p class="font-bold text-black text-sm">Tasker Fast Active</p>
							<p class="text-xs text-black/70">
								You have access to Tasker Fast vision model - no API key needed!
							</p>
						</div>
					{:else}
						<div class="bg-brutal-purple/20 border-3 border-black p-4">
							<h3 class="font-bold text-black mb-2">Upgrade to Pro</h3>
							<p class="text-sm text-black/70 mb-3">
								Get access to Tasker Fast vision model. No API key needed!
							</p>
							<p class="font-black text-2xl text-black mb-3">$10/month</p>
							<button
								onclick={() => auth.openCheckout()}
								class="btn-brutal bg-brutal-cyan text-black w-full"
							>
								SUBSCRIBE NOW
							</button>
						</div>
					{/if}
				{:else}
					<div class="border-3 border-black p-4 bg-white" style="box-shadow: 3px 3px 0 0 #000;">
						<p class="text-black/70 mb-3">
							Sign in to access Tasker Fast cloud model without managing API keys.
						</p>
						<button
							onclick={() => (showLoginModal = true)}
							class="btn-brutal bg-brutal-cyan text-black"
						>
							SIGN IN
						</button>
					</div>
				{/if}
			</div>
		</div>

		<!-- Default Model Section -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-purple h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-4">
				<div>
					<h2 class="text-xl font-bold text-black">DEFAULT MODEL</h2>
					<p class="text-sm text-black/60 font-medium mt-1">
						Choose the default model for running workflows. Only providers with API keys are shown.
					</p>
				</div>

				{#if availableModels.length > 0}
					<select
						class="input-brutal"
						value="{defaultProvider}|{defaultModel}"
						onchange={handleModelChange}
					>
						{#each PROVIDERS as provider}
							{@const providerModels = availableModels.filter((m) => m.provider === provider.id)}
							{#if providerModels.length > 0}
								<optgroup label={provider.name}>
									{#each providerModels as model}
										<option value="{model.provider}|{model.model}">
											{model.name}
										</option>
									{/each}
								</optgroup>
							{/if}
						{/each}
					</select>
				{:else}
					<div
						class="bg-brutal-yellow border-3 border-black p-4"
						style="box-shadow: 3px 3px 0 0 #000;"
					>
						<p class="font-bold text-black">No API keys configured</p>
						<p class="text-sm text-black/80 mt-1">
							Add at least one API key above to enable model selection.
						</p>
					</div>
				{/if}
			</div>
		</div>

		<!-- Execution Settings -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-orange h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-4">
				<div>
					<h2 class="text-xl font-bold text-black">EXECUTION</h2>
					<p class="text-sm text-black/60 font-medium mt-1">
						Configure default execution behavior for workflows
					</p>
				</div>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Default Max Steps</label>
					<input
						type="number"
						bind:value={defaultMaxSteps}
						class="input-brutal w-32"
						min="1"
						max="500"
					/>
					<p class="text-xs text-black/50 mt-1">
						Maximum steps before a run stops (default: 50). Can be overridden per-workflow.
					</p>
				</div>
			</div>
		</div>

		<!-- Danger Zone -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-magenta h-2 border-b-3 border-black"></div>
			<div class="p-6">
				<h2 class="text-xl font-bold text-black">DANGER ZONE</h2>
				<p class="text-sm text-black/60 font-medium mt-1 mb-4">
					Destructive actions that cannot be undone
				</p>

				<div class="flex flex-wrap gap-3">
					<button class="btn-brutal bg-white text-black text-sm"> Clear Local Data </button>
					<button class="btn-brutal bg-brutal-magenta text-black text-sm">
						Delete All Workflows
					</button>
				</div>
			</div>
		</div>

		<button
			onclick={saveSettings}
			disabled={isSaving}
			class="w-full btn-brutal bg-brutal-lime text-black text-xl py-4 disabled:opacity-50"
		>
			{isSaving ? 'SAVING...' : 'SAVE SETTINGS'}
		</button>
	{/if}
</div>

<!-- Login Modal -->
{#if showLoginModal}
	<LoginModal onClose={() => (showLoginModal = false)} />
{/if}
