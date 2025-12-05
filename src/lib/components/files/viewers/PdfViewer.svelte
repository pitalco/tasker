<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { decodeBase64ToBytes } from '$lib/services/filesService';
	import * as pdfjsLib from 'pdfjs-dist';

	let { contentBase64 }: { contentBase64: string } = $props();

	let container: HTMLDivElement;
	let currentPage = $state(1);
	let totalPages = $state(0);
	let scale = $state(1.5);
	let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;
	let rendering = $state(false);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			// Set worker source
			pdfjsLib.GlobalWorkerOptions.workerSrc = `https://cdnjs.cloudflare.com/ajax/libs/pdf.js/${pdfjsLib.version}/pdf.worker.min.js`;

			const bytes = decodeBase64ToBytes(contentBase64);
			pdfDoc = await pdfjsLib.getDocument({ data: bytes }).promise;
			totalPages = pdfDoc.numPages;

			if (totalPages > 0) {
				await renderPage(1);
			}
		} catch (e) {
			console.error('Failed to load PDF:', e);
			error = 'Failed to load PDF file';
		}
	});

	onDestroy(() => {
		if (pdfDoc) {
			pdfDoc.destroy();
		}
	});

	async function renderPage(pageNum: number) {
		if (!pdfDoc || rendering) return;

		rendering = true;
		try {
			const page = await pdfDoc.getPage(pageNum);
			const viewport = page.getViewport({ scale });

			// Clear container and create new canvas
			container.innerHTML = '';
			const canvas = document.createElement('canvas');
			const context = canvas.getContext('2d')!;

			canvas.height = viewport.height;
			canvas.width = viewport.width;
			container.appendChild(canvas);

			await page.render({
				canvasContext: context,
				viewport: viewport,
				canvas: canvas
			}).promise;

			currentPage = pageNum;
		} catch (e) {
			console.error('Failed to render page:', e);
		} finally {
			rendering = false;
		}
	}

	function prevPage() {
		if (currentPage > 1) {
			renderPage(currentPage - 1);
		}
	}

	function nextPage() {
		if (currentPage < totalPages) {
			renderPage(currentPage + 1);
		}
	}

	function zoomIn() {
		scale = Math.min(3, scale + 0.25);
		renderPage(currentPage);
	}

	function zoomOut() {
		scale = Math.max(0.5, scale - 0.25);
		renderPage(currentPage);
	}
</script>

{#if error}
	<p class="text-center text-red-600 font-medium py-8">{error}</p>
{:else}
	<div class="flex flex-col items-center gap-4">
		<!-- Controls -->
		<div class="flex gap-4 items-center flex-wrap justify-center">
			<!-- Page navigation -->
			<div class="flex gap-2 items-center">
				<button
					onclick={prevPage}
					disabled={currentPage <= 1 || rendering}
					class="px-3 py-2 border-3 border-black bg-white font-bold transition-transform disabled:opacity-50 disabled:cursor-not-allowed hover:enabled:-translate-y-0.5"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					←
				</button>
				<span class="px-4 py-2 border-3 border-black bg-white font-bold min-w-[120px] text-center">
					{currentPage} / {totalPages}
				</span>
				<button
					onclick={nextPage}
					disabled={currentPage >= totalPages || rendering}
					class="px-3 py-2 border-3 border-black bg-white font-bold transition-transform disabled:opacity-50 disabled:cursor-not-allowed hover:enabled:-translate-y-0.5"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					→
				</button>
			</div>

			<!-- Zoom controls -->
			<div class="flex gap-2 items-center">
				<button
					onclick={zoomOut}
					disabled={rendering}
					class="px-3 py-2 border-3 border-black bg-white font-bold transition-transform disabled:opacity-50 hover:enabled:-translate-y-0.5"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					-
				</button>
				<span class="px-3 py-2 border-3 border-black bg-white font-bold min-w-[70px] text-center">
					{Math.round(scale * 100)}%
				</span>
				<button
					onclick={zoomIn}
					disabled={rendering}
					class="px-3 py-2 border-3 border-black bg-white font-bold transition-transform disabled:opacity-50 hover:enabled:-translate-y-0.5"
					style="box-shadow: 2px 2px 0 0 #000;"
				>
					+
				</button>
			</div>
		</div>

		<!-- PDF container -->
		<div
			bind:this={container}
			class="border-3 border-black overflow-auto max-h-[60vh] bg-gray-200 p-4 flex justify-center"
		>
			{#if rendering}
				<p class="text-black/60 font-medium py-8">Loading page...</p>
			{/if}
		</div>
	</div>
{/if}
