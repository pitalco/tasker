<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import { createWorkflowState } from '$lib/stores/workflow.svelte';
	import { createRunsState } from '$lib/stores/runs.svelte';
	import { startSidecar, isSidecarRunning } from '$lib/services/sidecarService';
	import { page } from '$app/stores';
	import taskerIcon from '$lib/assets/tasker-icon.png';
	import taskerLogoFull from '$lib/assets/tasker-logo-full.png';

	let { children } = $props();

	// Initialize global state
	createWorkflowState();
	createRunsState();

	// Sidecar loading state
	let sidecarReady = $state(false);
	let loadingMessage = $state('Waking up the robots...');
	let loadingProgress = $state(0);

	const loadingMessages = [
		'Waking up the robots...',
		'Brewing digital coffee...',
		'Teaching browsers new tricks...',
		'Stretching automation muscles...',
		'Polishing the chrome...',
		'Initializing awesomeness...',
		'Almost there...'
	];

	onMount(async () => {
		// Cycle through fun messages
		let messageIndex = 0;
		const messageInterval = setInterval(() => {
			messageIndex = (messageIndex + 1) % loadingMessages.length;
			loadingMessage = loadingMessages[messageIndex];
		}, 800);

		// Animate progress bar
		const progressInterval = setInterval(() => {
			if (loadingProgress < 90) {
				loadingProgress += Math.random() * 15;
			}
		}, 200);

		try {
			// Check if already running
			const running = await isSidecarRunning();
			if (!running) {
				await startSidecar();
				// Give it a moment to fully initialize
				await new Promise((resolve) => setTimeout(resolve, 1000));
			}
			loadingProgress = 100;
			await new Promise((resolve) => setTimeout(resolve, 300));
			sidecarReady = true;
		} catch (error) {
			console.error('Failed to start sidecar:', error);
			loadingMessage = 'Failed to start engine! Please restart the app.';
		} finally {
			clearInterval(messageInterval);
			clearInterval(progressInterval);
		}
	});

	const navItems = [
		{ href: '/', label: 'Workflows', icon: 'stack' },
		{ href: '/runs', label: 'Runs', icon: 'bolt' },
		{ href: '/files', label: 'Files', icon: 'folder' },
		{ href: '/record', label: 'Record', icon: 'record' },
		{ href: '/settings', label: 'Settings', icon: 'cog' }
	];
</script>

<svelte:head>
	<title>Tasker</title>
	<meta name="description" content="Browser automation recording and replay" />
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&display=swap"
		rel="stylesheet"
	/>
</svelte:head>

{#if !sidecarReady}
	<!-- Fun Loading Screen -->
	<div class="fixed inset-0 bg-brutal-bg flex items-center justify-center z-50">
		<div class="text-center">
			<!-- Animated Logo -->
			<div class="relative mb-8">
				<div class="w-32 h-32 mx-auto relative">
					<img src={taskerIcon} alt="Tasker" class="w-full h-full object-contain animate-bounce" />
				</div>
				<!-- Decorative dots -->
				<div class="absolute -top-2 -left-2 w-4 h-4 bg-brutal-magenta border-2 border-black animate-ping"></div>
				<div class="absolute -bottom-2 -right-2 w-4 h-4 bg-brutal-cyan border-2 border-black animate-ping" style="animation-delay: 0.3s;"></div>
			</div>

			<!-- Title -->
			<h1 class="text-5xl font-black text-black tracking-tight mb-2">TASKER</h1>
			<p class="text-lg font-bold text-black/60 uppercase tracking-widest mb-8">Automation Engine</p>

			<!-- Progress bar -->
			<div class="w-80 mx-auto mb-6">
				<div class="h-6 bg-white border-4 border-black relative" style="box-shadow: 4px 4px 0 0 #000;">
					<div
						class="h-full bg-brutal-cyan transition-all duration-200 ease-out"
						style="width: {Math.min(loadingProgress, 100)}%;"
					></div>
					<!-- Stripes overlay -->
					<div class="absolute inset-0 opacity-20" style="background: repeating-linear-gradient(45deg, transparent, transparent 10px, #000 10px, #000 12px);"></div>
				</div>
			</div>

			<!-- Loading message -->
			<div class="h-8">
				<p class="text-lg font-bold text-black animate-pulse">{loadingMessage}</p>
			</div>

			<!-- Fun robot ASCII art -->
			<div class="mt-8 font-mono text-xs text-black/40 leading-tight">
				<pre class="inline-block text-left">{`
    ╔═══╗
    ║ ◉ ◉ ║
    ║  ▽  ║
    ╚═╦═╦═╝
      ║ ║
    ══╩═╩══
				`}</pre>
			</div>
		</div>
	</div>
{:else}
<div class="min-h-screen bg-brutal-bg">
	<!-- Sidebar (fixed) -->
	<aside class="fixed top-0 left-0 w-56 h-screen bg-brutal-yellow border-r-4 border-black flex flex-col z-40">
		<!-- Logo -->
		<div class="px-3 py-4 border-b-4 border-black">
			<img src={taskerLogoFull} alt="Tasker" class="w-full h-10 object-contain" />
		</div>

		<!-- Navigation -->
		<nav class="flex-1 p-4 space-y-2">
			{#each navItems as item}
				{@const isActive = $page.url.pathname === item.href ||
					(item.href !== '/' && $page.url.pathname.startsWith(item.href))}
				<a
					href={item.href}
					class="flex items-center gap-3 px-4 py-3 font-bold text-black border-3 border-black transition-all
						{isActive
							? 'bg-black text-brutal-yellow brutal-shadow-sm'
							: 'bg-white hover:bg-brutal-cyan hover:-translate-x-0.5 hover:-translate-y-0.5 hover:brutal-shadow'}"
					style={isActive ? '' : 'box-shadow: 2px 2px 0 0 #000;'}
				>
					{#if item.icon === 'stack'}
						<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
							<path d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
						</svg>
					{:else if item.icon === 'bolt'}
						<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
							<path d="M13 10V3L4 14h7v7l9-11h-7z" />
						</svg>
					{:else if item.icon === 'record'}
						<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
							<circle cx="12" cy="12" r="8" />
						</svg>
					{:else if item.icon === 'folder'}
						<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
							<path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
						</svg>
					{:else if item.icon === 'cog'}
						<svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
							<path d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
							<circle cx="12" cy="12" r="3" />
						</svg>
					{/if}
					{item.label}
				</a>
			{/each}
		</nav>

		<!-- Footer -->
		<div class="p-4 border-t-4 border-black">
			<div class="px-4 py-3 bg-white border-3 border-black text-xs font-bold text-center" style="box-shadow: 2px 2px 0 0 #000;">
				v0.1.0 — BETA
			</div>
		</div>
	</aside>

	<!-- Main content (offset by sidebar width) -->
	<main class="ml-56 min-h-screen p-8 overflow-auto">
		{@render children()}
	</main>
</div>
{/if}
