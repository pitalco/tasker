<script lang="ts">
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';

	let { content }: { content: string } = $props();
	// SECURITY: Sanitize HTML to prevent XSS attacks
	const html = $derived(DOMPurify.sanitize(marked(content) as string, { USE_PROFILES: { html: true } }));
</script>

<div
	class="prose prose-lg max-w-none border-3 border-black bg-white p-6 overflow-auto max-h-[70vh]"
>
	{@html html}
</div>
