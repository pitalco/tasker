<script lang="ts">
	import { formatFileSize } from '$lib/services/filesService';

	let {
		fileName,
		mimeType,
		fileSize,
		onDownload
	}: {
		fileName: string;
		mimeType: string;
		fileSize: number;
		onDownload: () => void;
	} = $props();
</script>

<div class="border-3 border-black bg-white p-8 text-center">
	<div class="mb-6">
		<svg
			class="w-16 h-16 mx-auto text-black/40"
			fill="none"
			stroke="currentColor"
			viewBox="0 0 24 24"
		>
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
			/>
		</svg>
	</div>

	<h3 class="text-xl font-bold mb-2">{fileName}</h3>

	<div class="text-black/60 mb-6 space-y-1">
		<p>Type: {mimeType}</p>
		<p>Size: {formatFileSize(fileSize)}</p>
	</div>

	<p class="text-black/60 mb-6">Preview not available for this file type.</p>

	<button
		onclick={onDownload}
		class="px-6 py-3 border-3 border-black bg-brutal-yellow font-bold hover:-translate-y-0.5 transition-transform"
		style="box-shadow: 3px 3px 0 0 #000;"
	>
		Download File
	</button>
</div>
