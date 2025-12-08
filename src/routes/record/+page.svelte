<script lang="ts">
	import { goto } from '$app/navigation';
	import { onDestroy, onMount, tick } from 'svelte';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';
	import {
		startSidecar,
		startRecording,
		stopRecording,
		cancelRecording,
		getRecordingStatus,
		getWebSocket,
		type SidecarWebSocket
	} from '$lib/services/sidecarService';
	import { getAuthToken, checkAuthStatus } from '$lib/services/authService';
	import { getSettings } from '$lib/services/settingsService';
	import type { WorkflowStep } from '$lib/types/workflow';

	const workflowState = getWorkflowState();

	let isRecording = $state(false);
	let isPaused = $state(false);
	let isSaving = $state(false);
	let steps = $state<WorkflowStep[]>([]);
	let error = $state<string | null>(null);
	let sessionId = $state<string | null>(null);
	let isStarting = $state(false);
	let loadingMessage = $state('Initializing...');
	let ws: SidecarWebSocket | null = null;

	// Settings validation
	let isCheckingSettings = $state(true);
	let settingsConfigured = $state(false);
	let settingsError = $state<string | null>(null);

	// Check if AI settings are properly configured
	onMount(async () => {
		try {
			const settings = await getSettings();
			const provider = settings.llm_config.default_provider;
			const model = settings.llm_config.default_model;

			if (!provider || !model) {
				settingsError = 'Please configure a default AI provider and model in Settings before recording.';
				settingsConfigured = false;
			} else if (provider === 'tasker-fast') {
				// Check if user has subscription for Tasker Fast
				const auth = await checkAuthStatus();
				if (!auth.has_subscription) {
					settingsError = 'Tasker Fast requires an active subscription. Subscribe or configure an API key in Settings.';
					settingsConfigured = false;
				} else {
					settingsConfigured = true;
				}
			} else {
				// Check if API key is configured for the provider
				const apiKeys = settings.llm_config.api_keys;
				const hasKey =
					(provider === 'gemini' && apiKeys.gemini) ||
					(provider === 'openai' && apiKeys.openai) ||
					(provider === 'anthropic' && apiKeys.anthropic);

				if (!hasKey) {
					settingsError = `No API key configured for ${provider}. Please add your API key in Settings.`;
					settingsConfigured = false;
				} else {
					settingsConfigured = true;
				}
			}
		} catch (e) {
			settingsError = 'Unable to load settings. Please check your configuration.';
			settingsConfigured = false;
		} finally {
			isCheckingSettings = false;
		}
	});

	// Handle step events from WebSocket
	function handleStepEvent(data: unknown) {
		const stepData = data as { step?: { id: string; order: number; name: string; action: unknown } };
		if (stepData.step) {
			const step: WorkflowStep = {
				id: stepData.step.id,
				order: stepData.step.order,
				name: stepData.step.name,
				action: stepData.step.action as WorkflowStep['action']
			};
			steps = [...steps, step];
		}
	}

	// Poll until recording is ready (status = 'recording')
	async function pollUntilReady(sid: string, timeoutMs: number = 30000): Promise<void> {
		const start = Date.now();
		while (Date.now() - start < timeoutMs) {
			try {
				const status = await getRecordingStatus(sid);
				if (status.status === 'recording') return;
				if (status.status === 'error') {
					throw new Error(status.error || 'Recording failed to start');
				}
			} catch {
				// Session might not exist yet, keep polling
			}
			await new Promise((r) => setTimeout(r, 200)); // Poll every 200ms
		}
		throw new Error('Recording startup timeout - browser may have failed to launch');
	}

	async function handleStartRecording() {
		error = null;
		isStarting = true;
		loadingMessage = 'Starting automation engine...';
		await tick(); // Force UI update NOW - prevents freeze

		try {
			// Start sidecar if not running
			await startSidecar();

			loadingMessage = 'Launching Chrome browser...';
			await tick();

			// Start recording via sidecar API (launches Chrome with blank tab)
			const response = await startRecording({
				headless: false,
				viewport_width: 1280,
				viewport_height: 720
			});

			sessionId = response.session_id;

			// If status is 'initializing', poll until ready
			if (response.status === 'initializing') {
				loadingMessage = 'Waiting for browser to be ready...';
				await tick();
				await pollUntilReady(sessionId, 30000);
			}

			isRecording = true;
			steps = [];

			// Connect WebSocket for real-time step events
			ws = getWebSocket();
			await ws.connect();
			ws.on('recording_step', handleStepEvent);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start recording';
			isRecording = false;
		} finally {
			isStarting = false;
		}
	}

	async function handlePauseResume() {
		// Pause/resume not yet implemented in sidecar
		isPaused = !isPaused;
	}

	async function handleStopRecording() {
		if (!sessionId) {
			error = 'No active recording session';
			return;
		}

		isSaving = true;

		try {
			// Get auth token if using Tasker Fast provider
			let authToken: string | undefined;
			try {
				const settings = await getSettings();
				if (settings.llm_config.default_provider === 'tasker-fast') {
					authToken = (await getAuthToken()) ?? undefined;
				}
			} catch {
				// If settings unavailable, continue without auth token
			}

			// Stop recording and generate task description via AI
			const response = await stopRecording(sessionId, authToken);

			// Disconnect WebSocket
			ws?.disconnect();
			ws = null;

			// Save workflow with AI-generated name and task description
			const workflow = await workflowState.createWorkflow({
				name: response.name,
				task_description: response.task_description,
				steps: [], // No steps - AI agent will execute from task description
				metadata: {
					recording_source: 'recorded'
				}
			});

			// Reset state
			isRecording = false;
			isPaused = false;
			isSaving = false;
			steps = [];
			sessionId = null;

			// Navigate to the workflow
			if (workflow) {
				goto(`/workflows/${workflow.id}`);
			} else {
				goto('/');
			}
		} catch (e) {
			isSaving = false;
			error = e instanceof Error ? e.message : 'Failed to save workflow';
		}
	}

	async function handleCancel() {
		if (sessionId) {
			try {
				await cancelRecording(sessionId);
			} catch (e) {
				console.error('Failed to cancel recording:', e);
			}
		}

		// Disconnect WebSocket
		ws?.disconnect();
		ws = null;

		isRecording = false;
		isPaused = false;
		steps = [];
		sessionId = null;
	}

	// Cleanup on component destroy
	onDestroy(() => {
		if (ws) {
			ws.disconnect();
		}
	});
</script>

<div class="record-page" class:recording={isRecording}>
	{#if error}
		<div class="error-banner">
			<span>{error}</span>
			<button onclick={() => (error = null)}>DISMISS</button>
		</div>
	{/if}

	{#if !isRecording}
		<div class="start-screen">
			<div class="header">
				<h1>Record Workflow</h1>
				<p>Record your browser actions to create an automation</p>
			</div>

			{#if isCheckingSettings}
				<div class="card">
					<div class="checking-settings">
						<span class="spinner"></span>
						<p>Checking configuration...</p>
					</div>
				</div>
			{:else if settingsError}
				<div class="card">
					<div class="settings-error">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="48" height="48">
							<circle cx="12" cy="12" r="10"/>
							<line x1="12" y1="8" x2="12" y2="12"/>
							<line x1="12" y1="16" x2="12.01" y2="16"/>
						</svg>
						<h3>CONFIGURATION REQUIRED</h3>
						<p>{settingsError}</p>
						<button onclick={() => goto('/settings')} class="settings-btn">
							GO TO SETTINGS
						</button>
					</div>
				</div>
			{:else}
				<div class="card">
					<div class="info-box">
						<h3>HOW IT WORKS</h3>
						<ol>
							<li><span class="num">1</span> Chrome opens with a blank tab</li>
							<li><span class="num">2</span> Navigate to any site and perform your actions</li>
							<li><span class="num">3</span> Click "Stop & Save" when done</li>
							<li><span class="num">4</span> Your workflow is saved automatically</li>
						</ol>
					</div>

					<button
						onclick={handleStartRecording}
						disabled={isStarting}
						class="start-btn"
					>
						{#if isStarting}
							<span class="spinner"></span>
							{loadingMessage.toUpperCase()}
						{:else}
							<svg viewBox="0 0 20 20" fill="currentColor" width="24" height="24">
								<circle cx="10" cy="10" r="6" />
							</svg>
							START RECORDING
					{/if}
				</button>
			</div>
			{/if}
		</div>
	{:else}
		<div class="recording-screen">
			<div class="controls-bar">
				<div class="status">
					{#if isPaused}
						<span class="status-badge paused">PAUSED</span>
					{:else}
						<span class="status-badge recording">
							<span class="pulse"></span>
							RECORDING
						</span>
					{/if}
					<span class="step-count">{steps.length} steps</span>
				</div>

				<div class="actions">
					<button onclick={handlePauseResume} class="btn secondary" disabled={isSaving}>
						{isPaused ? 'RESUME' : 'PAUSE'}
					</button>
					<button onclick={handleCancel} class="btn secondary" disabled={isSaving}>CANCEL</button>
					<button onclick={handleStopRecording} class="btn primary" disabled={isSaving}>
						{#if isSaving}
							<span class="spinner-small"></span>
							GENERATING...
						{:else}
							STOP & SAVE
						{/if}
					</button>
				</div>
			</div>

			{#if isSaving}
				<div class="saving-overlay">
					<div class="saving-content">
						<span class="spinner-large"></span>
						<h2>Generating Task Description</h2>
						<p>AI is analyzing your recording to create a detailed task description...</p>
					</div>
				</div>
			{/if}

			<div class="recording-content">
				<div class="status-message">
					<div class="chrome-icon">
						<svg viewBox="0 0 24 24" fill="currentColor" width="48" height="48">
							<circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"/>
							<circle cx="12" cy="12" r="4"/>
						</svg>
					</div>
					<h2>Chrome is running</h2>
					<p>Navigate to any site and perform the actions you want to automate. Your steps will appear below.</p>
				</div>

				<div class="steps-panel">
					<h3>RECORDED STEPS</h3>
					<div class="steps-list">
						{#each steps as step, i (step.id)}
							<div class="step-item">
								<span class="step-num">{i + 1}</span>
								<span class="step-name">{step.name}</span>
							</div>
						{/each}
						{#if steps.length === 0}
							<p class="empty-state">Your recorded steps will appear here</p>
						{/if}
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.record-page {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.record-page.recording {
		padding: 0;
	}

	.error-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		background: var(--brutal-magenta, #ff6b9d);
		border-bottom: 3px solid #000;
		font-weight: bold;
	}

	.error-banner button {
		background: none;
		border: none;
		text-decoration: underline;
		cursor: pointer;
		font-weight: bold;
	}

	/* Start Screen */
	.start-screen {
		max-width: 600px;
		margin: 0 auto;
		padding: 32px;
	}

	.header h1 {
		font-size: 2.5rem;
		font-weight: bold;
		margin: 0;
	}

	.header p {
		color: rgba(0, 0, 0, 0.6);
		margin: 8px 0 24px;
	}

	.card {
		background: white;
		border: 3px solid #000;
		padding: 24px;
		box-shadow: 6px 6px 0 0 #000;
	}

	.checking-settings {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		padding: 32px;
	}

	.checking-settings p {
		margin: 0;
		color: rgba(0, 0, 0, 0.6);
		font-weight: 500;
	}

	.settings-error {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: 12px;
		padding: 24px;
	}

	.settings-error svg {
		color: var(--brutal-magenta, #ff6b9d);
	}

	.settings-error h3 {
		margin: 0;
		font-size: 1.25rem;
		font-weight: bold;
	}

	.settings-error p {
		margin: 0;
		color: rgba(0, 0, 0, 0.7);
		max-width: 400px;
	}

	.settings-btn {
		margin-top: 8px;
		padding: 12px 24px;
		background: var(--brutal-cyan, #00d4ff);
		border: 3px solid #000;
		font-weight: bold;
		cursor: pointer;
		box-shadow: 4px 4px 0 0 #000;
		transition:
			transform 0.1s,
			box-shadow 0.1s;
	}

	.settings-btn:hover {
		transform: translate(2px, 2px);
		box-shadow: 2px 2px 0 0 #000;
	}

	.info-box {
		background: var(--brutal-cyan, #00d4ff);
		border: 3px solid #000;
		padding: 16px;
		margin-bottom: 24px;
		box-shadow: 4px 4px 0 0 #000;
	}

	.info-box h3 {
		font-weight: bold;
		margin: 0 0 12px;
	}

	.info-box ol {
		list-style: none;
		padding: 0;
		margin: 0;
	}

	.info-box li {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 8px;
	}

	.info-box .num {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		background: #000;
		color: #fff;
		font-size: 0.75rem;
		font-weight: bold;
		flex-shrink: 0;
	}

	.start-btn {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 12px;
		padding: 16px;
		background: var(--brutal-magenta, #ff6b9d);
		border: 3px solid #000;
		font-size: 1.25rem;
		font-weight: bold;
		cursor: pointer;
		box-shadow: 6px 6px 0 0 #000;
		transition:
			transform 0.1s,
			box-shadow 0.1s;
	}

	.start-btn:hover:not(:disabled) {
		transform: translate(2px, 2px);
		box-shadow: 4px 4px 0 0 #000;
	}

	.start-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 3px solid rgba(0, 0, 0, 0.3);
		border-top-color: #000;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.spinner-small {
		width: 16px;
		height: 16px;
		border: 2px solid rgba(0, 0, 0, 0.3);
		border-top-color: #000;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	.spinner-large {
		width: 48px;
		height: 48px;
		border: 4px solid rgba(255, 255, 255, 0.3);
		border-top-color: var(--brutal-lime, #c4ff4d);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	/* Saving Overlay */
	.saving-overlay {
		position: absolute;
		inset: 0;
		background: rgba(0, 0, 0, 0.9);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.saving-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		text-align: center;
		padding: 32px;
	}

	.saving-content h2 {
		margin: 0;
		color: white;
		font-size: 1.5rem;
	}

	.saving-content p {
		margin: 0;
		color: #888;
		font-size: 0.875rem;
		max-width: 300px;
	}

	/* Recording Screen */
	.recording-screen {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.controls-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		background: #1a1a1a;
		border-bottom: 3px solid #000;
	}

	.status {
		display: flex;
		align-items: center;
		gap: 16px;
	}

	.status-badge {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 12px;
		font-weight: bold;
		font-size: 0.875rem;
	}

	.status-badge.recording {
		background: #ef4444;
		color: white;
	}

	.status-badge.paused {
		background: #f59e0b;
		color: black;
	}

	.pulse {
		width: 8px;
		height: 8px;
		background: white;
		border-radius: 50%;
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}

	.step-count {
		color: #888;
		font-weight: 500;
	}

	.actions {
		display: flex;
		gap: 8px;
	}

	.btn {
		padding: 8px 16px;
		border: 2px solid #000;
		font-weight: bold;
		cursor: pointer;
		transition:
			transform 0.1s,
			box-shadow 0.1s;
	}

	.btn.secondary {
		background: #333;
		color: white;
	}

	.btn.primary {
		background: var(--brutal-lime, #c4ff4d);
		color: black;
		box-shadow: 3px 3px 0 0 #000;
	}

	.btn:hover:not(:disabled) {
		transform: translate(1px, 1px);
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.recording-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		background: #1a1a1a;
		overflow: hidden;
	}

	.status-message {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px;
		text-align: center;
		background: #222;
		border-bottom: 3px solid #333;
	}

	.chrome-icon {
		color: var(--brutal-lime, #c4ff4d);
		margin-bottom: 16px;
	}

	.status-message h2 {
		margin: 0 0 8px;
		font-size: 1.25rem;
		color: white;
	}

	.status-message p {
		margin: 0 0 8px;
		color: #888;
		font-size: 0.875rem;
	}

	.steps-panel {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.steps-panel h3 {
		padding: 12px 16px;
		margin: 0;
		font-size: 0.875rem;
		text-transform: uppercase;
		color: #888;
		border-bottom: 1px solid #333;
	}

	.steps-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
	}

	.step-item {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 12px;
		background: #2a2a2a;
		border-radius: 4px;
		margin-bottom: 4px;
		font-size: 0.875rem;
	}

	.step-num {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		background: #444;
		border-radius: 4px;
		font-size: 0.75rem;
		font-weight: bold;
		color: #888;
		flex-shrink: 0;
	}

	.step-name {
		color: #ccc;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.empty-state {
		color: #666;
		text-align: center;
		padding: 24px;
		font-size: 0.875rem;
	}
</style>
