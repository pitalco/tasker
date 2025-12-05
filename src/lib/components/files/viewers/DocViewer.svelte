<script lang="ts">
	import { onMount } from 'svelte';
	import mammoth from 'mammoth';
	import { decodeBase64ToBytes } from '$lib/services/filesService';

	let { contentBase64 }: { contentBase64: string } = $props();

	let html = $state('');
	let error = $state<string | null>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			const bytes = decodeBase64ToBytes(contentBase64);
			const result = await mammoth.convertToHtml({ arrayBuffer: bytes.buffer as ArrayBuffer });
			html = result.value;

			if (result.messages.length > 0) {
				console.warn('Mammoth warnings:', result.messages);
			}
		} catch (e) {
			console.error('Failed to parse DOCX file:', e);
			error = 'Failed to load document. Only .docx files are supported.';
		} finally {
			loading = false;
		}
	});
</script>

{#if loading}
	<div class="flex items-center justify-center py-8">
		<p class="text-black/60 font-medium">Loading document...</p>
	</div>
{:else if error}
	<p class="text-center text-red-600 font-medium py-8">{error}</p>
{:else}
	<div
		class="prose prose-lg max-w-none border-3 border-black bg-white p-6 overflow-auto max-h-[70vh]"
	>
		{@html html}
	</div>
{/if}
