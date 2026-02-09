import { invoke } from '@tauri-apps/api/core';
import type {
	Run,
	RunStep,
	RunLog,
	RunListResponse,
	RunListQuery,
	StartRunRequest,
	StartRunResponse
} from '$lib/types/run';

// List runs with optional filters
export async function listRuns(query?: RunListQuery): Promise<RunListResponse> {
	return invoke<RunListResponse>('list_runs', {
		page: query?.page ?? null,
		perPage: query?.per_page ?? null,
		status: query?.status ?? null,
		workflowId: query?.workflow_id ?? null
	});
}

// Get a specific run by ID
export async function getRun(runId: string): Promise<Run> {
	return invoke<Run>('get_run', { runId });
}

// Start a new run
export async function startRun(request: StartRunRequest): Promise<StartRunResponse> {
	return invoke<StartRunResponse>('start_run', { request });
}

// Cancel a run
export async function cancelRun(runId: string): Promise<{ run_id: string; status: string }> {
	return invoke<{ run_id: string; status: string }>('cancel_run', { runId });
}

// Delete a run
export async function deleteRun(runId: string): Promise<{ run_id: string; deleted: boolean }> {
	return invoke<{ run_id: string; deleted: boolean }>('delete_run', { runId });
}

// Get run steps
export async function getRunSteps(runId: string): Promise<RunStep[]> {
	return invoke<RunStep[]>('get_run_steps', { runId });
}

// Get run logs
export async function getRunLogs(runId: string): Promise<RunLog[]> {
	return invoke<RunLog[]>('get_run_logs', { runId });
}

// Helper to format run status for display
export function formatRunStatus(status: string): string {
	const statusMap: Record<string, string> = {
		pending: 'PENDING',
		running: 'RUNNING',
		completed: 'COMPLETED',
		failed: 'FAILED',
		cancelled: 'CANCELLED'
	};
	return statusMap[status] || status.toUpperCase();
}

// Helper to get status color class
export function getStatusColorClass(status: string): string {
	const colorMap: Record<string, string> = {
		pending: 'bg-brutal-orange',
		running: 'bg-brutal-cyan',
		completed: 'bg-brutal-green',
		failed: 'bg-brutal-magenta',
		cancelled: 'bg-gray-400'
	};
	return colorMap[status] || 'bg-gray-300';
}

// Helper to format duration
export function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
	const minutes = Math.floor(ms / 60000);
	const seconds = ((ms % 60000) / 1000).toFixed(0);
	return `${minutes}m ${seconds}s`;
}

// Helper to format relative time
export function formatRelativeTime(dateString: string): string {
	const date = new Date(dateString);
	const now = new Date();
	const diffMs = now.getTime() - date.getTime();
	const diffMins = Math.floor(diffMs / 60000);
	const diffHours = Math.floor(diffMs / 3600000);
	const diffDays = Math.floor(diffMs / 86400000);

	if (diffMins < 1) return 'just now';
	if (diffMins < 60) return `${diffMins}m ago`;
	if (diffHours < 24) return `${diffHours}h ago`;
	if (diffDays < 7) return `${diffDays}d ago`;
	return date.toLocaleDateString();
}
