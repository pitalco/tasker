<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
	import { listen } from '@tauri-apps/api/event';
	import BrowserTabs, { type Tab } from './BrowserTabs.svelte';
	import { type RecordingEvent } from '$lib/services/webviewService';

	interface Props {
		onStep?: (event: RecordingEvent) => void;
		recording?: boolean;
		initialUrl?: string;
	}

	let { onStep, recording = false, initialUrl = 'https://google.com' }: Props = $props();

	let tabs: Tab[] = $state([]);
	let activeTab = $state('');
	let browserWindow: WebviewWindow | null = null;
	let unlisten: (() => void) | null = null;

	async function createTab(url: string = 'https://google.com') {
		const label = `browser-${Date.now()}`;
		const tab: Tab = { id: label, label, url, title: 'Tasker Browser', loading: true };
		tabs = [...tabs, tab];
		activeTab = label;

		try {
			// Close existing browser window if any
			if (browserWindow) {
				try {
					await browserWindow.close();
				} catch (e) {
					// Ignore close errors
				}
				browserWindow = null;
			}

			// Create a separate WebviewWindow (works reliably on Linux)
			browserWindow = new WebviewWindow(label, {
				url,
				title: 'Tasker Browser - Recording',
				width: 1200,
				height: 800,
				center: true,
				decorations: true,
				resizable: true,
				focus: true,
			});

			// Listen for window events
			browserWindow.once('tauri://created', () => {
				console.log('Browser window created:', label);
				tabs = tabs.map((t) => (t.id === label ? { ...t, loading: false } : t));
			});

			browserWindow.once('tauri://error', (e) => {
				console.error('Browser window error:', e);
				tabs = tabs.filter((t) => t.id !== label);
				browserWindow = null;
			});

			browserWindow.once('tauri://close-requested', () => {
				console.log('Browser window close requested');
				tabs = tabs.filter((t) => t.id !== label);
				browserWindow = null;
				activeTab = '';
			});

		} catch (e) {
			console.error('Failed to create browser window:', e);
			tabs = tabs.filter((t) => t.id !== label);
			browserWindow = null;
		}
	}

	async function closeTab(id: string) {
		if (browserWindow && activeTab === id) {
			try {
				await browserWindow.close();
			} catch (e) {
				console.error('Failed to close browser window:', e);
			}
			browserWindow = null;
		}
		tabs = tabs.filter((t) => t.id !== id);
		activeTab = tabs.length > 0 ? tabs[tabs.length - 1].id : '';
	}

	async function selectTab(id: string) {
		activeTab = id;
		if (browserWindow) {
			try {
				await browserWindow.setFocus();
			} catch (e) {
				// Ignore focus errors
			}
		}
	}

	async function handleNavigate(url: string) {
		if (browserWindow && activeTab) {
			// Close and reopen with new URL (simplest approach)
			const currentLabel = activeTab;
			tabs = tabs.map((t) => (t.id === currentLabel ? { ...t, url, loading: true } : t));

			try {
				await browserWindow.close();
			} catch (e) {
				// Ignore
			}

			browserWindow = new WebviewWindow(currentLabel + '-nav', {
				url,
				title: 'Tasker Browser - Recording',
				width: 1200,
				height: 800,
				center: true,
				decorations: true,
				resizable: true,
				focus: true,
			});

			browserWindow.once('tauri://created', () => {
				tabs = tabs.map((t) => (t.id === currentLabel ? { ...t, loading: false } : t));
			});
		}
	}

	async function handleBack() {
		// Not easily supported with WebviewWindow, would need JS eval
		console.log('Back not supported in separate window mode');
	}

	async function handleForward() {
		console.log('Forward not supported in separate window mode');
	}

	async function handleRefresh() {
		if (browserWindow) {
			// Reopen same URL
			const currentTab = tabs.find((t) => t.id === activeTab);
			if (currentTab) {
				handleNavigate(currentTab.url);
			}
		}
	}

	function handleRecordingEvent(event: RecordingEvent) {
		if (event.actionType === 'page_loaded' && event.data.title) {
			tabs = tabs.map((t) => (t.id === activeTab ? { ...t, title: event.data.title! } : t));
		}
		if (event.actionType === 'navigate' && event.data.url) {
			tabs = tabs.map((t) => (t.id === activeTab ? { ...t, url: event.data.url } : t));
		}
		if (onStep) {
			onStep(event);
		}
	}

	onMount(async () => {
		console.log('EmbeddedBrowser mounting...');

		// Listen for recording events
		unlisten = (await listen<RecordingEvent>('recording_event', (event) => {
			console.log('Recording event:', event.payload);
			handleRecordingEvent(event.payload);
		})) as unknown as () => void;

		// Create initial browser window
		if (initialUrl) {
			await createTab(initialUrl);
		}
	});

	onDestroy(async () => {
		if (unlisten) {
			unlisten();
		}
		// Close browser window on destroy
		if (browserWindow) {
			try {
				await browserWindow.close();
			} catch (e) {
				// Ignore cleanup errors
			}
		}
	});
</script>

<div class="embedded-browser">
	<BrowserTabs
		bind:tabs
		bind:activeTab
		on:new={() => createTab()}
		on:close={(e) => closeTab(e.detail)}
		on:select={(e) => selectTab(e.detail)}
		on:navigate={(e) => handleNavigate(e.detail)}
		on:back={handleBack}
		on:forward={handleForward}
		on:refresh={handleRefresh}
	/>

	<div class="browser-viewport">
		{#if tabs.length === 0}
			<div class="placeholder">
				<p>Click + to open a browser window</p>
			</div>
		{:else}
			<div class="browser-info">
				<p>Browser window is open separately.</p>
				<p>Recording actions in the browser window...</p>
				{#if browserWindow}
					<button class="focus-btn" onclick={() => browserWindow?.setFocus()}>
						Focus Browser Window
					</button>
				{/if}
			</div>
		{/if}
	</div>

	{#if recording}
		<div class="recording-indicator">
			<span class="dot"></span>
			Recording
		</div>
	{/if}
</div>

<style>
	.embedded-browser {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--surface-1, #1a1a1a);
		border-radius: 8px;
		overflow: hidden;
		position: relative;
	}

	.browser-viewport {
		flex: 1;
		position: relative;
		background: var(--surface-2, #2a2a2a);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.placeholder, .browser-info {
		text-align: center;
		color: var(--text-muted, #666);
	}

	.browser-info p {
		margin: 8px 0;
	}

	.focus-btn {
		margin-top: 16px;
		padding: 10px 20px;
		background: var(--primary, #4a9eff);
		color: white;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		font-size: 14px;
	}

	.focus-btn:hover {
		background: var(--primary-hover, #3a8eef);
	}

	.recording-indicator {
		position: absolute;
		top: 50px;
		right: 12px;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		background: rgba(239, 68, 68, 0.9);
		color: white;
		border-radius: 4px;
		font-size: 12px;
		font-weight: 500;
		z-index: 100;
	}

	.recording-indicator .dot {
		width: 8px;
		height: 8px;
		background: white;
		border-radius: 50%;
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>
