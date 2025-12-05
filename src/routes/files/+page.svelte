<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { listFiles, deleteFile as deleteFileApi, downloadFile, formatFileSize, getFileCategory, getCategoryColorClass } from '$lib/services/filesService';
	import type { TaskerFile, FileCategory } from '$lib/types/file';
	import FileViewerModal from '$lib/components/files/FileViewerModal.svelte';

	let files = $state<TaskerFile[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let total = $state(0);

	// Filters
	let searchQuery = $state('');
	let categoryFilter = $state<FileCategory | 'all'>('all');
	let showDeleteConfirm = $state<string | null>(null);
	let selectedFile = $state<TaskerFile | null>(null);

	const categoryFilters: { value: FileCategory | 'all'; label: string }[] = [
		{ value: 'all', label: 'ALL' },
		{ value: 'text', label: 'TEXT' },
		{ value: 'code', label: 'CODE' },
		{ value: 'markdown', label: 'MARKDOWN' },
		{ value: 'pdf', label: 'PDF' },
		{ value: 'image', label: 'IMAGES' },
		{ value: 'csv', label: 'CSV' },
		{ value: 'excel', label: 'EXCEL' },
		{ value: 'document', label: 'DOCS' }
	];

	const filteredFiles = $derived(() => {
		let result = files;

		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			result = result.filter(
				(f) =>
					f.file_name.toLowerCase().includes(query) ||
					f.file_path.toLowerCase().includes(query) ||
					f.run_name?.toLowerCase().includes(query) ||
					f.workflow_name?.toLowerCase().includes(query)
			);
		}

		if (categoryFilter !== 'all') {
			result = result.filter((f) => getFileCategory(f.mime_type, f.file_name) === categoryFilter);
		}

		return result;
	});

	onMount(() => {
		loadFiles();
	});

	async function loadFiles() {
		loading = true;
		error = null;

		try {
			const response = await listFiles();
			files = response.files;
			total = response.total;
		} catch (e) {
			console.error('Failed to load files:', e);
			error = 'Failed to load files';
		} finally {
			loading = false;
		}
	}

	async function handleDelete(fileId: string) {
		try {
			await deleteFileApi(fileId);
			files = files.filter((f) => f.id !== fileId);
			total = Math.max(0, total - 1);
			showDeleteConfirm = null;
		} catch (e) {
			console.error('Failed to delete file:', e);
			error = 'Failed to delete file';
		}
	}

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

	function confirmDelete(fileId: string, event: Event) {
		event.stopPropagation();
		showDeleteConfirm = fileId;
	}

	function cancelDelete(event: Event) {
		event.stopPropagation();
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

	function goToRun(runId: string, event: Event) {
		event.stopPropagation();
		goto(`/runs/${runId}`);
	}
</script>

<svelte:head>
	<title>Files | Tasker</title>
</svelte:head>

<div class="space-y-8">
	<!-- Header -->
	<div class="flex items-end justify-between">
		<div>
			<h1 class="text-4xl font-bold text-black tracking-tight">Files</h1>
			<p class="text-lg text-black/60 font-medium mt-1">Files created by your workflows</p>
		</div>
	</div>

	<!-- Search and filters -->
	<div class="flex flex-col sm:flex-row gap-4">
		<div class="flex-1">
			<input
				type="text"
				bind:value={searchQuery}
				placeholder="Search files..."
				class="input-brutal"
			/>
		</div>
	</div>

	<!-- Category filters -->
	<div class="flex flex-wrap gap-2">
		{#each categoryFilters as filter}
			<button
				onclick={() => (categoryFilter = filter.value)}
				class="px-4 py-2 border-3 border-black font-bold text-sm transition-all cursor-pointer {categoryFilter === filter.value
					? 'bg-black text-white'
					: 'bg-white text-black hover:-translate-y-0.5'}"
				style="box-shadow: {categoryFilter === filter.value ? '0 0 0 0 #000' : '2px 2px 0 0 #000'};"
			>
				{filter.label}
			</button>
		{/each}
	</div>

	<!-- Error message -->
	{#if error}
		<div class="card-brutal bg-brutal-magenta p-4 flex items-center justify-between">
			<span class="font-bold">{error}</span>
			<button onclick={() => (error = null)} class="font-bold underline">
				DISMISS
			</button>
		</div>
	{/if}

	<!-- File count -->
	{#if !loading && filteredFiles().length > 0}
		<div class="flex items-center gap-3">
			<span class="px-3 py-1 bg-black text-white font-bold text-sm">
				{filteredFiles().length} FILE{filteredFiles().length !== 1 ? 'S' : ''}
			</span>
			{#if searchQuery.trim()}
				<span class="text-sm font-medium text-black/60">
					matching "{searchQuery}"
				</span>
			{:else if categoryFilter !== 'all'}
				<span class="text-sm font-medium text-black/60">
					filtered by {categoryFilter}
				</span>
			{/if}
		</div>
	{/if}

	<!-- Files list -->
	{#if loading}
		<div class="flex items-center justify-center py-16">
			<div class="flex flex-col items-center gap-4">
				<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
				<span class="font-bold text-black">LOADING...</span>
			</div>
		</div>
	{:else if filteredFiles().length === 0}
		<div class="card-brutal p-12 text-center">
			{#if searchQuery.trim() || categoryFilter !== 'all'}
				<div class="w-16 h-16 mx-auto bg-brutal-orange border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-8 h-8" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2">NO RESULTS</h3>
				<p class="text-black/60 font-medium mb-6">
					No files match your search criteria
				</p>
				<button
					onclick={() => { searchQuery = ''; categoryFilter = 'all'; }}
					class="btn-brutal bg-white text-black"
				>
					CLEAR FILTERS
				</button>
			{:else}
				<div class="w-20 h-20 mx-auto bg-brutal-purple border-3 border-black flex items-center justify-center mb-6" style="box-shadow: 4px 4px 0 0 #000;">
					<svg class="w-10 h-10" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
					</svg>
				</div>
				<h3 class="text-2xl font-bold text-black mb-2">NO FILES YET</h3>
				<p class="text-black/60 font-medium mb-8 max-w-md mx-auto">
					Files created by your workflows will appear here. Run a workflow that writes files to get started.
				</p>
				<a href="/" class="btn-brutal bg-brutal-purple text-black inline-flex items-center gap-2">
					<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
						<path d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
					</svg>
					VIEW WORKFLOWS
				</a>
			{/if}
		</div>
	{:else}
		<div class="space-y-4">
			{#each filteredFiles() as file (file.id)}
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
							{#if file.run_name || file.workflow_name}
								<div class="flex items-center gap-2 mt-2">
									{#if file.workflow_name}
										<span class="text-xs text-black/50">Workflow: {file.workflow_name}</span>
									{/if}
									{#if file.run_name && file.run_id}
										<button
											onclick={(e) => goToRun(file.run_id, e)}
											class="text-xs text-black/50 hover:text-black underline cursor-pointer"
										>
											Run: {file.run_name}
										</button>
									{/if}
								</div>
							{/if}
						</div>
						<div class="flex items-center gap-2">
							<button
								onclick={(e) => handleDownload(file, e)}
								class="px-3 py-2 bg-brutal-yellow border-2 border-black font-bold text-sm hover:-translate-y-0.5 transition-transform cursor-pointer"
								style="box-shadow: 2px 2px 0 0 #000;"
							>
								DOWNLOAD
							</button>
							{#if showDeleteConfirm === file.id}
								<div class="flex items-center gap-2">
									<button
										onclick={(e) => { e.stopPropagation(); handleDelete(file.id); }}
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
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Viewer modal -->
{#if selectedFile}
	<FileViewerModal file={selectedFile} onClose={() => (selectedFile = null)} />
{/if}
