<script lang="ts">
	import Papa from 'papaparse';

	let { content }: { content: string } = $props();

	const parsed = $derived(Papa.parse(content, { header: true }));
	const headers = $derived((parsed.meta.fields as string[]) || []);
	const rows = $derived(parsed.data as Record<string, string>[]);
</script>

<div class="border-3 border-black overflow-auto max-h-[70vh]">
	<table class="w-full border-collapse min-w-max">
		<thead class="sticky top-0">
			<tr class="bg-brutal-yellow border-b-3 border-black">
				{#each headers as header}
					<th class="p-3 text-left font-bold border-r-2 border-black last:border-r-0 whitespace-nowrap">
						{header}
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each rows as row, i}
				<tr class="{i % 2 === 0 ? 'bg-white' : 'bg-gray-50'} border-b border-black">
					{#each headers as header}
						<td class="p-3 border-r border-black last:border-r-0">
							{row[header] || ''}
						</td>
					{/each}
				</tr>
			{/each}
		</tbody>
	</table>
</div>

{#if rows.length === 0}
	<p class="text-center text-black/60 font-medium py-8">No data in CSV file</p>
{/if}
