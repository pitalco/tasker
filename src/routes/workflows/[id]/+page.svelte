<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';
	import { downloadTaskfile } from '$lib/services/taskfileService';
	import { formatStepDescription } from '$lib/utils/stepFormatter';
	import type { Workflow, WorkflowStep, WorkflowVariable } from '$lib/types/workflow';

	const workflowState = getWorkflowState();

	let workflow = $state<Workflow | null>(null);
	let isLoading = $state(true);
	let isSaving = $state(false);
	let error = $state<string | null>(null);
	let hasChanges = $state(false);

	// Editable fields
	let editName = $state('');
	let editDescription = $state('');
	let editStartUrl = $state('');
	let editSteps = $state<WorkflowStep[]>([]);
	let editVariables = $state<WorkflowVariable[]>([]);

	// UI state
	let expandedSteps = $state<Set<string>>(new Set());
	let editingStepId = $state<string | null>(null);
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
			editDescription = found.description || '';
			editStartUrl = found.metadata?.start_url || '';
			editSteps = JSON.parse(JSON.stringify(found.steps)); // Deep clone
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
				description: editDescription || undefined,
				steps: editSteps,
				variables: editVariables,
				metadata: {
					...workflow.metadata,
					start_url: editStartUrl
				}
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

	function toggleStep(stepId: string) {
		const newSet = new Set(expandedSteps);
		if (newSet.has(stepId)) {
			newSet.delete(stepId);
		} else {
			newSet.add(stepId);
		}
		expandedSteps = newSet;
	}

	function moveStep(index: number, direction: 'up' | 'down') {
		const newIndex = direction === 'up' ? index - 1 : index + 1;
		if (newIndex < 0 || newIndex >= editSteps.length) return;

		const newSteps = [...editSteps];
		[newSteps[index], newSteps[newIndex]] = [newSteps[newIndex], newSteps[index]];

		// Update order property
		newSteps.forEach((step, i) => {
			step.order = i + 1;
		});

		editSteps = newSteps;
		markChanged();
	}

	function deleteStep(index: number) {
		editSteps = editSteps.filter((_, i) => i !== index);
		editSteps.forEach((step, i) => {
			step.order = i + 1;
		});
		markChanged();
	}

	function updateStepDescription(index: number, description: string) {
		editSteps[index].description = description;
		editSteps = [...editSteps];
		markChanged();
	}

	function updateStepValue(index: number, value: string) {
		const action = editSteps[index].action;
		if (action) {
			if (action.type === 'type') {
				(action as { type: 'type'; text: string }).text = value;
			} else if (action.type === 'navigate') {
				(action as { type: 'navigate'; url: string }).url = value;
			}
			editSteps = [...editSteps];
			markChanged();
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

	function getActionIcon(actionType: string): string {
		switch (actionType) {
			case 'click':
				return 'M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5M7.188 2.239l.777 2.897M5.136 7.965l-2.898-.777M13.95 4.05l-2.122 2.122m-5.657 5.656l-2.12 2.122';
			case 'type':
				return 'M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z';
			case 'navigate':
				return 'M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14';
			case 'scroll':
				return 'M19 14l-7 7m0 0l-7-7m7 7V3';
			case 'select':
				return 'M8 9l4-4 4 4m0 6l-4 4-4-4';
			case 'wait':
				return 'M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z';
			default:
				return 'M13 10V3L4 14h7v7l9-11h-7z';
		}
	}

	function getActionColor(actionType: string): string {
		switch (actionType) {
			case 'click':
				return 'bg-brutal-magenta';
			case 'type':
				return 'bg-brutal-cyan';
			case 'navigate':
				return 'bg-brutal-purple';
			case 'scroll':
				return 'bg-brutal-orange';
			case 'select':
				return 'bg-brutal-lime';
			default:
				return 'bg-brutal-yellow';
		}
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
				class="text-sm font-bold text-black/60 hover:text-black mb-2 flex items-center gap-1"
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

				<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
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
						<label class="block text-sm font-bold text-black uppercase mb-2">Start URL</label>
						<input
							type="url"
							bind:value={editStartUrl}
							oninput={markChanged}
							class="input-brutal"
							placeholder="https://example.com"
						/>
					</div>
				</div>

				<div>
					<label class="block text-sm font-bold text-black uppercase mb-2">Description</label>
					<textarea
						bind:value={editDescription}
						oninput={markChanged}
						class="input-brutal h-20 resize-none"
						placeholder="What does this workflow do?"
					></textarea>
				</div>

				<!-- Metadata badges -->
				<div class="flex flex-wrap gap-2">
					<span class="px-3 py-1 bg-black text-white font-bold text-xs">
						{editSteps.length} STEPS
					</span>
					{#if workflow.metadata?.recording_source === 'recorded'}
						<span class="px-3 py-1 bg-brutal-magenta border-2 border-black font-bold text-xs">
							RECORDED
						</span>
					{/if}
					<span class="px-3 py-1 bg-brutal-purple border-2 border-black font-bold text-xs">
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

		<!-- Steps -->
		<div class="card-brutal p-0 overflow-hidden">
			<div class="bg-brutal-orange h-2 border-b-3 border-black"></div>
			<div class="p-6 space-y-4">
				<h2 class="text-xl font-bold text-black">STEPS ({editSteps.length})</h2>

				{#if editSteps.length === 0}
					<div class="text-center py-8 border-3 border-dashed border-black/30">
						<p class="text-black/60 font-medium">No steps in this workflow</p>
					</div>
				{:else}
					<div class="space-y-3">
						{#each editSteps as step, index (step.id)}
							<div class="border-3 border-black bg-white" style="box-shadow: 2px 2px 0 0 #000;">
								<!-- Step header -->
								<div
									role="button"
									tabindex="0"
									onclick={() => toggleStep(step.id)}
									onkeydown={(e) => e.key === 'Enter' && toggleStep(step.id)}
									class="w-full p-4 flex items-center gap-4 text-left hover:bg-gray-50 cursor-pointer"
								>
									<span
										class="w-8 h-8 flex items-center justify-center bg-black text-white font-bold text-sm flex-shrink-0"
									>
										{index + 1}
									</span>

									<div
										class="w-8 h-8 flex items-center justify-center {getActionColor(
											step.action?.type || ''
										)} border-2 border-black flex-shrink-0"
									>
										<svg
											class="w-4 h-4"
											fill="none"
											stroke="currentColor"
											stroke-width="2"
											viewBox="0 0 24 24"
										>
											<path d={getActionIcon(step.action?.type || '')} />
										</svg>
									</div>

									<div class="flex-1 min-w-0">
										<div class="font-bold text-black truncate">{formatStepDescription(step)}</div>
										<div class="text-xs text-black/60 font-medium uppercase">
											{step.action?.type || 'action'}
										</div>
									</div>

									<div class="flex items-center gap-2">
										<button
											onclick={(e) => {
												e.stopPropagation();
												moveStep(index, 'up');
											}}
											disabled={index === 0}
											class="p-1.5 border-2 border-black bg-white disabled:opacity-30"
											title="Move up"
										>
											<svg
												class="w-4 h-4"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												viewBox="0 0 24 24"
											>
												<path d="M5 15l7-7 7 7" />
											</svg>
										</button>
										<button
											onclick={(e) => {
												e.stopPropagation();
												moveStep(index, 'down');
											}}
											disabled={index === editSteps.length - 1}
											class="p-1.5 border-2 border-black bg-white disabled:opacity-30"
											title="Move down"
										>
											<svg
												class="w-4 h-4"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												viewBox="0 0 24 24"
											>
												<path d="M19 9l-7 7-7-7" />
											</svg>
										</button>
										<button
											onclick={(e) => {
												e.stopPropagation();
												deleteStep(index);
											}}
											class="p-1.5 border-2 border-black bg-brutal-magenta"
											title="Delete step"
										>
											<svg
												class="w-4 h-4"
												fill="none"
												stroke="currentColor"
												stroke-width="2"
												viewBox="0 0 24 24"
											>
												<path
													d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
												/>
											</svg>
										</button>
										<svg
											class="w-5 h-5 transition-transform {expandedSteps.has(step.id)
												? 'rotate-180'
												: ''}"
											fill="none"
											stroke="currentColor"
											stroke-width="2"
											viewBox="0 0 24 24"
										>
											<path d="M19 9l-7 7-7-7" />
										</svg>
									</div>
								</div>

								<!-- Step details (expanded) -->
								{#if expandedSteps.has(step.id)}
									<div class="border-t-3 border-black p-4 bg-gray-50 space-y-4">
										<div>
											<label class="block text-sm font-bold text-black uppercase mb-2"
												>Step Description</label
											>
											<input
												type="text"
												value={step.description || ''}
												oninput={(e) => updateStepDescription(index, e.currentTarget.value)}
												class="input-brutal"
											/>
										</div>

										{#if step.action?.type === 'type'}
											<div>
												<label class="block text-sm font-bold text-black uppercase mb-2">
													Text to Type
												</label>
												<input
													type="text"
													value={(step.action as { text?: string }).text || ''}
													oninput={(e) => updateStepValue(index, e.currentTarget.value)}
													class="input-brutal"
													placeholder={'Enter text or {{variable}}'}
												/>
											</div>
										{/if}

										{#if step.action?.type === 'navigate'}
											<div>
												<label class="block text-sm font-bold text-black uppercase mb-2">
													URL
												</label>
												<input
													type="text"
													value={(step.action as { url?: string }).url || ''}
													oninput={(e) => updateStepValue(index, e.currentTarget.value)}
													class="input-brutal"
													placeholder="https://..."
												/>
											</div>
										{/if}

										{#if step.action?.type === 'click' || step.action?.type === 'type'}
											{@const action = step.action as { selector?: { css?: string; xpath?: string; text?: string } }}
											{#if action.selector}
												<div>
													<label class="block text-sm font-bold text-black uppercase mb-2"
														>Selector</label
													>
													<code
														class="block p-3 bg-black text-brutal-lime font-mono text-sm break-all"
													>
														{action.selector.css || action.selector.xpath || action.selector.text || 'N/A'}
													</code>
												</div>
											{/if}
										{/if}

										{#if step.screenshot_path}
											<div>
												<label class="block text-sm font-bold text-black uppercase mb-2"
													>Screenshot</label
												>
												<img
													src={step.screenshot_path}
													alt="Step screenshot"
													class="border-3 border-black max-w-full"
												/>
											</div>
										{/if}
									</div>
								{/if}
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
								editDescription = workflow?.description || '';
								editStartUrl = workflow?.metadata?.start_url || '';
								editSteps = JSON.parse(JSON.stringify(workflow?.steps || []));
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
