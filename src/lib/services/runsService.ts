import { getSidecarUrls } from './sidecarService';
import type {
	Run,
	RunStep,
	RunLog,
	RunListResponse,
	RunListQuery,
	StartRunRequest,
	StartRunResponse
} from '$lib/types/run';

// Get the base HTTP URL for the sidecar
async function getSidecarHttpUrl(): Promise<string> {
	const [httpUrl] = await getSidecarUrls();
	return httpUrl;
}

// List runs with optional filters
export async function listRuns(query?: RunListQuery): Promise<RunListResponse> {
	const baseUrl = await getSidecarHttpUrl();
	const params = new URLSearchParams();

	if (query?.page) params.set('page', query.page.toString());
	if (query?.per_page) params.set('per_page', query.per_page.toString());
	if (query?.status) params.set('status', query.status);
	if (query?.workflow_id) params.set('workflow_id', query.workflow_id);

	const url = `${baseUrl}/runs${params.toString() ? '?' + params.toString() : ''}`;
	const response = await fetch(url);

	if (!response.ok) {
		throw new Error(`Failed to list runs: ${response.statusText}`);
	}

	return response.json();
}

// Get a specific run by ID
export async function getRun(runId: string): Promise<Run> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs/${runId}`);

	if (!response.ok) {
		throw new Error(`Failed to get run: ${response.statusText}`);
	}

	return response.json();
}

// Start a new run
export async function startRun(request: StartRunRequest): Promise<StartRunResponse> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(request)
	});

	if (!response.ok) {
		throw new Error(`Failed to start run: ${response.statusText}`);
	}

	return response.json();
}

// Cancel a run
export async function cancelRun(runId: string): Promise<{ run_id: string; status: string }> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs/${runId}/cancel`, {
		method: 'POST'
	});

	if (!response.ok) {
		throw new Error(`Failed to cancel run: ${response.statusText}`);
	}

	return response.json();
}

// Delete a run
export async function deleteRun(runId: string): Promise<{ run_id: string; deleted: boolean }> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs/${runId}`, {
		method: 'DELETE'
	});

	if (!response.ok) {
		throw new Error(`Failed to delete run: ${response.statusText}`);
	}

	return response.json();
}

// Get run steps
export async function getRunSteps(runId: string): Promise<RunStep[]> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs/${runId}/steps`);

	if (!response.ok) {
		throw new Error(`Failed to get run steps: ${response.statusText}`);
	}

	return response.json();
}

// Get run logs
export async function getRunLogs(runId: string): Promise<RunLog[]> {
	const baseUrl = await getSidecarHttpUrl();
	const response = await fetch(`${baseUrl}/runs/${runId}/logs`);

	if (!response.ok) {
		throw new Error(`Failed to get run logs: ${response.statusText}`);
	}

	return response.json();
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
