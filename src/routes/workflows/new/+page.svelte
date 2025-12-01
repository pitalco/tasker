<script lang="ts">
	import { goto } from '$app/navigation';
	import { getWorkflowState } from '$lib/stores/workflow.svelte';

	const workflowState = getWorkflowState();

	// State
	let mode = $state<'choose' | 'describe'>('choose');
	let name = $state('');
	let taskDescription = $state('');
	let isCreating = $state(false);
	let error = $state<string | null>(null);

	async function createTextWorkflow() {
		if (!name.trim()) {
			error = 'Please enter a workflow name';
			return;
		}
		if (!taskDescription.trim()) {
			error = 'Please describe what you want to automate';
			return;
		}

		error = null;
		isCreating = true;

		try {
			const workflow = await workflowState.createWorkflow({
				name: name.trim(),
				task_description: taskDescription.trim(),
				metadata: {
					recording_source: 'text_description'
				}
			});

			if (workflow) {
				goto(`/workflows/${workflow.id}`);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to create workflow';
		} finally {
			isCreating = false;
		}
	}
</script>

<div class="max-w-3xl mx-auto space-y-8">
	<!-- Header -->
	<div>
		<a href="/" class="inline-flex items-center gap-2 text-black/60 font-bold hover:text-black mb-4">
			<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
				<path d="M15 19l-7-7 7-7" />
			</svg>
			BACK
		</a>
		<h1 class="text-4xl font-bold text-black tracking-tight">New Workflow</h1>
		<p class="text-lg text-black/60 font-medium mt-1">Choose how to create your automation</p>
	</div>

	{#if mode === 'choose'}
		<!-- Two option cards -->
		<div class="grid md:grid-cols-2 gap-6">
			<!-- Recording option -->
			<a
				href="/record"
				class="card-brutal p-8 hover:-translate-y-1 transition-transform group"
			>
				<div class="w-16 h-16 bg-brutal-magenta border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-8 h-8" fill="currentColor" viewBox="0 0 20 20">
						<circle cx="10" cy="10" r="6" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2 group-hover:underline">Start Recording</h3>
				<p class="text-black/60 font-medium mb-4">
					Record your actions in a browser and let AI learn from them. Best for complex or repetitive tasks.
				</p>
				<span class="inline-block px-3 py-1 bg-brutal-yellow border-2 border-black font-bold text-sm">
					RECOMMENDED
				</span>
			</a>

			<!-- Describe option -->
			<button
				onclick={() => (mode = 'describe')}
				class="card-brutal p-8 hover:-translate-y-1 transition-transform text-left group"
			>
				<div class="w-16 h-16 bg-brutal-purple border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-8 h-8" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2 group-hover:underline">Just Describe</h3>
				<p class="text-black/60 font-medium">
					Tell AI what you want to do in plain English. Great for quick tasks or when you know exactly what you need.
				</p>
			</button>
		</div>
	{:else}
		<!-- Description form -->
		<form onsubmit={(e) => { e.preventDefault(); createTextWorkflow(); }} class="card-brutal p-8 space-y-6">
			<div>
				<label for="name" class="block font-bold text-black mb-2">WORKFLOW NAME</label>
				<input
					id="name"
					type="text"
					bind:value={name}
					placeholder="e.g., Search Amazon for Keyboards"
					class="input-brutal"
				/>
			</div>

			<div>
				<label for="description" class="block font-bold text-black mb-2">WHAT DO YOU WANT TO AUTOMATE?</label>
				<textarea
					id="description"
					bind:value={taskDescription}
					rows="6"
					placeholder="Describe what you want to automate in plain English...

Example: Go to amazon.com, search for 'mechanical keyboard', filter by 4+ stars, sort by price low to high, and screenshot the first 5 results."
					class="input-brutal resize-none"
				></textarea>
				<p class="text-sm text-black/60 mt-2">
					Tip: Be specific about URLs, search terms, and the steps you want. The AI will figure out the details.
				</p>
			</div>

			{#if error}
				<div class="card-brutal bg-brutal-magenta p-4 flex items-center justify-between">
					<span class="font-bold">{error}</span>
					<button type="button" onclick={() => (error = null)} class="font-bold underline">
						DISMISS
					</button>
				</div>
			{/if}

			<div class="flex gap-4">
				<button
					type="button"
					onclick={() => (mode = 'choose')}
					class="btn-brutal bg-white text-black"
				>
					BACK
				</button>
				<button
					type="submit"
					disabled={isCreating}
					class="btn-brutal bg-brutal-purple text-black flex-1 disabled:opacity-50"
				>
					{isCreating ? 'CREATING...' : 'CREATE WORKFLOW'}
				</button>
			</div>
		</form>
	{/if}
</div>
