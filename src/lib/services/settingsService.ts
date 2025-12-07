import { invoke } from '@tauri-apps/api/core';

export interface ApiKeys {
	gemini?: string;
	openai?: string;
	anthropic?: string;
}

export interface LLMConfig {
	api_keys: ApiKeys;
	default_provider: string;
	default_model: string;
}

export interface AppSettings {
	llm_config: LLMConfig;
	default_max_steps: number;
}

// Available models per provider
export const PROVIDER_MODELS: Record<string, { id: string; name: string }[]> = {
	gemini: [
		{ id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash' },
		{ id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro' },
		{ id: 'gemini-3-pro-preview', name: 'Gemini 3 Pro' }
	],
	openai: [
		{ id: 'gpt-4o', name: 'GPT-4o' },
		{ id: 'gpt-4o-mini', name: 'GPT-4o Mini' }
	],
	anthropic: [
		{ id: 'claude-sonnet-4-5-20250929', name: 'Claude Sonnet 4.5' },
		{ id: 'claude-haiku-4-5-20251001', name: 'Claude Haiku 4.5' }
	],
	'tasker-fast': [{ id: 'tasker-fast', name: 'Tasker Fast (Vision)' }]
};

export interface ProviderInfo {
	id: string;
	name: string;
	requiresSubscription?: boolean;
}

export const PROVIDERS: ProviderInfo[] = [
	{ id: 'gemini', name: 'Google Gemini' },
	{ id: 'openai', name: 'OpenAI' },
	{ id: 'anthropic', name: 'Anthropic' },
	{ id: 'tasker-fast', name: 'Tasker Fast', requiresSubscription: true }
];

export async function getSettings(): Promise<AppSettings> {
	return invoke<AppSettings>('get_settings');
}

export async function updateSettings(options: {
	api_keys?: ApiKeys;
	default_provider?: string;
	default_model?: string;
	default_max_steps?: number;
}): Promise<AppSettings> {
	return invoke<AppSettings>('update_settings', {
		apiKeys: options.api_keys,
		defaultProvider: options.default_provider,
		defaultModel: options.default_model,
		defaultMaxSteps: options.default_max_steps
	});
}

// Helper to get available models (from providers with keys configured OR subscription)
export function getAvailableModels(
	apiKeys: ApiKeys,
	hasSubscription: boolean = false
): { provider: string; model: string; name: string }[] {
	const available: { provider: string; model: string; name: string }[] = [];

	for (const provider of PROVIDERS) {
		// Check if provider requires subscription or API key
		let canAccess = false;

		if (provider.requiresSubscription) {
			// Subscription-based provider (like Tasker Fast)
			canAccess = hasSubscription;
		} else {
			// API key-based provider
			const hasKey = (apiKeys[provider.id as keyof ApiKeys] ?? '').length > 0;
			canAccess = hasKey;
		}

		if (canAccess) {
			for (const model of PROVIDER_MODELS[provider.id] || []) {
				available.push({
					provider: provider.id,
					model: model.id,
					name: `${provider.name} - ${model.name}`
				});
			}
		}
	}

	return available;
}
