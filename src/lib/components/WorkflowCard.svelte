<script lang="ts">
	import type { Workflow } from '$lib/types/workflow';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';

	let { workflow }: { workflow: Workflow } = $props();

	const workflowState = getWorkflowState();

	let showDeleteConfirm = $state(false);

	function formatDate(dateString: string): string {
		const date = new Date(dateString);
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	async function handleDelete() {
		await workflowState.deleteWorkflow(workflow.id);
		showDeleteConfirm = false;
	}

	const colors = ['bg-brutal-cyan', 'bg-brutal-lime', 'bg-brutal-purple', 'bg-brutal-orange'];
	const colorIndex = workflow.name.length % colors.length;
	const accentColor = colors[colorIndex];
</script>

<div class="card-brutal p-0 overflow-hidden group">
	<!-- Colored header bar -->
	<div class="{accentColor} h-2 border-b-3 border-black"></div>

	<div class="p-5">
		<div class="flex items-start justify-between gap-3">
			<div class="flex-1 min-w-0">
				<a href="/workflows/{workflow.id}" class="block">
					<h3 class="text-lg font-bold text-black truncate group-hover:underline decoration-2 underline-offset-2">
						{workflow.name}
					</h3>
				</a>
				{#if workflow.task_description}
					<p class="text-sm text-black/60 mt-1 line-clamp-2 font-medium">{workflow.task_description}</p>
				{/if}
			</div>

			<!-- Action buttons -->
			<div class="flex items-center gap-1">
				<a
					href="/replay/{workflow.id}"
					class="p-2 border-2 border-black bg-brutal-lime hover:-translate-y-0.5 transition-transform"
					title="Run workflow"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
						<path d="M8 5v14l11-7z" />
					</svg>
				</a>
				<button
					onclick={() => (showDeleteConfirm = true)}
					class="p-2 border-2 border-black bg-brutal-magenta hover:-translate-y-0.5 transition-transform cursor-pointer"
					title="Delete workflow"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					<svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
						<path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
					</svg>
				</button>
			</div>
		</div>

		<!-- Stats row -->
		<div class="mt-4">
			<span class="text-xs font-bold text-black/60 uppercase">
				{formatDate(workflow.updated_at)}
			</span>
		</div>

		<!-- Recorded badge -->
		{#if workflow.metadata.recording_source === 'recorded'}
			<div class="mt-3">
				<span class="inline-flex items-center gap-1.5 px-3 py-1 bg-brutal-magenta border-2 border-black text-xs font-bold">
					<svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
						<circle cx="10" cy="10" r="5" />
					</svg>
					RECORDED
				</span>
			</div>
		{/if}
	</div>
</div>

<!-- Delete confirmation modal -->
{#if showDeleteConfirm}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
		<div class="card-brutal p-6 w-full max-w-sm bg-white">
			<h3 class="text-xl font-bold text-black">DELETE WORKFLOW?</h3>
			<p class="mt-3 text-black/70 font-medium">
				Are you sure you want to delete "<span class="font-bold">{workflow.name}</span>"? This action cannot be undone.
			</p>
			<div class="mt-6 flex justify-end gap-3">
				<button
					onclick={() => (showDeleteConfirm = false)}
					class="btn-brutal bg-white text-black"
				>
					CANCEL
				</button>
				<button
					onclick={handleDelete}
					class="btn-brutal bg-brutal-magenta text-black"
				>
					DELETE
				</button>
			</div>
		</div>
	</div>
{/if}
