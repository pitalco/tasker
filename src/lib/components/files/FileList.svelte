<script lang="ts">
	import type { TaskerFile } from '$lib/types/file';
	import { downloadFile, formatFileSize, getFileCategory, getCategoryColorClass } from '$lib/services/filesService';
	import FileViewerModal from './FileViewerModal.svelte';

	let {
		files,
		loading = false,
		showDelete = true,
		onDelete
	}: {
		files: TaskerFile[];
		loading?: boolean;
		showDelete?: boolean;
		onDelete?: (file: TaskerFile) => void;
	} = $props();

	let selectedFile = $state<TaskerFile | null>(null);
	let showDeleteConfirm = $state<string | null>(null);

	async function handleDownload(file: TaskerFile, event: Event) {
		event.stopPropagation();
		try {
			await downloadFile(file.id, file.file_name);
		} catch (e) {
			console.error('Failed to download file:', e);
		}
	}

	function viewFile(file: TaskerFile) {
		selectedFile = file;
	}

	function closeModal() {
		selectedFile = null;
	}

	function confirmDelete(fileId: string, event: Event) {
		event.stopPropagation();
		showDeleteConfirm = fileId;
	}

	function cancelDelete(event: Event) {
		event.stopPropagation();
		showDeleteConfirm = null;
	}

	function handleDelete(file: TaskerFile, event: Event) {
		event.stopPropagation();
		onDelete?.(file);
		showDeleteConfirm = null;
	}

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}
</script>

<!-- Files list -->
{#if loading}
	<div class="flex items-center justify-center py-16">
		<div class="flex flex-col items-center gap-4">
			<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
			<span class="font-bold text-black">LOADING...</span>
		</div>
	</div>
{:else if files.length === 0}
	<div class="card-brutal p-12 text-center">
		<div class="w-16 h-16 mx-auto bg-brutal-purple border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
			<svg class="w-8 h-8" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
				<path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
			</svg>
		</div>
		<h3 class="text-2xl font-bold text-black mb-2">NO FILES</h3>
		<p class="text-black/60 font-medium">
			No files have been created for this run yet.
		</p>
	</div>
{:else}
	<div class="space-y-4">
		{#each files as file (file.id)}
			{@const category = getFileCategory(file.mime_type, file.file_name)}
			<div
				class="card-brutal bg-white p-4 cursor-pointer hover:-translate-y-0.5 transition-transform"
				onclick={() => viewFile(file)}
				role="button"
				tabindex="0"
				onkeypress={(e) => e.key === 'Enter' && viewFile(file)}
			>
				<div class="flex items-start justify-between gap-4">
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-3 mb-2">
							<span class="px-2 py-1 text-xs font-bold border-2 border-black {getCategoryColorClass(category)}">
								{category.toUpperCase()}
							</span>
							<span class="text-sm font-medium text-black/60">
								{formatDate(file.created_at)}
							</span>
							<span class="text-sm font-medium text-black/40">
								{formatFileSize(file.file_size)}
							</span>
						</div>
						<h3 class="font-bold text-lg text-black truncate">
							{file.file_name}
						</h3>
						<p class="text-sm text-black/60 mt-1 truncate">{file.file_path}</p>
					</div>
					<div class="flex items-center gap-2">
						<button
							onclick={(e) => handleDownload(file, e)}
							class="px-3 py-2 bg-brutal-yellow border-2 border-black font-bold text-sm hover:-translate-y-0.5 transition-transform cursor-pointer"
							style="box-shadow: 2px 2px 0 0 #000;"
						>
							DOWNLOAD
						</button>
						{#if showDelete && onDelete}
							{#if showDeleteConfirm === file.id}
								<div class="flex items-center gap-2">
									<button
										onclick={(e) => handleDelete(file, e)}
										class="px-3 py-2 bg-brutal-magenta border-2 border-black font-bold text-sm cursor-pointer hover:-translate-y-0.5 transition-transform"
										style="box-shadow: 2px 2px 0 0 #000;"
									>
										CONFIRM
									</button>
									<button
										onclick={cancelDelete}
										class="px-3 py-2 bg-white border-2 border-black font-bold text-sm cursor-pointer hover:-translate-y-0.5 transition-transform"
										style="box-shadow: 2px 2px 0 0 #000;"
									>
										CANCEL
									</button>
								</div>
							{:else}
								<button
									onclick={(e) => confirmDelete(file.id, e)}
									class="px-3 py-2 bg-white border-2 border-black font-bold text-sm hover:-translate-y-0.5 transition-transform cursor-pointer"
									style="box-shadow: 2px 2px 0 0 #000;"
								>
									DELETE
								</button>
							{/if}
						{/if}
					</div>
				</div>
			</div>
		{/each}
	</div>

	<!-- File count -->
	<div class="flex items-center gap-3 mt-4">
		<span class="px-3 py-1 bg-black text-white font-bold text-sm">
			{files.length} FILE{files.length !== 1 ? 'S' : ''}
		</span>
	</div>
{/if}

<!-- Viewer modal -->
{#if selectedFile}
	<FileViewerModal file={selectedFile} onClose={closeModal} />
{/if}
