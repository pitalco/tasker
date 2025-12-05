<script lang="ts">
	import { getFileCategory, decodeBase64 } from '$lib/services/filesService';
	import type { FileContentResponse } from '$lib/types/file';
	import TextViewer from './viewers/TextViewer.svelte';
	import MarkdownViewer from './viewers/MarkdownViewer.svelte';
	import ImageViewer from './viewers/ImageViewer.svelte';
	import CsvViewer from './viewers/CsvViewer.svelte';
	import ExcelViewer from './viewers/ExcelViewer.svelte';
	import PdfViewer from './viewers/PdfViewer.svelte';
	import DocViewer from './viewers/DocViewer.svelte';
	import FallbackViewer from './viewers/FallbackViewer.svelte';

	let {
		file,
		onDownload
	}: {
		file: FileContentResponse;
		onDownload: () => void;
	} = $props();

	const category = $derived(getFileCategory(file.mime_type, file.file_name));
	const textContent = $derived(
		category === 'text' || category === 'code' || category === 'markdown' || category === 'csv'
			? decodeBase64(file.content_base64)
			: ''
	);
</script>

{#if category === 'text' || category === 'code'}
	<TextViewer content={textContent} fileName={file.file_name} mimeType={file.mime_type} />
{:else if category === 'markdown'}
	<MarkdownViewer content={textContent} />
{:else if category === 'image'}
	<ImageViewer contentBase64={file.content_base64} mimeType={file.mime_type} />
{:else if category === 'csv'}
	<CsvViewer content={textContent} />
{:else if category === 'excel'}
	<ExcelViewer contentBase64={file.content_base64} />
{:else if category === 'pdf'}
	<PdfViewer contentBase64={file.content_base64} />
{:else if category === 'document'}
	<DocViewer contentBase64={file.content_base64} />
{:else}
	<FallbackViewer
		fileName={file.file_name}
		mimeType={file.mime_type}
		fileSize={file.file_size}
		{onDownload}
	/>
{/if}
