import { getContext, setContext } from 'svelte';
import type { Run, RunStep, RunLog, RunStatus, RunListQuery, StartRunRequest } from '$lib/types/run';
import * as runsService from '$lib/services/runsService';

const RUNS_KEY = Symbol('runs');

class RunsState {
	runs = $state<Run[]>([]);
	currentRun = $state<Run | null>(null);
	currentSteps = $state<RunStep[]>([]);
	currentLogs = $state<RunLog[]>([]);
	isLoading = $state(false);
	error = $state<string | null>(null);
	total = $state(0);
	page = $state(1);
	perPage = $state(20);
	statusFilter = $state<RunStatus | null>(null);
	workflowFilter = $state<string | null>(null);

	async loadRuns() {
		this.isLoading = true;
		this.error = null;
		try {
			const query: RunListQuery = {
				page: this.page,
				per_page: this.perPage
			};
			if (this.statusFilter) query.status = this.statusFilter;
			if (this.workflowFilter) query.workflow_id = this.workflowFilter;

			const response = await runsService.listRuns(query);
			this.runs = response.runs;
			this.total = response.total;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load runs';
			console.error('Failed to load runs:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async loadRun(id: string) {
		this.isLoading = true;
		this.error = null;
		try {
			this.currentRun = await runsService.getRun(id);
			// Load steps and logs in parallel
			const [steps, logs] = await Promise.all([
				runsService.getRunSteps(id),
				runsService.getRunLogs(id)
			]);
			this.currentSteps = steps;
			this.currentLogs = logs;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load run';
			console.error('Failed to load run:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async startRun(request: StartRunRequest): Promise<string | null> {
		this.isLoading = true;
		this.error = null;
		try {
			const response = await runsService.startRun(request);
			// Reload runs to include the new one
			await this.loadRuns();
			return response.run_id;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to start run';
			console.error('Failed to start run:', e);
			return null;
		} finally {
			this.isLoading = false;
		}
	}

	async cancelRun(id: string): Promise<boolean> {
		this.error = null;
		try {
			await runsService.cancelRun(id);
			// Update the run in the list
			this.runs = this.runs.map((r) =>
				r.id === id ? { ...r, status: 'cancelled' as RunStatus } : r
			);
			if (this.currentRun?.id === id) {
				this.currentRun = { ...this.currentRun, status: 'cancelled' };
			}
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to cancel run';
			console.error('Failed to cancel run:', e);
			return false;
		}
	}

	async deleteRun(id: string): Promise<boolean> {
		this.error = null;
		try {
			await runsService.deleteRun(id);
			this.runs = this.runs.filter((r) => r.id !== id);
			if (this.currentRun?.id === id) {
				this.currentRun = null;
			}
			this.total = Math.max(0, this.total - 1);
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to delete run';
			console.error('Failed to delete run:', e);
			return false;
		}
	}

	async refreshSteps(runId: string) {
		try {
			this.currentSteps = await runsService.getRunSteps(runId);
		} catch (e) {
			console.error('Failed to refresh steps:', e);
		}
	}

	async refreshLogs(runId: string) {
		try {
			this.currentLogs = await runsService.getRunLogs(runId);
		} catch (e) {
			console.error('Failed to refresh logs:', e);
		}
	}

	setStatusFilter(status: RunStatus | null) {
		this.statusFilter = status;
		this.page = 1;
		this.loadRuns();
	}

	setWorkflowFilter(workflowId: string | null) {
		this.workflowFilter = workflowId;
		this.page = 1;
		this.loadRuns();
	}

	setPage(newPage: number) {
		this.page = newPage;
		this.loadRuns();
	}

	// Handle real-time updates from WebSocket
	handleRunUpdate(run: Run) {
		// Update in list if exists
		const existingIndex = this.runs.findIndex((r) => r.id === run.id);
		if (existingIndex >= 0) {
			this.runs = [
				...this.runs.slice(0, existingIndex),
				run,
				...this.runs.slice(existingIndex + 1)
			];
		} else {
			// Add to beginning of list
			this.runs = [run, ...this.runs];
			this.total++;
		}

		// Update current run if it's the same
		if (this.currentRun?.id === run.id) {
			this.currentRun = run;
		}
	}

	handleStepUpdate(step: RunStep) {
		if (this.currentRun?.id === step.run_id) {
			const existingIndex = this.currentSteps.findIndex((s) => s.id === step.id);
			if (existingIndex >= 0) {
				this.currentSteps = [
					...this.currentSteps.slice(0, existingIndex),
					step,
					...this.currentSteps.slice(existingIndex + 1)
				];
			} else {
				this.currentSteps = [...this.currentSteps, step];
			}
		}
	}

	handleLogUpdate(log: RunLog) {
		if (this.currentRun?.id === log.run_id) {
			this.currentLogs = [...this.currentLogs, log];
		}
	}

	clearError() {
		this.error = null;
	}

	clearCurrent() {
		this.currentRun = null;
		this.currentSteps = [];
		this.currentLogs = [];
	}
}

export function createRunsState() {
	return setContext(RUNS_KEY, new RunsState());
}

export function getRunsState(): RunsState {
	return getContext<RunsState>(RUNS_KEY);
}
