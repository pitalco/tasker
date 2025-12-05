<script lang="ts">
	let { contentBase64, mimeType }: { contentBase64: string; mimeType: string } = $props();
	let zoom = $state(1);

	const dataUrl = $derived(`data:${mimeType};base64,${contentBase64}`);

	function zoomIn() {
		zoom = Math.min(3, zoom + 0.25);
	}

	function zoomOut() {
		zoom = Math.max(0.25, zoom - 0.25);
	}

	function resetZoom() {
		zoom = 1;
	}
</script>

<div class="flex flex-col items-center gap-4">
	<!-- Zoom controls -->
	<div class="flex gap-2 items-center">
		<button
			onclick={zoomOut}
			class="px-3 py-2 border-3 border-black bg-white font-bold hover:-translate-y-0.5 transition-transform"
			style="box-shadow: 2px 2px 0 0 #000;"
		>
			-
		</button>
		<button
			onclick={resetZoom}
			class="px-4 py-2 border-3 border-black bg-white font-bold min-w-[80px]"
			style="box-shadow: 2px 2px 0 0 #000;"
		>
			{Math.round(zoom * 100)}%
		</button>
		<button
			onclick={zoomIn}
			class="px-3 py-2 border-3 border-black bg-white font-bold hover:-translate-y-0.5 transition-transform"
			style="box-shadow: 2px 2px 0 0 #000;"
		>
			+
		</button>
	</div>

	<!-- Image container -->
	<div class="border-3 border-black overflow-auto max-h-[60vh] max-w-full bg-gray-100 p-4">
		<img
			src={dataUrl}
			alt="File preview"
			style="transform: scale({zoom}); transform-origin: top left;"
			class="max-w-none"
		/>
	</div>
</div>
