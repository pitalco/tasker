<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';
	import { importTaskfileFromFile } from '$lib/services/taskfileService';
	import WorkflowCard from '$lib/components/WorkflowCard.svelte';

	const workflowState = getWorkflowState();

	let searchQuery = $state('');
	let sortBy = $state<'name' | 'updated'>('updated');
	let isImporting = $state(false);
	let importError = $state<string | null>(null);
	let fileInput = $state<HTMLInputElement | null>(null);

	const filteredWorkflows = $derived(() => {
		let workflows = [...workflowState.workflows];

		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			workflows = workflows.filter(w =>
				w.name.toLowerCase().includes(query) ||
				(w.description?.toLowerCase().includes(query))
			);
		}

		if (sortBy === 'name') {
			workflows.sort((a, b) => a.name.localeCompare(b.name));
		} else {
			workflows.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
		}

		return workflows;
	});

	onMount(() => {
		workflowState.loadWorkflows();
	});

	function openImportDialog() {
		importError = null;
		fileInput?.click();
	}

	async function handleFileSelect(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		isImporting = true;
		importError = null;

		try {
			const result = await importTaskfileFromFile(file);
			await workflowState.loadWorkflows();
			// Navigate to the imported workflow
			goto(`/workflows/${result.workflow_id}`);
		} catch (e) {
			importError = e instanceof Error ? e.message : 'Failed to import taskfile';
		} finally {
			isImporting = false;
			// Clear the input so the same file can be imported again
			input.value = '';
		}
	}
</script>

<div class="space-y-8">
	<!-- Header -->
	<div class="flex items-end justify-between">
		<div>
			<h1 class="text-4xl font-bold text-black tracking-tight">Workflows</h1>
			<p class="text-lg text-black/60 font-medium mt-1">All your saved browser automation workflows</p>
		</div>
		<div class="flex gap-3">
			<!-- Hidden file input -->
			<input
				bind:this={fileInput}
				type="file"
				accept=".yaml,.yml"
				class="hidden"
				onchange={handleFileSelect}
			/>
			<button
				onclick={openImportDialog}
				disabled={isImporting}
				class="btn-brutal bg-brutal-purple text-black flex items-center gap-2 disabled:opacity-50"
			>
				<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
					<path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
				</svg>
				{isImporting ? 'IMPORTING...' : 'IMPORT'}
			</button>
			<a
				href="/record"
				class="btn-brutal bg-brutal-magenta text-black flex items-center gap-2"
			>
				<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
					<circle cx="10" cy="10" r="6" />
				</svg>
				RECORD NEW
			</a>
		</div>
	</div>

	<!-- Search and filters -->
	<div class="flex flex-col sm:flex-row gap-4">
		<div class="flex-1">
			<div class="relative">
				<svg class="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-black/40" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
					<path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search workflows..."
					class="input-brutal pl-12"
				/>
			</div>
		</div>
		<div class="flex gap-2">
			<button
				onclick={() => (sortBy = 'updated')}
				class="px-4 py-3 border-3 border-black font-bold transition-all {sortBy === 'updated'
					? 'bg-black text-white'
					: 'bg-white text-black hover:-translate-y-0.5'}"
				style="box-shadow: {sortBy === 'updated' ? '0 0 0 0 #000' : '3px 3px 0 0 #000'};"
			>
				RECENT
			</button>
			<button
				onclick={() => (sortBy = 'name')}
				class="px-4 py-3 border-3 border-black font-bold transition-all {sortBy === 'name'
					? 'bg-black text-white'
					: 'bg-white text-black hover:-translate-y-0.5'}"
				style="box-shadow: {sortBy === 'name' ? '0 0 0 0 #000' : '3px 3px 0 0 #000'};"
			>
				A-Z
			</button>
		</div>
	</div>

	<!-- Error messages -->
	{#if workflowState.error}
		<div class="card-brutal bg-brutal-magenta p-4 flex items-center justify-between">
			<span class="font-bold">{workflowState.error}</span>
			<button onclick={() => workflowState.clearError()} class="font-bold underline">
				DISMISS
			</button>
		</div>
	{/if}

	{#if importError}
		<div class="card-brutal bg-brutal-magenta p-4 flex items-center justify-between">
			<span class="font-bold">{importError}</span>
			<button onclick={() => (importError = null)} class="font-bold underline">
				DISMISS
			</button>
		</div>
	{/if}

	<!-- Workflow count -->
	{#if !workflowState.isLoading && filteredWorkflows().length > 0}
		<div class="flex items-center gap-3">
			<span class="px-3 py-1 bg-black text-white font-bold text-sm">
				{filteredWorkflows().length} WORKFLOW{filteredWorkflows().length !== 1 ? 'S' : ''}
			</span>
			{#if searchQuery.trim()}
				<span class="text-sm font-medium text-black/60">
					matching "{searchQuery}"
				</span>
			{/if}
		</div>
	{/if}

	<!-- Workflows grid -->
	{#if workflowState.isLoading}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else if filteredWorkflows().length === 0}
		<div class="card-brutal p-12 text-center">
			{#if searchQuery.trim()}
				<div class="w-16 h-16 mx-auto bg-brutal-orange border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-8 h-8" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2">NO RESULTS</h3>
				<p class="text-black/60 font-medium mb-6">
					No workflows match "{searchQuery}"
				</p>
				<button
					onclick={() => (searchQuery = '')}
					class="btn-brutal bg-white text-black"
				>
					CLEAR SEARCH
				</button>
			{:else}
				<div class="w-20 h-20 mx-auto bg-brutal-purple border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-10 h-10" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2">NO WORKFLOWS YET</h3>
				<p class="text-black/60 font-medium mb-8 max-w-md mx-auto">
					Get started by recording a new workflow or importing an existing one.
				</p>
				<div class="flex justify-center gap-4">
					<a href="/record" class="btn-brutal bg-brutal-magenta text-black flex items-center gap-2">
						<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
							<circle cx="10" cy="10" r="6" />
						</svg>
						START RECORDING
					</a>
					<button
						onclick={openImportDialog}
						class="btn-brutal bg-brutal-purple text-black flex items-center gap-2"
					>
						<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
							<path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						IMPORT
					</button>
				</div>
			{/if}
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
			{#each filteredWorkflows() as workflow (workflow.id)}
				<WorkflowCard {workflow} />
			{/each}
		</div>
	{/if}
</div>
