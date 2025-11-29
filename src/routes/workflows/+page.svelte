<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';

	// Redirect to root - workflows are now at /
	onMount(() => {
		goto('/', { replaceState: true });
	});
</script>

<div class="flex items-center justify-center py-16">
	<div class="flex flex-col items-center gap-4">
		<div class="w-12 h-12 border-4 border-black border-t-brutal-yellow animate-spin"></div>
		<span class="font-bold text-black">REDIRECTING...</span>
	</div>
</div>
