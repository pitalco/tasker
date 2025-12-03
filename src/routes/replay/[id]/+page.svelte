<script lang="ts">
	import { page } from '$app/stores';
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';
	import {
		startReplay,
		stopReplay,
		getReplayStatus,
		startSidecar,
		getWebSocket,
		type ReplayStatus
	} from '$lib/services/sidecarService';
	import {
		getSettings,
		PROVIDERS,
		PROVIDER_MODELS,
		type ApiKeys
	} from '$lib/services/settingsService';
	import type { Workflow } from '$lib/types/workflow';

	const workflowState = getWorkflowState();
	const ws = getWebSocket();

	let workflow = $state<Workflow | null>(null);
	let isLoading = $state(true);
	let isRunning = $state(false);
	let isStarting = $state(false);
	let sessionId = $state<string | null>(null);
	let error = $state<string | null>(null);
	let currentStep = $state(0);
	let totalSteps = $state(0);
	let statusPolling = $state<ReturnType<typeof setInterval> | null>(null);
	let results = $state<Array<{ step_id: string; success: boolean; error?: string; tool_name?: string; tool_params?: string }>>([]);

	// Friendly display names for tools
	const toolDisplayNames: Record<string, string> = {
		'click_element': 'Clicked on element',
		'input_text': 'Typed text',
		'select_dropdown_option': 'Selected option',
		'go_to_url': 'Navigated to URL',
		'search_google': 'Searched Google',
		'scroll_down': 'Scrolled down',
		'scroll_up': 'Scrolled up',
		'go_back': 'Went back',
		'send_keys': 'Pressed keys',
		'execute_javascript': 'Ran script',
		'extract_page_content': 'Extracted content',
		'get_dropdown_options': 'Got dropdown options',
		'read_file': 'Read file',
		'write_file': 'Wrote file',
		'replace_in_file': 'Replaced in file',
		'done': 'Completed task'
	};

	function getToolDisplay(toolName?: string): string {
		if (!toolName) return 'Processing';
		return toolDisplayNames[toolName] || toolName;
	}

	// Settings - AI is ALWAYS used, recorded steps are hints
	let llmProvider = $state('google');
	let llmModel = $state('gemini-3-pro-preview');
	let iterations = $state(1);
	let headless = $state(false);
	let taskDescription = $state('');

	// API keys from settings
	let apiKeys = $state<ApiKeys>({});
	let hasAnyKeys = $state(false);

	const workflowId = $derived($page.params.id);

	// Get available providers (those with keys configured)
	const availableProviders = $derived(
		PROVIDERS.filter(p => (apiKeys[p.id as keyof ApiKeys] ?? '').length > 0)
	);

	// Get models for current provider
	const availableModels = $derived(PROVIDER_MODELS[llmProvider] || []);

	onMount(async () => {
		// Load workflow
		await workflowState.loadWorkflows();
		workflow = workflowState.workflows.find((w) => w.id === workflowId) || null;

		if (!workflow) {
			error = 'Workflow not found';
			isLoading = false;
			return;
		}

		totalSteps = workflow.steps.length;

		// Initialize task description from workflow (for text-only workflows)
		if (workflow.task_description) {
			taskDescription = workflow.task_description;
		}

		// Load settings to get API keys and defaults
		try {
			const settings = await getSettings();
			apiKeys = settings.llm_config.api_keys;
			llmProvider = settings.llm_config.default_provider;
			llmModel = settings.llm_config.default_model;

			// Check if any keys are configured
			hasAnyKeys = Object.values(apiKeys).some(key => key && key.length > 0);
		} catch {
			console.warn('Failed to load settings');
		}

		isLoading = false;
	});

	onDestroy(() => {
		// Clean up listeners when component unmounts
		ws.off('replay_step', handleStepResult);
		ws.off('replay_complete', handleComplete);
		ws.off('error', handleError);

		// Clear polling interval if active
		if (statusPolling) {
			clearInterval(statusPolling);
		}
	});

	$effect(() => {
		// Update model when provider changes (ensure valid model for provider)
		const models = PROVIDER_MODELS[llmProvider] || [];
		if (models.length > 0 && !models.find(m => m.id === llmModel)) {
			llmModel = models[0].id;
		}
	});

	// Get the API key for the current provider
	function getApiKeyForProvider(): string | undefined {
		return apiKeys[llmProvider as keyof ApiKeys];
	}

	async function handleStart() {
		if (!workflow) return;

		error = null;
		isStarting = true;

		try {
			await startSidecar();

			// Connect WebSocket
			try {
				await ws.connect();

				// Clear any stale listeners first (defensive)
				ws.off('replay_step', handleStepResult);
				ws.off('replay_complete', handleComplete);
				ws.off('error', handleError);

				// Add fresh listeners
				ws.on('replay_step', handleStepResult);
				ws.on('replay_complete', handleComplete);
				ws.on('error', handleError);
			} catch {
				console.warn('WebSocket failed, using polling');
			}

			const response = await startReplay({
				workflow,
				llm_provider: llmProvider,
				llm_model: llmModel,
				task_description: taskDescription || undefined,
				iterations,
				headless
			});

			sessionId = response.session_id;
			isRunning = true;
			isStarting = false;
			totalSteps = workflow.steps.length * iterations;

			// Start polling
			statusPolling = setInterval(pollStatus, 1000);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start replay';
			isStarting = false;
		}
	}

	function handleStepResult(data: unknown) {
		const stepData = data as { result: { step_id: string; success: boolean; error?: string; tool_name?: string; tool_params?: string } };
		if (stepData.result) {
			results = [...results, stepData.result];
			currentStep = results.length;
		}
	}

	function handleComplete(_data: unknown) {
		isRunning = false;
		if (statusPolling) {
			clearInterval(statusPolling);
			statusPolling = null;
		}
	}

	function handleError(data: unknown) {
		const errorData = data as { error: string };
		error = errorData.error;
		isRunning = false;
	}

	async function pollStatus() {
		if (!sessionId) return;

		try {
			const status: ReplayStatus = await getReplayStatus(sessionId);
			currentStep = status.current_step;

			if (status.status === 'completed' || status.status === 'error') {
				isRunning = false;
				if (statusPolling) {
					clearInterval(statusPolling);
					statusPolling = null;
				}
				if (status.error) {
					error = status.error;
				}
			}
		} catch {
			// Ignore polling errors
		}
	}

	async function handleStop() {
		if (!sessionId) return;

		if (statusPolling) {
			clearInterval(statusPolling);
			statusPolling = null;
		}

		ws.off('replay_step', handleStepResult);
		ws.off('replay_complete', handleComplete);
		ws.off('error', handleError);

		try {
			await stopReplay(sessionId);
		} catch {
			// Ignore errors
		}

		isRunning = false;
		sessionId = null;
	}

	const progress = $derived(totalSteps > 0 ? (currentStep / totalSteps) * 100 : 0);
	const successCount = $derived(results.filter((r) => r.success).length);
	const failureCount = $derived(results.filter((r) => !r.success).length);

	// Check if current provider has an API key
	const hasKeyForCurrentProvider = $derived(
		(apiKeys[llmProvider as keyof ApiKeys] ?? '').length > 0
	);
</script>

<div class="max-w-4xl mx-auto space-y-8">
	<!-- Header -->
	<div class="flex items-start justify-between">
		<div>
			<button onclick={() => goto('/')} class="text-sm font-bold text-black/60 hover:text-black mb-2 flex items-center gap-1 cursor-pointer">
				<svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
					<path d="M15 19l-7-7 7-7" />
				</svg>
				BACK TO WORKFLOWS
			</button>
			<h1 class="text-4xl font-bold text-black tracking-tight">Run Workflow</h1>
			{#if workflow}
				<p class="text-lg text-black/60 font-medium mt-1">{workflow.name}</p>
			{/if}
		</div>
	</div>

	{#if error}
		<div class="bg-brutal-magenta border-3 border-black p-4 flex items-center justify-between" style="box-shadow: 4px 4px 0 0 #000;">
			<span class="font-bold text-black">{error}</span>
			<button onclick={() => (error = null)} class="font-bold underline">DISMISS</button>
		</div>
	{/if}

	{#if isLoading}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else if !workflow}
		<div class="card-brutal p-12 text-center">
			<h3 class="text-2xl font-bold text-black mb-2">WORKFLOW NOT FOUND</h3>
			<p class="text-black/60 font-medium mb-6">The workflow you're looking for doesn't exist.</p>
			<a href="/" class="btn-brutal bg-brutal-cyan text-black">VIEW ALL WORKFLOWS</a>
		</div>
	{:else if !isRunning}
		<!-- No API Keys Warning -->
		{#if !hasAnyKeys && availableProviders.length === 0}
			<div class="bg-brutal-yellow border-3 border-black p-4" style="box-shadow: 4px 4px 0 0 #000;">
				<div class="flex items-start gap-3">
					<svg class="w-6 h-6 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
						<path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
					</svg>
					<div>
						<p class="font-bold text-black">No API keys configured</p>
						<p class="text-sm text-black/80 mt-1">
							AI agent requires an API key. Configure API keys in Settings.
						</p>
						<a href="/settings" class="inline-block mt-3 btn-brutal bg-white text-black text-sm py-2">
							GO TO SETTINGS
						</a>
					</div>
				</div>
			</div>
		{/if}

		<!-- Settings -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-cyan h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-6">
				<h2 class="text-xl font-bold text-black">AI AGENT SETTINGS</h2>
				<p class="text-sm text-black/60 font-medium -mt-4">
					AI uses recorded steps as hints, adapting to page changes
				</p>

				<!-- LLM Provider -->
				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">LLM Provider</label>
					<div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
						{#each availableProviders as provider}
							<button
								onclick={() => (llmProvider = provider.id)}
								class="p-3 border-3 border-black font-bold text-sm cursor-pointer {llmProvider === provider.id ? 'bg-brutal-lime' : 'bg-white'}"
								style="box-shadow: 2px 2px 0 0 #000;"
							>
								{provider.name.toUpperCase()}
							</button>
						{/each}
					</div>
					{#if availableProviders.length < PROVIDERS.length}
						<p class="text-xs text-black/60 font-medium mt-2">
							<a href="/settings" class="underline">Add more API keys</a> to use other providers
						</p>
					{/if}
				</div>

				<!-- Model -->
				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Model</label>
					<select bind:value={llmModel} class="input-brutal cursor-pointer">
						{#each availableModels as model}
							<option value={model.id}>{model.name}</option>
						{/each}
					</select>
				</div>

				<!-- API Key Status -->
				<div class="flex items-center gap-2 p-3 border-3 border-black {hasKeyForCurrentProvider ? 'bg-brutal-lime/30' : 'bg-brutal-magenta/30'}" style="box-shadow: 2px 2px 0 0 #000;">
					{#if hasKeyForCurrentProvider}
						<svg class="w-5 h-5 text-green-600" fill="currentColor" viewBox="0 0 20 20">
							<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
						</svg>
						<span class="font-bold text-sm">API key configured in Settings</span>
					{:else}
						<svg class="w-5 h-5 text-red-600" fill="currentColor" viewBox="0 0 20 20">
							<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
						</svg>
						<span class="font-bold text-sm">No API key - <a href="/settings" class="underline">configure in Settings</a></span>
					{/if}
				</div>

				<!-- Task Description -->
				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Custom Instructions (Optional)</label>
					<textarea
						bind:value={taskDescription}
						placeholder="Add any specific instructions for the AI..."
						class="input-brutal h-24 resize-none"
					></textarea>
				</div>

				<div class="grid grid-cols-1 sm:grid-cols-2 gap-6">
					<!-- Iterations -->
					<div>
						<label class="block text-sm font-bold text-black uppercase mb-2">Iterations</label>
						<input type="number" bind:value={iterations} min="1" max="100" class="input-brutal" />
						<p class="text-xs text-black/60 font-medium mt-1">Run the workflow multiple times</p>
					</div>

					<!-- Headless -->
					<div class="flex items-center justify-between sm:justify-start sm:gap-6">
						<div>
							<div class="font-bold text-black">Headless Mode</div>
							<div class="text-sm text-black/60 font-medium">Run without visible browser</div>
						</div>
						<button
							onclick={() => (headless = !headless)}
							class="w-14 h-8 border-3 border-black transition-all cursor-pointer {headless ? 'bg-brutal-lime' : 'bg-white'}"
							style="box-shadow: 2px 2px 0 0 #000;"
						>
							<div class="w-5 h-5 bg-black transition-transform duration-150 {headless ? 'translate-x-6' : 'translate-x-1'}"></div>
						</button>
					</div>
				</div>
			</div>
		</div>

		<!-- Start Button -->
		<button
			onclick={handleStart}
			disabled={isStarting || !hasKeyForCurrentProvider}
			class="w-full btn-brutal bg-brutal-lime text-black text-xl py-4 flex items-center justify-center gap-3 disabled:opacity-50 disabled:cursor-not-allowed"
		>
			{#if isStarting}
				<div class="w-6 h-6 border-3 border-black border-t-transparent animate-spin"></div>
				STARTING...
			{:else}
				<svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
					<path d="M8 5v14l11-7z" />
				</svg>
				RUN WORKFLOW
			{/if}
		</button>
	{:else}
		<!-- Running State -->
		<div class="card-brutal p-8 space-y-8">
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					<div class="w-4 h-4 bg-brutal-lime animate-pulse"></div>
					<span class="font-bold text-xl text-black">RUNNING</span>
				</div>
				<button onclick={handleStop} class="btn-brutal bg-brutal-magenta text-black">
					STOP
				</button>
			</div>

			<!-- Progress -->
			<div>
				<div class="flex justify-between mb-2">
					<span class="font-bold text-black">Progress</span>
				</div>
				<div class="h-6 border-3 border-black bg-white relative overflow-hidden" style="box-shadow: 2px 2px 0 0 #000;">
					<!-- Animated fill bar -->
					<div class="absolute inset-0 bg-brutal-lime animate-progress-pulse"></div>
					<!-- Moving stripes overlay -->
					<div class="absolute inset-0 opacity-30 animate-progress-stripes" style="background: repeating-linear-gradient(45deg, transparent, transparent 10px, #000 10px, #000 12px);"></div>
				</div>
			</div>

			<!-- Stats -->
			<div class="grid grid-cols-3 gap-4">
				<div class="bg-brutal-cyan border-3 border-black p-4 text-center" style="box-shadow: 2px 2px 0 0 #000;">
					<div class="text-3xl font-bold text-black">{currentStep}</div>
					<div class="text-xs font-bold text-black/60 uppercase">Current</div>
				</div>
				<div class="bg-brutal-lime border-3 border-black p-4 text-center" style="box-shadow: 2px 2px 0 0 #000;">
					<div class="text-3xl font-bold text-black">{successCount}</div>
					<div class="text-xs font-bold text-black/60 uppercase">Success</div>
				</div>
				<div class="bg-brutal-magenta border-3 border-black p-4 text-center" style="box-shadow: 2px 2px 0 0 #000;">
					<div class="text-3xl font-bold text-black">{failureCount}</div>
					<div class="text-xs font-bold text-black/60 uppercase">Failed</div>
				</div>
			</div>

			<!-- Results Log -->
			{#if results.length > 0}
				<div class="border-3 border-black p-4 max-h-64 overflow-y-auto" style="box-shadow: 2px 2px 0 0 #000;">
					<div class="text-sm font-bold text-black/60 uppercase mb-2">Results Log</div>
					{#each results as result, i}
						<div class="flex items-center gap-2 py-1 text-sm font-medium">
							<span class="w-5 h-5 flex items-center justify-center text-xs font-bold {result.success ? 'bg-brutal-lime' : 'bg-brutal-magenta'}">
								{result.success ? '✓' : '✗'}
							</span>
							<span>Step {i + 1}: {getToolDisplay(result.tool_name)}</span>
							{#if result.error}
								<span class="text-black/60">- {result.error}</span>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>
