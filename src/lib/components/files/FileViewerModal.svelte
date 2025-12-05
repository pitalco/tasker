<script lang="ts">
	import { untrack } from 'svelte';
	import { formatFileSize, getFileContent, downloadFile } from '$lib/services/filesService';
	import type { TaskerFile, FileContentResponse } from '$lib/types/file';
	import FileViewer from './FileViewer.svelte';

	let {
		file,
		onClose
	}: {
		file: TaskerFile;
		onClose: () => void;
	} = $props();

	let fileContent = $state<FileContentResponse | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let downloading = $state(false);

	$effect(() => {
		// Track file.id - effect only re-runs when file changes
		file.id;
		// Untrack loadContent to prevent state updates from re-triggering effect
		untrack(() => loadContent());
	});

	async function loadContent() {
		loading = true;
		error = null;

		try {
			fileContent = await getFileContent(file.id);
		} catch (e) {
			console.error('Failed to load file content:', e);
			error = 'Failed to load file content';
		} finally {
			loading = false;
		}
	}

	async function handleDownload() {
		downloading = true;
		try {
			await downloadFile(file.id, file.file_name);
		} catch (e) {
			console.error('Failed to download file:', e);
		} finally {
			downloading = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<div
	class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4"
	onclick={onClose}
	role="dialog"
	aria-modal="true"
	aria-labelledby="modal-title"
>
	<!-- Modal -->
	<div
		class="bg-white border-3 border-black w-full max-w-5xl max-h-[90vh] flex flex-col"
		style="box-shadow: 6px 6px 0 0 #000;"
		onclick={(e) => e.stopPropagation()}
		role="document"
	>
		<!-- Header -->
		<div class="flex items-center justify-between p-4 border-b-3 border-black bg-brutal-yellow">
			<div class="flex-1 min-w-0">
				<h2 id="modal-title" class="text-xl font-bold truncate">{file.file_name}</h2>
				<p class="text-sm text-black/60">
					{file.file_path} • {formatFileSize(file.file_size)}
					{#if file.run_name}
						• Run: {file.run_name}
					{/if}
				</p>
			</div>

			<div class="flex gap-2 ml-4">
				<button
					onclick={handleDownload}
					disabled={downloading}
					class="px-4 py-2 border-3 border-black bg-white font-bold hover:-translate-y-0.5 transition-transform disabled:opacity-50 cursor-pointer"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					{downloading ? 'Saving...' : 'Download'}
				</button>
				<button
					onclick={onClose}
					class="px-3 py-2 border-3 border-black bg-white font-bold hover:-translate-y-0.5 transition-transform cursor-pointer"
					style="box-shadow: 2px 2px 0 0 #000;"
					aria-label="Close"
				>
					✕
				</button>
			</div>
		</div>

		<!-- Content -->
		<div class="flex-1 overflow-auto p-4">
			{#if loading}
				<div class="flex items-center justify-center py-12">
					<p class="text-black/60 font-medium">Loading file...</p>
				</div>
			{:else if error}
				<div class="flex items-center justify-center py-12">
					<p class="text-red-600 font-medium">{error}</p>
				</div>
			{:else if fileContent}
				<FileViewer file={fileContent} onDownload={handleDownload} />
			{/if}
		</div>
	</div>
</div>
