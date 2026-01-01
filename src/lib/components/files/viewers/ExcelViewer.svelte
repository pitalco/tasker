<script lang="ts">
	import { onMount } from 'svelte';
	import * as XLSX from 'xlsx';
	import DOMPurify from 'dompurify';
	import { decodeBase64ToBytes } from '$lib/services/filesService';

	let { contentBase64 }: { contentBase64: string } = $props();

	let sheets = $state<string[]>([]);
	let activeSheet = $state(0);
	let tableHtml = $state('');
	let workbook: XLSX.WorkBook | null = null;

	onMount(() => {
		try {
			const bytes = decodeBase64ToBytes(contentBase64);
			workbook = XLSX.read(bytes, { type: 'array' });
			sheets = workbook.SheetNames;

			if (sheets.length > 0) {
				renderSheet(0);
			}
		} catch (e) {
			console.error('Failed to parse Excel file:', e);
		}
	});

	function renderSheet(index: number) {
		if (!workbook) return;
		activeSheet = index;
		const sheet = workbook.Sheets[sheets[index]];
		// SECURITY: Sanitize HTML to prevent XSS attacks
		tableHtml = DOMPurify.sanitize(XLSX.utils.sheet_to_html(sheet, { editable: false }), { USE_PROFILES: { html: true } });
	}
</script>

<div class="space-y-4">
	<!-- Sheet tabs -->
	{#if sheets.length > 1}
		<div class="flex gap-2 flex-wrap">
			{#each sheets as sheet, i}
				<button
					onclick={() => renderSheet(i)}
					class="px-4 py-2 border-3 border-black font-bold transition-all {i === activeSheet
						? 'bg-black text-white'
						: 'bg-white hover:-translate-y-0.5'}"
					style="box-shadow: {i === activeSheet ? '0 0 0 0 #000' : '2px 2px 0 0 #000'};"
				>
					{sheet}
				</button>
			{/each}
		</div>
	{/if}

	<!-- Table container -->
	<div class="border-3 border-black overflow-auto max-h-[60vh] excel-table">
		{@html tableHtml}
	</div>
</div>

{#if sheets.length === 0}
	<p class="text-center text-black/60 font-medium py-8">Failed to load Excel file</p>
{/if}

<style>
	.excel-table :global(table) {
		width: 100%;
		border-collapse: collapse;
	}
	.excel-table :global(th),
	.excel-table :global(td) {
		padding: 0.5rem;
		border: 1px solid #000;
		text-align: left;
	}
	.excel-table :global(th) {
		background-color: #ffde59;
		font-weight: bold;
	}
	.excel-table :global(tr:nth-child(even)) {
		background-color: #f9fafb;
	}
</style>
