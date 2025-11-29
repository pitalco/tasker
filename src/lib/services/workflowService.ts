import { invoke } from '@tauri-apps/api/core';
import type { Workflow, CreateWorkflowRequest, UpdateWorkflowRequest } from '$lib/types/workflow';

export async function getWorkflows(): Promise<Workflow[]> {
	return invoke<Workflow[]>('get_workflows');
}

export async function getWorkflow(id: string): Promise<Workflow | null> {
	return invoke<Workflow | null>('get_workflow', { id });
}

export async function createWorkflow(request: CreateWorkflowRequest): Promise<Workflow> {
	return invoke<Workflow>('create_workflow', { request });
}

export async function updateWorkflow(
	id: string,
	request: UpdateWorkflowRequest
): Promise<Workflow | null> {
	return invoke<Workflow | null>('update_workflow', { id, request });
}

export async function deleteWorkflow(id: string): Promise<boolean> {
	return invoke<boolean>('delete_workflow', { id });
}
