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
	vllm_base_url?: string;
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
		{ id: 'gemini-3-flash-preview', name: 'Gemini 3 Flash' },
		{ id: 'gemini-3-pro-preview', name: 'Gemini 3 Pro' }
	],
	openai: [
		{ id: 'gpt-4o', name: 'GPT-4o' },
		{ id: 'gpt-4o-mini', name: 'GPT-4o Mini' }
	],
	anthropic: [
		{ id: 'claude-opus-4-5', name: 'Claude Opus 4.5' },
		{ id: 'claude-sonnet-4-5', name: 'Claude Sonnet 4.5' },
		{ id: 'claude-haiku-4-5', name: 'Claude Haiku 4.5' }
	],
	ollama: [
		{ id: 'qwen35-4b-mind2web', name: 'Qwen3.5 4B Mind2Web (local)' }
	],
	vllm: [
		{ id: 'qwen35-4b-mind2web', name: 'Qwen3.5 4B Mind2Web (vLLM)' }
	]
};

export interface ProviderInfo {
	id: string;
	name: string;
	/** If true, no API key is required */
	local?: boolean;
}

export const PROVIDERS: ProviderInfo[] = [
	{ id: 'gemini', name: 'Google Gemini' },
	{ id: 'openai', name: 'OpenAI' },
	{ id: 'anthropic', name: 'Anthropic' },
	{ id: 'ollama', name: 'Ollama (Local)', local: true },
	{ id: 'vllm', name: 'vLLM (Local)', local: true }
];

export async function getSettings(): Promise<AppSettings> {
	return invoke<AppSettings>('get_settings');
}

export async function updateSettings(options: {
	api_keys?: ApiKeys;
	default_provider?: string;
	default_model?: string;
	default_max_steps?: number;
	vllm_base_url?: string;
}): Promise<AppSettings> {
	return invoke<AppSettings>('update_settings', {
		apiKeys: options.api_keys,
		defaultProvider: options.default_provider,
		defaultModel: options.default_model,
		defaultMaxSteps: options.default_max_steps,
		vllmBaseUrl: options.vllm_base_url
	});
}

// Helper to get available models (from providers with keys configured, or local providers)
export function getAvailableModels(apiKeys: ApiKeys): { provider: string; model: string; name: string }[] {
	const available: { provider: string; model: string; name: string }[] = [];

	for (const provider of PROVIDERS) {
		const hasKey = provider.local || (apiKeys[provider.id as keyof ApiKeys] ?? '').length > 0;

		if (hasKey) {
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
