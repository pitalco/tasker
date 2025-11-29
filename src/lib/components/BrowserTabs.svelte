<script module lang="ts">
	export interface Tab {
		id: string;
		label: string;
		url: string;
		title: string;
		loading: boolean;
	}
</script>

<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	let { tabs = $bindable<Tab[]>([]), activeTab = $bindable('') } = $props();
	const dispatch = createEventDispatcher<{
		select: string;
		close: string;
		new: void;
		navigate: string;
		refresh: void;
		back: void;
		forward: void;
	}>();

	let urlInput = $state('');

	$effect(() => {
		const currentTab = tabs.find((t) => t.id === activeTab);
		if (currentTab) {
			urlInput = currentTab.url;
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			let url = urlInput.trim();
			if (url && !url.startsWith('http://') && !url.startsWith('https://')) {
				url = 'https://' + url;
			}
			dispatch('navigate', url);
		}
	}
</script>

<div class="browser-chrome">
	<div class="tab-bar">
		{#each tabs as tab (tab.id)}
			<div
				class="tab"
				class:active={tab.id === activeTab}
				onclick={() => dispatch('select', tab.id)}
				onkeydown={(e) => e.key === 'Enter' && dispatch('select', tab.id)}
				role="tab"
				tabindex="0"
			>
				{#if tab.loading}
					<span class="spinner"></span>
				{/if}
				<span class="title">{tab.title || 'New Tab'}</span>
				<button
					class="close"
					onclick={(e) => {
						e.stopPropagation();
						dispatch('close', tab.id);
					}}
					aria-label="Close tab"
				>x</button>
			</div>
		{/each}
		<button class="new-tab" onclick={() => dispatch('new')}>+</button>
	</div>

	<div class="address-bar">
		<button class="nav-btn" onclick={() => dispatch('back')} title="Back">
			<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
				<path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z" />
			</svg>
		</button>
		<button class="nav-btn" onclick={() => dispatch('forward')} title="Forward">
			<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
				<path d="M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8-8-8z" />
			</svg>
		</button>
		<button class="nav-btn" onclick={() => dispatch('refresh')} title="Reload">
			<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
				<path
					d="M17.65 6.35A7.958 7.958 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
				/>
			</svg>
		</button>
		<input
			type="text"
			bind:value={urlInput}
			onkeydown={handleKeydown}
			placeholder="Enter URL..."
			class="url-input"
		/>
	</div>
</div>

<style>
	.browser-chrome {
		display: flex;
		flex-direction: column;
		background: var(--surface-2, #2a2a2a);
		border-bottom: 1px solid var(--border, #333);
	}

	.tab-bar {
		display: flex;
		align-items: center;
		padding: 4px 8px 0;
		gap: 2px;
		height: 36px;
		background: var(--surface-1, #1a1a1a);
	}

	.tab {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		background: var(--surface-2, #2a2a2a);
		border: none;
		border-radius: 8px 8px 0 0;
		color: var(--text-muted, #888);
		cursor: pointer;
		max-width: 200px;
		min-width: 100px;
		font-size: 12px;
		transition: background 0.15s;
	}

	.tab:hover {
		background: var(--surface-3, #333);
	}

	.tab.active {
		background: var(--surface-3, #3a3a3a);
		color: var(--text, #fff);
	}

	.tab .title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}

	.tab .close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: none;
		background: transparent;
		color: var(--text-muted, #888);
		cursor: pointer;
		border-radius: 4px;
		font-size: 12px;
		padding: 0;
	}

	.tab .close:hover {
		background: var(--surface-4, #444);
		color: var(--text, #fff);
	}

	.spinner {
		width: 12px;
		height: 12px;
		border: 2px solid var(--text-muted, #888);
		border-top-color: transparent;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.new-tab {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		background: transparent;
		color: var(--text-muted, #888);
		cursor: pointer;
		border-radius: 4px;
		font-size: 16px;
	}

	.new-tab:hover {
		background: var(--surface-3, #333);
		color: var(--text, #fff);
	}

	.address-bar {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 6px 8px;
	}

	.nav-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		background: transparent;
		color: var(--text-muted, #888);
		cursor: pointer;
		border-radius: 4px;
	}

	.nav-btn:hover {
		background: var(--surface-3, #333);
		color: var(--text, #fff);
	}

	.url-input {
		flex: 1;
		padding: 6px 12px;
		border: none;
		border-radius: 16px;
		background: var(--surface-1, #1a1a1a);
		color: var(--text, #fff);
		font-size: 13px;
	}

	.url-input:focus {
		outline: 1px solid var(--primary, #4a9eff);
	}

	.url-input::placeholder {
		color: var(--text-muted, #666);
	}
</style>
