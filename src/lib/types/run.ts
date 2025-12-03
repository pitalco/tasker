// Run status enum
export type RunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

// Run model matching backend
export interface Run {
	id: string;
	workflow_id?: string;
	workflow_name?: string;
	task_description?: string;
	custom_instructions?: string;
	status: RunStatus;
	error?: string;
	result?: string;
	started_at: string; // Also used as created_at
	completed_at?: string;
	metadata: Record<string, unknown>;
}

// Run step model
export interface RunStep {
	id: string;
	run_id: string;
	step_number: number;
	tool_name: string;
	params: Record<string, unknown>;
	success: boolean;
	result?: unknown;
	error?: string;
	screenshot?: string;
	duration_ms: number;
	timestamp: string;
}

// Run log entry
export interface RunLog {
	id: string;
	run_id: string;
	level: 'debug' | 'info' | 'warn' | 'error';
	message: string;
	timestamp: string;
}

// API responses
export interface RunListResponse {
	runs: Run[];
	total: number;
	page: number;
	per_page: number;
}

export interface StartRunResponse {
	run_id: string;
	status: string;
}

// Query parameters for listing runs
export interface RunListQuery {
	page?: number;
	per_page?: number;
	status?: RunStatus;
	workflow_id?: string;
}

// Request to start a new run
export interface StartRunRequest {
	workflow_id?: string;
	workflow_name?: string;
	task_description?: string;
	custom_instructions?: string;
	start_url?: string;
	headless?: boolean;
	viewport_width?: number;
	viewport_height?: number;
	hints?: unknown;
}

// WebSocket event types for runs
export interface RunUpdateEvent {
	type: 'run_status' | 'run_step' | 'run_log';
	run_id: string;
	data: Run | RunStep | RunLog;
}
