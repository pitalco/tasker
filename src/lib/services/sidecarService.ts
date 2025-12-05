import { invoke } from '@tauri-apps/api/core';
import type { Workflow } from '$lib/types/workflow';

// Types
export interface RecordingResponse {
	session_id: string;
	status: string;
}

export interface RecordingStatus {
	session_id: string;
	status: string;
	step_count: number;
	current_step?: number;
	error?: string;
}

export interface ReplayResponse {
	session_id: string;
	status: string;
}

export interface ReplayStatus {
	session_id: string;
	status: string;
	step_count: number;
	current_step: number;
	error?: string;
}

export interface LLMProviders {
	providers: Record<string, string[]>;
}

export interface StartRecordingOptions {
	start_url?: string;
	headless?: boolean;
	viewport_width?: number;
	viewport_height?: number;
}

// AI agent is ALWAYS used - recorded workflow serves as hints
export interface StartReplayOptions {
	workflow: Workflow;
	llm_provider?: string;
	llm_model?: string;
	task_description?: string;
	variables?: Record<string, unknown>;
	iterations?: number;
	headless?: boolean;
	/** Optional condition - agent will NOT stop until this is met */
	stop_when?: string;
	/** Max steps override (undefined = use global default) */
	max_steps?: number;
}

// Sidecar management
export async function startSidecar(): Promise<boolean> {
	return invoke<boolean>('start_sidecar');
}

export async function stopSidecar(): Promise<boolean> {
	return invoke<boolean>('stop_sidecar');
}

export async function isSidecarRunning(): Promise<boolean> {
	return invoke<boolean>('is_sidecar_running');
}

export async function getSidecarUrls(): Promise<[string, string]> {
	return invoke<[string, string]>('get_sidecar_urls');
}

// Recording
export async function startRecording(options: StartRecordingOptions): Promise<RecordingResponse> {
	return invoke<RecordingResponse>('start_recording', { request: options });
}

export async function stopRecording(sessionId: string): Promise<{ name: string; task_description: string }> {
	return invoke<{ name: string; task_description: string }>('stop_recording', { sessionId });
}

export async function cancelRecording(sessionId: string): Promise<boolean> {
	return invoke<boolean>('cancel_recording', { sessionId });
}

export async function getRecordingStatus(sessionId: string): Promise<RecordingStatus> {
	return invoke<RecordingStatus>('get_recording_status', { sessionId });
}

// Replay
export async function getLLMProviders(): Promise<LLMProviders> {
	return invoke<LLMProviders>('get_llm_providers');
}

export async function startReplay(options: StartReplayOptions): Promise<ReplayResponse> {
	return invoke<ReplayResponse>('start_replay', { request: options });
}

export async function stopReplay(sessionId: string): Promise<boolean> {
	return invoke<boolean>('stop_replay', { sessionId });
}

export async function getReplayStatus(sessionId: string): Promise<ReplayStatus> {
	return invoke<ReplayStatus>('get_replay_status', { sessionId });
}

// WebSocket connection for real-time updates
export class SidecarWebSocket {
	private ws: WebSocket | null = null;
	private reconnectAttempts = 0;
	private maxReconnectAttempts = 5;
	private listeners: Map<string, Set<(data: unknown) => void>> = new Map();

	async connect(): Promise<void> {
		// Close existing connection first to prevent duplicates
		if (this.ws) {
			this.ws.onclose = null; // Prevent reconnect attempt
			this.ws.close();
			this.ws = null;
		}

		const [, wsUrl] = await getSidecarUrls();

		return new Promise((resolve, reject) => {
			this.ws = new WebSocket(wsUrl);

			this.ws.onopen = () => {
				this.reconnectAttempts = 0;
				resolve();
			};

			this.ws.onerror = (error) => {
				reject(error);
			};

			this.ws.onclose = () => {
				this.handleDisconnect();
			};

			this.ws.onmessage = (event) => {
				try {
					const data = JSON.parse(event.data);
					this.emit(data.type, data);
				} catch {
					console.error('Failed to parse WebSocket message');
				}
			};
		});
	}

	private handleDisconnect(): void {
		if (this.reconnectAttempts < this.maxReconnectAttempts) {
			this.reconnectAttempts++;
			setTimeout(() => this.connect(), 1000 * this.reconnectAttempts);
		}
	}

	on(event: string, callback: (data: unknown) => void): void {
		if (!this.listeners.has(event)) {
			this.listeners.set(event, new Set());
		}
		this.listeners.get(event)!.add(callback);
	}

	off(event: string, callback: (data: unknown) => void): void {
		this.listeners.get(event)?.delete(callback);
	}

	private emit(event: string, data: unknown): void {
		this.listeners.get(event)?.forEach((callback) => callback(data));
	}

	disconnect(): void {
		this.ws?.close();
		this.ws = null;
		this.listeners.clear();
	}

	send(data: unknown): void {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(JSON.stringify(data));
		}
	}
}

// Singleton WebSocket instance
let wsInstance: SidecarWebSocket | null = null;

export function getWebSocket(): SidecarWebSocket {
	if (!wsInstance) {
		wsInstance = new SidecarWebSocket();
	}
	return wsInstance;
}
