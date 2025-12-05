<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { getRunsState } from '$lib/stores/runs.svelte';
	import {
		formatRunStatus,
		getStatusColorClass,
		formatRelativeTime,
		formatDuration
	} from '$lib/services/runsService';
	import { getWebSocket, startSidecar } from '$lib/services/sidecarService';
	import { listFilesForRun, deleteFile as deleteFileApi } from '$lib/services/filesService';
	import { marked } from 'marked';
	import type { RunStep } from '$lib/types/run';
	import type { TaskerFile } from '$lib/types/file';
	import FileList from '$lib/components/files/FileList.svelte';

	const runsState = getRunsState();
	const ws = getWebSocket();

	// Format step into human-readable display
	function formatStepDisplay(step: RunStep): string {
		const params = step.params || {};

		switch (step.tool_name) {
			case 'search_google':
				return `Searched Google(query: "${params.query || ''}")`;
			case 'go_to_url':
				return `Navigated to URL(${params.url || ''})`;
			case 'click_element':
				return `Clicked Element(index: ${params.index ?? ''})`;
			case 'input_text':
				const text = String(params.text || '');
				const displayText = text.length > 30 ? text.slice(0, 27) + '...' : text;
				return `Typed Text(index: ${params.index ?? ''}, text: "${displayText}")`;
			case 'select_dropdown_option':
				return `Selected Option(index: ${params.index ?? ''}, option: "${params.option || ''}")`;
			case 'scroll_down':
				return `Scrolled Down(${params.amount ? `${params.amount}px` : 'default'})`;
			case 'scroll_up':
				return `Scrolled Up(${params.amount ? `${params.amount}px` : 'default'})`;
			case 'go_back':
				return 'Went Back';
			case 'send_keys':
				return `Pressed Keys(${params.keys || ''})`;
			case 'execute_javascript':
				const script = String(params.script || '');
				const displayScript = script.length > 40 ? script.slice(0, 37) + '...' : script;
				return `Ran Script(${displayScript})`;
			case 'extract_page_content':
				return 'Extracted Page Content';
			case 'get_dropdown_options':
				return `Got Dropdown Options(index: ${params.index ?? ''})`;
			case 'done':
				return 'Completed Task';
			default:
				// Fallback: convert snake_case to Title Case
				const readable = step.tool_name.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
				return readable;
		}
	}

	let activeTab = $state<'steps' | 'result' | 'files'>('result');
	let isLive = $state(false);
	let runFiles = $state<TaskerFile[]>([]);
	let filesLoading = $state(false);

	const runId = $derived($page.params.id);

	// WebSocket event handlers
	function handleStepUpdate(data: unknown) {
		const stepData = data as { session_id: string };
		if (stepData.session_id === runId) {
			// Reload steps from DB since WebSocket sends simplified StepResult, not full RunStep
			runsState.loadRun(runId);
		}
	}

	function handleComplete(data: unknown) {
		const completeData = data as { session_id: string };
		if (completeData.session_id === runId) {
			// Auto-switch to result tab and refresh to get final result
			activeTab = 'result';
			runsState.loadRun(runId);
			isLive = false;
		}
	}

	onMount(async () => {
		// Load run data from DB
		if (runId) {
			await runsState.loadRun(runId);
		}

		// Connect to WebSocket for real-time updates
		try {
			await startSidecar();
			await ws.connect();

			// Add event listeners
			ws.on('replay_step', handleStepUpdate);
			ws.on('replay_complete', handleComplete);

			// Check if run is active
			if (runsState.currentRun?.status === 'running' || runsState.currentRun?.status === 'pending') {
				isLive = true;
			}
		} catch {
			console.warn('WebSocket connection failed, viewing historical data only');
		}
	});

	// Track live status based on run state
	$effect(() => {
		if (runsState.currentRun?.status === 'running' || runsState.currentRun?.status === 'pending') {
			isLive = true;
		} else {
			isLive = false;
		}
	});

	onDestroy(() => {
		// Clean up WebSocket listeners
		ws.off('replay_step', handleStepUpdate);
		ws.off('replay_complete', handleComplete);
		runsState.clearCurrent();
	});

	async function handleCancel() {
		if (runId) {
			await runsState.cancelRun(runId);
		}
	}

	function goBack() {
		goto('/runs');
	}

	async function loadFiles() {
		if (!runId) return;
		filesLoading = true;
		try {
			const response = await listFilesForRun(runId);
			runFiles = response.files;
		} catch (e) {
			console.error('Failed to load files:', e);
		} finally {
			filesLoading = false;
		}
	}

	async function handleDeleteFile(file: TaskerFile) {
		if (!confirm(`Are you sure you want to delete "${file.file_name}"?`)) {
			return;
		}
		try {
			await deleteFileApi(file.id);
			runFiles = runFiles.filter((f) => f.id !== file.id);
		} catch (e) {
			console.error('Failed to delete file:', e);
			alert('Failed to delete file');
		}
	}

	function switchToFilesTab() {
		activeTab = 'files';
		if (runFiles.length === 0 && !filesLoading) {
			loadFiles();
		}
	}
</script>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<button
			onclick={goBack}
			aria-label="Go back to runs list"
			class="p-2 bg-white border-3 border-black hover:-translate-y-0.5 transition-transform cursor-pointer"
			style="box-shadow: 2px 2px 0 0 #000;"
		>
			<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
				<path d="M15 19l-7-7 7-7" />
			</svg>
		</button>
		<div class="flex-1">
			<h1 class="text-3xl font-bold text-black tracking-tight">
				{runsState.currentRun?.workflow_name || runsState.currentRun?.task_description || 'Run Details'}
			</h1>
			{#if runsState.currentRun}
				<p class="text-sm text-black/60 font-medium mt-1">
					ID: {runsState.currentRun.id}
				</p>
			{/if}
		</div>
		{#if isLive}
			<div class="flex items-center gap-2 px-3 py-2 bg-brutal-cyan border-2 border-black">
				<div class="w-3 h-3 bg-black rounded-full animate-pulse"></div>
				<span class="font-bold text-sm">LIVE</span>
			</div>
		{/if}
	</div>

	<!-- Loading -->
	{#if runsState.isLoading && !runsState.currentRun}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else if runsState.currentRun}
		<!-- Status and metadata -->
		<div class="card-brutal bg-white p-6">
			<div class="grid grid-cols-2 md:grid-cols-3 gap-6">
				<div>
					<span class="text-xs font-bold text-black/60 uppercase">Status</span>
					<div class="mt-1">
						<span class="px-3 py-1 text-sm font-bold border-2 border-black {getStatusColorClass(runsState.currentRun.status)}">
							{formatRunStatus(runsState.currentRun.status)}
						</span>
					</div>
				</div>
				<div>
					<span class="text-xs font-bold text-black/60 uppercase">Started</span>
					<p class="font-bold mt-1">
						{runsState.currentRun.started_at ? formatRelativeTime(runsState.currentRun.started_at) : '-'}
					</p>
				</div>
				<div>
					<span class="text-xs font-bold text-black/60 uppercase">Completed</span>
					<p class="font-bold mt-1">
						{runsState.currentRun.completed_at ? formatRelativeTime(runsState.currentRun.completed_at) : '-'}
					</p>
				</div>
			</div>

			{#if runsState.currentRun.task_description}
				<div class="mt-6 pt-6 border-t-2 border-black">
					<span class="text-xs font-bold text-black/60 uppercase">Task Description</span>
					<p class="font-medium mt-1">{runsState.currentRun.task_description}</p>
				</div>
			{/if}

			{#if runsState.currentRun.error}
				<div class="mt-6 p-4 bg-brutal-magenta/20 border-2 border-brutal-magenta">
					<span class="text-xs font-bold text-brutal-magenta uppercase">Error</span>
					<p class="font-medium mt-1 text-brutal-magenta">{runsState.currentRun.error}</p>
				</div>
			{/if}

			{#if runsState.currentRun.status === 'running' || runsState.currentRun.status === 'pending'}
				<div class="mt-6 flex justify-end">
					<button
						onclick={handleCancel}
						class="btn-brutal bg-brutal-orange text-black"
					>
						CANCEL RUN
					</button>
				</div>
			{/if}
		</div>

		<!-- Progress bar when running -->
		{#if isLive}
			<div class="card-brutal bg-white p-4">
				<div class="flex justify-between mb-2">
					<span class="font-bold text-black">Progress</span>
					<span class="font-bold text-black/60">{runsState.currentSteps.length} steps</span>
				</div>
				<div class="h-4 border-3 border-black bg-white relative overflow-hidden" style="box-shadow: 2px 2px 0 0 #000;">
					<!-- Animated fill bar -->
					<div class="absolute inset-0 bg-brutal-lime animate-progress-pulse"></div>
					<!-- Moving stripes overlay -->
					<div class="absolute inset-0 opacity-30 animate-progress-stripes" style="background: repeating-linear-gradient(45deg, transparent, transparent 10px, #000 10px, #000 12px);"></div>
				</div>
			</div>
		{/if}

		<!-- Tabs -->
		<div class="flex gap-2 border-b-3 border-black">
			{#if runsState.currentRun?.result}
				<button
					onclick={() => (activeTab = 'result')}
					class="px-6 py-3 font-bold border-3 border-black border-b-0 cursor-pointer {activeTab === 'result'
						? 'bg-black text-white'
						: 'bg-brutal-green text-black hover:bg-brutal-green/80'}"
				>
					RESULT
				</button>
			{/if}
			<button
				onclick={() => (activeTab = 'steps')}
				class="px-6 py-3 font-bold border-3 border-black border-b-0 cursor-pointer {activeTab === 'steps'
					? 'bg-black text-white'
					: 'bg-white text-black hover:bg-gray-100'}"
			>
				STEPS ({runsState.currentSteps.length})
			</button>
			<button
				onclick={switchToFilesTab}
				class="px-6 py-3 font-bold border-3 border-black border-b-0 cursor-pointer {activeTab === 'files'
					? 'bg-black text-white'
					: 'bg-brutal-purple text-black hover:bg-brutal-purple/80'}"
			>
				FILES
			</button>
		</div>

		<!-- Steps tab -->
		{#if activeTab === 'steps'}
			<div class="space-y-3">
				{#if runsState.currentSteps.length === 0}
					<div class="card-brutal bg-white p-8 text-center">
						<p class="text-black/60 font-medium">No steps recorded yet</p>
					</div>
				{:else}
					{#each runsState.currentSteps as step (step.id)}
						<div class="card-brutal bg-white p-4">
							<div class="flex items-start justify-between gap-4">
								<div class="flex-1">
									<div class="flex items-center gap-3 mb-2">
										<span class="w-8 h-8 flex items-center justify-center bg-black text-white font-bold text-sm">
											{step.step_number}
										</span>
										<span class="font-bold text-lg">{formatStepDisplay(step)}</span>
										{#if step.success}
											<svg class="w-5 h-5 text-green-600" fill="currentColor" viewBox="0 0 20 20">
												<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
											</svg>
										{:else}
											<svg class="w-5 h-5 text-red-600" fill="currentColor" viewBox="0 0 20 20">
												<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
											</svg>
										{/if}
									</div>
									<div class="pl-11">
										{#if step.error}
											<p class="text-sm text-brutal-magenta font-medium">{step.error}</p>
										{/if}
									</div>
								</div>
								<div class="text-right">
									<span class="text-sm font-medium text-black/60">
										{formatDuration(step.duration_ms)}
									</span>
								</div>
							</div>
							{#if step.screenshot}
								<div class="mt-4 pl-11">
									<img
										src="data:image/png;base64,{step.screenshot}"
										alt="Step screenshot"
										class="max-w-full h-auto border-2 border-black"
										style="max-height: 300px; object-fit: contain;"
									/>
								</div>
							{/if}
						</div>
					{/each}
				{/if}
			</div>
		{/if}

		<!-- Result tab -->
		{#if activeTab === 'result' && runsState.currentRun?.result}
			<div class="card-brutal bg-white p-6">
				<div class="prose prose-lg max-w-none">
					{@html marked(runsState.currentRun.result)}
				</div>
			</div>
		{/if}

		<!-- Files tab -->
		{#if activeTab === 'files'}
			<FileList files={runFiles} loading={filesLoading} showDelete={true} onDelete={handleDeleteFile} />
		{/if}
	{:else}
		<div class="card-brutal bg-white p-8 text-center">
			<p class="font-bold text-black/60">Run not found</p>
			<button onclick={goBack} class="btn-brutal bg-brutal-purple text-black mt-4">
				BACK TO RUNS
			</button>
		</div>
	{/if}
</div>
