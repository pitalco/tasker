import { invoke } from '@tauri-apps/api/core';
import type {
	Taskfile,
	ValidationResult,
	ImportResult,
	ExportResult
} from '$lib/types/taskfile';

/**
 * Parse a Taskfile YAML string
 */
export async function parseTaskfile(yamlContent: string): Promise<Taskfile> {
	return invoke<Taskfile>('parse_taskfile', { yamlContent });
}

/**
 * Validate a parsed Taskfile
 */
export async function validateTaskfile(taskfile: Taskfile): Promise<ValidationResult> {
	return invoke<ValidationResult>('validate_taskfile', { taskfile });
}

/**
 * Import a Taskfile and create a new workflow
 */
export async function importTaskfile(yamlContent: string): Promise<ImportResult> {
	return invoke<ImportResult>('import_taskfile', { yamlContent });
}

/**
 * Export a workflow as a Taskfile YAML string
 */
export async function exportTaskfile(workflowId: string): Promise<ExportResult> {
	return invoke<ExportResult>('export_taskfile', { workflowId });
}

/**
 * Get a suggested filename for a taskfile export
 */
export async function suggestTaskfileFilename(workflowId: string): Promise<string> {
	return invoke<string>('suggest_taskfile_filename', { workflowId });
}

/**
 * Save a taskfile using native file dialog
 */
export async function downloadTaskfile(workflowId: string): Promise<boolean> {
	return invoke<boolean>('save_taskfile', { workflowId });
}

/**
 * Read a Taskfile from a File input
 */
export async function readTaskfileFromFile(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(reader.result as string);
		reader.onerror = () => reject(new Error('Failed to read file'));
		reader.readAsText(file);
	});
}

/**
 * Import a Taskfile from a File input
 */
export async function importTaskfileFromFile(file: File): Promise<ImportResult> {
	const content = await readTaskfileFromFile(file);
	return importTaskfile(content);
}
