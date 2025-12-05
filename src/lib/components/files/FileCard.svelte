<script lang="ts">
	import { formatFileSize, getFileCategory, getCategoryColorClass } from '$lib/services/filesService';
	import type { TaskerFile } from '$lib/types/file';

	let {
		file,
		onView,
		onDownload,
		onDelete
	}: {
		file: TaskerFile;
		onView: () => void;
		onDownload: () => void;
		onDelete?: () => void;
	} = $props();

	const category = $derived(getFileCategory(file.mime_type, file.file_name));
	const colorClass = $derived(getCategoryColorClass(category));

	const categoryIcons: Record<string, string> = {
		text: '📄',
		code: '💻',
		markdown: '📝',
		pdf: '📕',
		image: '🖼️',
		csv: '📊',
		excel: '📗',
		document: '📘',
		other: '📎'
	};

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}
</script>

<div
	class="border-3 border-black bg-white p-4 hover:-translate-y-1 transition-transform cursor-pointer"
	style="box-shadow: 4px 4px 0 0 #000;"
	onclick={onView}
	onkeypress={(e) => e.key === 'Enter' && onView()}
	role="button"
	tabindex="0"
>
	<div class="flex items-start gap-3">
		<!-- Icon -->
		<div class="text-2xl">{categoryIcons[category] || '📎'}</div>

		<!-- Info -->
		<div class="flex-1 min-w-0">
			<h3 class="font-bold truncate" title={file.file_name}>{file.file_name}</h3>
			<p class="text-sm text-black/60 truncate" title={file.file_path}>{file.file_path}</p>

			<div class="flex flex-wrap gap-2 mt-2">
				<span class="text-xs px-2 py-1 border-2 border-black {colorClass} font-medium">
					{category.toUpperCase()}
				</span>
				<span class="text-xs text-black/60">{formatFileSize(file.file_size)}</span>
			</div>

			{#if file.run_name || file.workflow_name}
				<div class="mt-2 text-xs text-black/50">
					{#if file.workflow_name}
						<span>Workflow: {file.workflow_name}</span>
					{/if}
					{#if file.run_name}
						<span class="ml-2">Run: {file.run_name}</span>
					{/if}
				</div>
			{/if}

			<p class="text-xs text-black/40 mt-1">{formatDate(file.created_at)}</p>
		</div>
	</div>

	<!-- Actions -->
	<div class="flex gap-2 mt-3 pt-3 border-t-2 border-black/10" onclick={(e) => e.stopPropagation()}>
		<button
			onclick={onView}
			class="flex-1 px-3 py-2 border-2 border-black bg-white text-sm font-bold hover:bg-gray-50 transition-colors"
		>
			View
		</button>
		<button
			onclick={onDownload}
			class="flex-1 px-3 py-2 border-2 border-black bg-brutal-yellow text-sm font-bold hover:bg-yellow-400 transition-colors"
		>
			Download
		</button>
		{#if onDelete}
			<button
				onclick={onDelete}
				class="px-3 py-2 border-2 border-black bg-red-100 text-sm font-bold hover:bg-red-200 transition-colors"
				title="Delete file"
			>
				🗑️
			</button>
		{/if}
	</div>
</div>
