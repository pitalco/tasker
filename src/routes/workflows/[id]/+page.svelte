<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';
	import { downloadTaskfile } from '$lib/services/taskfileService';
	import type { Workflow, WorkflowVariable } from '$lib/types/workflow';

	const workflowState = getWorkflowState();

	let workflow = $state<Workflow | null>(null);
	let isLoading = $state(true);
	let isSaving = $state(false);
	let error = $state<string | null>(null);
	let hasChanges = $state(false);

	// Editable fields
	let editName = $state('');
	let editTaskDescription = $state('');
	let editStopWhen = $state('');
	let editMaxSteps = $state<number | null>(null);
	let editVariables = $state<WorkflowVariable[]>([]);

	// UI state
	let showDeleteConfirm = $state(false);
	let showAddVariable = $state(false);
	let newVariableName = $state('');
	let newVariableValue = $state('');
	let isExporting = $state(false);

	const workflowId = $derived($page.params.id);

	onMount(async () => {
		await workflowState.loadWorkflows();
		const found = workflowState.workflows.find((w) => w.id === workflowId);

		if (found) {
			workflow = found;
			editName = found.name;
			editTaskDescription = found.task_description || '';
			editStopWhen = found.stop_when || '';
			editMaxSteps = found.max_steps ?? null;
			editVariables = JSON.parse(JSON.stringify(found.variables || []));
		}

		isLoading = false;
	});

	function markChanged() {
		hasChanges = true;
	}

	async function handleSave() {
		if (!workflow) return;

		isSaving = true;
		error = null;

		try {
			await workflowState.updateWorkflow(workflow.id, {
				name: editName,
				task_description: editTaskDescription || undefined,
				stop_when: editStopWhen || undefined,
				max_steps: editMaxSteps ?? undefined,
				variables: editVariables,
				metadata: workflow.metadata
			});

			hasChanges = false;
			// Refresh workflow
			await workflowState.loadWorkflows();
			workflow = workflowState.workflows.find((w) => w.id === workflowId) || null;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		} finally {
			isSaving = false;
		}
	}

	async function handleDelete() {
		if (!workflow) return;

		try {
			await workflowState.deleteWorkflow(workflow.id);
			goto('/');
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete';
		}
	}

	function addVariable() {
		if (!newVariableName.trim()) return;

		editVariables = [
			...editVariables,
			{
				name: newVariableName.trim(),
				type: 'string',
				default_value: newVariableValue || undefined
			}
		];

		newVariableName = '';
		newVariableValue = '';
		showAddVariable = false;
		markChanged();
	}

	function removeVariable(name: string) {
		editVariables = editVariables.filter((v) => v.name !== name);
		markChanged();
	}

	async function handleExport() {
		if (!workflow) return;

		isExporting = true;
		error = null;

		try {
			await downloadTaskfile(workflow.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to export taskfile';
		} finally {
			isExporting = false;
		}
	}
</script>

<div class="max-w-4xl mx-auto space-y-8">
	<!-- Header -->
	<div class="flex items-start justify-between">
		<div>
			<button
				onclick={() => goto('/')}
				class="text-sm font-bold text-black/60 hover:text-black mb-2 flex items-center gap-1 cursor-pointer"
			>
				<svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
					<path d="M15 19l-7-7 7-7" />
				</svg>
				BACK TO WORKFLOWS
			</button>
			<h1 class="text-4xl font-bold text-black tracking-tight">Edit Workflow</h1>
		</div>

		{#if workflow && !isLoading}
			<div class="flex items-center gap-3">
				<button
					onclick={handleExport}
					disabled={isExporting}
					class="btn-brutal bg-brutal-purple text-black flex items-center gap-2 disabled:opacity-50"
				>
					<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
					</svg>
					{isExporting ? 'EXPORTING...' : 'EXPORT'}
				</button>
				<a
					href="/replay/{workflow.id}"
					class="btn-brutal bg-brutal-lime text-black flex items-center gap-2"
				>
					<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
						<path d="M8 5v14l11-7z" />
					</svg>
					RUN
				</a>
				<button
					onclick={() => (showDeleteConfirm = true)}
					class="btn-brutal bg-brutal-magenta text-black"
				>
					DELETE
				</button>
			</div>
		{/if}
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
	{:else}
		<!-- Workflow Details -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-cyan h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-6">
				<h2 class="text-xl font-bold text-black">DETAILS</h2>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Name</label>
					<input
						type="text"
						bind:value={editName}
						oninput={markChanged}
						class="input-brutal"
						placeholder="Workflow name"
					/>
				</div>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Task Description</label>
					<textarea
						bind:value={editTaskDescription}
						oninput={markChanged}
						class="input-brutal h-60 resize-none"
						placeholder="What does this workflow automate?"
					></textarea>
				</div>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Stop When (Optional)</label>
					<textarea
						bind:value={editStopWhen}
						oninput={markChanged}
						class="input-brutal h-16 resize-none"
						placeholder="e.g., you have collected at least 10 results"
					></textarea>
					<p class="text-xs text-black/50 mt-1">
						Agent will NOT stop until this condition is met
					</p>
				</div>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Max Steps (Optional)</label>
					<input
						type="number"
						bind:value={editMaxSteps}
						oninput={markChanged}
						class="input-brutal w-32"
						placeholder="50"
						min="1"
						max="500"
					/>
					<p class="text-xs text-black/50 mt-1">
						Leave empty to use global default from Settings
					</p>
				</div>

				<!-- Metadata badges -->
				<div class="flex flex-wrap gap-2">
					{#if workflow.metadata?.recording_source === 'recorded'}
						<span class="px-3 py-1 bg-brutal-magenta border-2 border-black font-bold text-xs">
							RECORDED
						</span>
					{:else if workflow.metadata?.recording_source === 'text_description'}
						<span class="px-3 py-1 bg-brutal-purple border-2 border-black font-bold text-xs">
							TEXT-BASED
						</span>
					{/if}
					<span class="px-3 py-1 bg-brutal-cyan border-2 border-black font-bold text-xs">
						{new Date(workflow.created_at).toLocaleDateString()}
					</span>
				</div>
			</div>
		</div>

		<!-- Variables -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-lime h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-4">
				<div class="flex items-center justify-between">
					<h2 class="text-xl font-bold text-black">VARIABLES</h2>
					<button
						onclick={() => (showAddVariable = true)}
						class="btn-brutal bg-white text-black text-sm py-2"
					>
						+ ADD VARIABLE
					</button>
				</div>

				<p class="text-sm text-black/60 font-medium">
					Use variables in your workflow with {'{{variable_name}}'} syntax. Variables can be filled
					at runtime.
				</p>

				{#if editVariables.length === 0}
					<div class="text-center py-6 border-3 border-dashed border-black/30">
						<p class="text-black/60 font-medium">No variables defined</p>
					</div>
				{:else}
					<div class="space-y-2">
						{#each editVariables as variable}
							<div
								class="flex items-center justify-between p-3 bg-brutal-lime/30 border-2 border-black"
							>
								<div>
									<span class="font-bold text-black">{`{{${variable.name}}}`}</span>
									{#if variable.default_value}
										<span class="text-black/60 ml-2">= {variable.default_value}</span>
									{/if}
								</div>
								<button
									onclick={() => removeVariable(variable.name)}
									class="p-1 hover:bg-black/10"
									title="Remove variable"
								>
									<svg
										class="w-4 h-4"
										fill="none"
										stroke="currentColor"
										stroke-width="2"
										viewBox="0 0 24 24"
									>
										<path d="M6 18L18 6M6 6l12 12" />
									</svg>
								</button>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- Save bar -->
		{#if hasChanges}
			<div
				class="fixed bottom-0 left-0 right-0 bg-brutal-yellow border-t-4 border-black p-4 z-50"
			>
				<div class="max-w-4xl mx-auto flex items-center justify-between">
					<span class="font-bold text-black">You have unsaved changes</span>
					<div class="flex gap-3">
						<button
							onclick={() => {
								editName = workflow?.name || '';
								editTaskDescription = workflow?.task_description || '';
								editStopWhen = workflow?.stop_when || '';
								editMaxSteps = workflow?.max_steps ?? null;
								editVariables = JSON.parse(JSON.stringify(workflow?.variables || []));
								hasChanges = false;
							}}
							class="btn-brutal bg-white text-black"
						>
							DISCARD
						</button>
						<button
							onclick={handleSave}
							disabled={isSaving}
							class="btn-brutal bg-black text-white disabled:opacity-50"
						>
							{isSaving ? 'SAVING...' : 'SAVE CHANGES'}
						</button>
					</div>
				</div>
			</div>
		{/if}
	{/if}
</div>

<!-- Delete confirmation modal -->
{#if showDeleteConfirm}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
		<div class="card-brutal p-6 w-full max-w-sm bg-white">
			<h3 class="text-xl font-bold text-black">DELETE WORKFLOW?</h3>
			<p class="mt-3 text-black/70 font-medium">
				Are you sure you want to delete "<span class="font-bold">{workflow?.name}</span>"? This
				action cannot be undone.
			</p>
			<div class="mt-6 flex justify-end gap-3">
				<button onclick={() => (showDeleteConfirm = false)} class="btn-brutal bg-white text-black">
					CANCEL
				</button>
				<button onclick={handleDelete} class="btn-brutal bg-brutal-magenta text-black">
					DELETE
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Add variable modal -->
{#if showAddVariable}
	<div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
		<div class="card-brutal p-6 w-full max-w-sm bg-white">
			<h3 class="text-xl font-bold text-black mb-6">ADD VARIABLE</h3>
			<div class="space-y-4">
				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Variable Name</label>
					<input
						type="text"
						bind:value={newVariableName}
						class="input-brutal"
						placeholder="company_name"
					/>
				</div>
				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2"
						>Default Value (Optional)</label
					>
					<input
						type="text"
						bind:value={newVariableValue}
						class="input-brutal"
						placeholder="Acme Inc"
					/>
				</div>
			</div>
			<div class="mt-6 flex justify-end gap-3">
				<button
					onclick={() => {
						showAddVariable = false;
						newVariableName = '';
						newVariableValue = '';
					}}
					class="btn-brutal bg-white text-black"
				>
					CANCEL
				</button>
				<button
					onclick={addVariable}
					disabled={!newVariableName.trim()}
					class="btn-brutal bg-brutal-lime text-black disabled:opacity-50"
				>
					ADD
				</button>
			</div>
		</div>
	</div>
{/if}
