<script lang="ts">
	import { marked } from 'marked';

	let { content }: { content: string } = $props();
	const html = $derived(marked(content));
</script>

<div
	class="prose prose-lg max-w-none border-3 border-black bg-white p-6 overflow-auto max-h-[70vh]"
>
	{@html html}
</div>
