<script lang="ts">
	import { onMount } from 'svelte';
	import hljs from 'highlight.js';
	import 'highlight.js/styles/github-dark.css';
	import { getHighlightLanguage } from '$lib/services/filesService';

	let { content, fileName, mimeType }: { content: string; fileName: string; mimeType: string } =
		$props();

	let codeElement: HTMLElement;
	const language = $derived(getHighlightLanguage(fileName, mimeType));

	onMount(() => {
		if (codeElement) {
			hljs.highlightElement(codeElement);
		}
	});
</script>

<div class="border-3 border-black bg-gray-900 overflow-auto max-h-[70vh]">
	<pre class="p-4 m-0"><code bind:this={codeElement} class="language-{language} text-sm">{content}</code></pre>
</div>
