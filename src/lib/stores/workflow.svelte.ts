import { getContext, setContext } from 'svelte';
import type { Workflow, CreateWorkflowRequest, UpdateWorkflowRequest } from '$lib/types/workflow';
import * as workflowService from '$lib/services/workflowService';

const WORKFLOW_KEY = Symbol('workflow');

class WorkflowState {
	workflows = $state<Workflow[]>([]);
	currentWorkflow = $state<Workflow | null>(null);
	isLoading = $state(false);
	error = $state<string | null>(null);

	async loadWorkflows() {
		this.isLoading = true;
		this.error = null;
		try {
			this.workflows = await workflowService.getWorkflows();
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load workflows';
			console.error('Failed to load workflows:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async loadWorkflow(id: string) {
		this.isLoading = true;
		this.error = null;
		try {
			this.currentWorkflow = await workflowService.getWorkflow(id);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load workflow';
			console.error('Failed to load workflow:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async createWorkflow(request: CreateWorkflowRequest): Promise<Workflow | null> {
		this.isLoading = true;
		this.error = null;
		try {
			const workflow = await workflowService.createWorkflow(request);
			this.workflows = [workflow, ...this.workflows];
			return workflow;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to create workflow';
			console.error('Failed to create workflow:', e);
			return null;
		} finally {
			this.isLoading = false;
		}
	}

	async updateWorkflow(id: string, request: UpdateWorkflowRequest): Promise<boolean> {
		this.isLoading = true;
		this.error = null;
		try {
			const updated = await workflowService.updateWorkflow(id, request);
			if (updated) {
				this.workflows = this.workflows.map((w) => (w.id === id ? updated : w));
				if (this.currentWorkflow?.id === id) {
					this.currentWorkflow = updated;
				}
				return true;
			}
			return false;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to update workflow';
			console.error('Failed to update workflow:', e);
			return false;
		} finally {
			this.isLoading = false;
		}
	}

	async deleteWorkflow(id: string): Promise<boolean> {
		this.isLoading = true;
		this.error = null;
		try {
			const success = await workflowService.deleteWorkflow(id);
			if (success) {
				this.workflows = this.workflows.filter((w) => w.id !== id);
				if (this.currentWorkflow?.id === id) {
					this.currentWorkflow = null;
				}
			}
			return success;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to delete workflow';
			console.error('Failed to delete workflow:', e);
			return false;
		} finally {
			this.isLoading = false;
		}
	}

	clearError() {
		this.error = null;
	}
}

export function createWorkflowState() {
	return setContext(WORKFLOW_KEY, new WorkflowState());
}

export function getWorkflowState(): WorkflowState {
	return getContext<WorkflowState>(WORKFLOW_KEY);
}
