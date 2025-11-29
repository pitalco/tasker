<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getRunsState } from '$lib/stores/runs.svelte';
	import { formatRunStatus, getStatusColorClass, formatRelativeTime } from '$lib/services/runsService';
	import type { RunStatus } from '$lib/types/run';

	const runsState = getRunsState();

	const statusFilters: { value: RunStatus | null; label: string }[] = [
		{ value: null, label: 'ALL' },
		{ value: 'running', label: 'RUNNING' },
		{ value: 'completed', label: 'COMPLETED' },
		{ value: 'failed', label: 'FAILED' },
		{ value: 'pending', label: 'PENDING' },
		{ value: 'cancelled', label: 'CANCELLED' }
	];

	let selectedRun = $state<string | null>(null);
	let showDeleteConfirm = $state<string | null>(null);

	onMount(() => {
		runsState.loadRuns();
	});

	function viewRun(runId: string) {
		goto(`/runs/${runId}`);
	}

	async function handleCancel(runId: string, event: Event) {
		event.stopPropagation();
		await runsState.cancelRun(runId);
	}

	async function handleDelete(runId: string) {
		await runsState.deleteRun(runId);
		showDeleteConfirm = null;
	}

	function confirmDelete(runId: string, event: Event) {
		event.stopPropagation();
		showDeleteConfirm = runId;
	}

	function cancelDelete(event: Event) {
		event.stopPropagation();
		showDeleteConfirm = null;
	}
</script>

<div class="space-y-8">
	<!-- Header -->
	<div class="flex items-end justify-between">
		<div>
			<h1 class="text-4xl font-bold text-black tracking-tight">Runs</h1>
			<p class="text-lg text-black/60 font-medium mt-1">Execution history and real-time logs</p>
		</div>
	</div>

	<!-- Filters -->
	<div class="flex flex-wrap gap-2">
		{#each statusFilters as filter}
			<button
				onclick={() => runsState.setStatusFilter(filter.value)}
				class="px-4 py-2 border-3 border-black font-bold text-sm transition-all {runsState.statusFilter === filter.value
					? 'bg-black text-white'
					: 'bg-white text-black hover:-translate-y-0.5'}"
				style="box-shadow: {runsState.statusFilter === filter.value ? '0 0 0 0 #000' : '2px 2px 0 0 #000'};"
			>
				{filter.label}
			</button>
		{/each}
	</div>

	<!-- Error message -->
	{#if runsState.error}
		<div class="card-brutal bg-brutal-magenta p-4 flex items-center justify-between">
			<span class="font-bold">{runsState.error}</span>
			<button onclick={() => runsState.clearError()} class="font-bold underline">
				DISMISS
			</button>
		</div>
	{/if}

	<!-- Run count -->
	{#if !runsState.isLoading && runsState.runs.length > 0}
		<div class="flex items-center gap-3">
			<span class="px-3 py-1 bg-black text-white font-bold text-sm">
				{runsState.total} RUN{runsState.total !== 1 ? 'S' : ''}
			</span>
			{#if runsState.statusFilter}
				<span class="text-sm font-medium text-black/60">
					filtered by {runsState.statusFilter}
				</span>
			{/if}
		</div>
	{/if}

	<!-- Runs list -->
	{#if runsState.isLoading}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else if runsState.runs.length === 0}
		<div class="card-brutal p-12 text-center">
			<div class="w-20 h-20 mx-auto bg-brutal-cyan border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
				<svg class="w-10 h-10" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
					<path d="M13 10V3L4 14h7v7l9-11h-7z" />
				</svg>
			</div>
			<h3 class="text-2xl font-bold text-black mb-2">NO RUNS YET</h3>
			<p class="text-black/60 font-medium mb-8 max-w-md mx-auto">
				Start a workflow run from the Workflows page to see execution history here.
			</p>
			<a href="/" class="btn-brutal bg-brutal-purple text-black inline-flex items-center gap-2">
				<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
					<path d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
				</svg>
				VIEW WORKFLOWS
			</a>
		</div>
	{:else}
		<div class="space-y-4">
			{#each runsState.runs as run (run.id)}
				<div
					class="card-brutal bg-white p-4 cursor-pointer hover:-translate-y-0.5 transition-transform"
					onclick={() => viewRun(run.id)}
					role="button"
					tabindex="0"
					onkeypress={(e) => e.key === 'Enter' && viewRun(run.id)}
				>
					<div class="flex items-start justify-between gap-4">
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-3 mb-2">
								<span class="px-2 py-1 text-xs font-bold border-2 border-black {getStatusColorClass(run.status)}">
									{formatRunStatus(run.status)}
								</span>
								<span class="text-sm font-medium text-black/60">
									{formatRelativeTime(run.created_at)}
								</span>
							</div>
							<h3 class="font-bold text-lg text-black truncate">
								{run.workflow_name || run.task_description || 'Unnamed Run'}
							</h3>
							{#if run.task_description && run.workflow_name}
								<p class="text-sm text-black/60 mt-1 truncate">{run.task_description}</p>
							{/if}
							{#if run.error}
								<p class="text-sm text-brutal-magenta font-medium mt-2 truncate">
									Error: {run.error}
								</p>
							{/if}
						</div>
						<div class="flex items-center gap-2">
							{#if run.status === 'running' || run.status === 'pending'}
								<button
									onclick={(e) => handleCancel(run.id, e)}
									class="px-3 py-2 bg-brutal-orange border-2 border-black font-bold text-sm hover:-translate-y-0.5 transition-transform"
									style="box-shadow: 2px 2px 0 0 #000;"
								>
									CANCEL
								</button>
							{/if}
							{#if showDeleteConfirm === run.id}
								<div class="flex items-center gap-2">
									<button
										onclick={(e) => { e.stopPropagation(); handleDelete(run.id); }}
										class="px-3 py-2 bg-brutal-magenta border-2 border-black font-bold text-sm"
										style="box-shadow: 2px 2px 0 0 #000;"
									>
										CONFIRM
									</button>
									<button
										onclick={cancelDelete}
										class="px-3 py-2 bg-white border-2 border-black font-bold text-sm"
										style="box-shadow: 2px 2px 0 0 #000;"
									>
										CANCEL
									</button>
								</div>
							{:else}
								<button
									onclick={(e) => confirmDelete(run.id, e)}
									class="px-3 py-2 bg-white border-2 border-black font-bold text-sm hover:-translate-y-0.5 transition-transform"
									style="box-shadow: 2px 2px 0 0 #000;"
								>
									DELETE
								</button>
							{/if}
						</div>
					</div>
				</div>
			{/each}
		</div>

		<!-- Pagination -->
		{#if runsState.total > runsState.perPage}
			<div class="flex justify-center gap-2 mt-8">
				<button
					onclick={() => runsState.setPage(runsState.page - 1)}
					disabled={runsState.page <= 1}
					class="px-4 py-2 border-3 border-black font-bold disabled:opacity-50 disabled:cursor-not-allowed {runsState.page > 1 ? 'bg-white hover:-translate-y-0.5' : 'bg-gray-200'}"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					PREV
				</button>
				<span class="px-4 py-2 font-bold">
					Page {runsState.page} of {Math.ceil(runsState.total / runsState.perPage)}
				</span>
				<button
					onclick={() => runsState.setPage(runsState.page + 1)}
					disabled={runsState.page >= Math.ceil(runsState.total / runsState.perPage)}
					class="px-4 py-2 border-3 border-black font-bold disabled:opacity-50 disabled:cursor-not-allowed {runsState.page < Math.ceil(runsState.total / runsState.perPage) ? 'bg-white hover:-translate-y-0.5' : 'bg-gray-200'}"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					NEXT
				</button>
			</div>
		{/if}
	{/if}
</div>
