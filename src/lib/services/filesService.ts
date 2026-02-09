import { invoke } from '@tauri-apps/api/core';
import type { TaskerFile, FileContentResponse, FileListResponse, FileCategory } from '$lib/types/file';

// List all files with pagination
export async function listFiles(limit?: number, offset?: number): Promise<FileListResponse> {
	return invoke<FileListResponse>('get_all_files', {
		limit: limit ?? null,
		offset: offset ?? null
	});
}

// List files for a specific run
export async function listFilesForRun(runId: string): Promise<FileListResponse> {
	const files = await invoke<TaskerFile[]>('get_files_for_run', { runId });
	return { files, total: files.length };
}

// Get file content by ID (returns base64 encoded content)
export async function getFileContent(fileId: string): Promise<FileContentResponse> {
	return invoke<FileContentResponse>('get_file_content', { fileId });
}

// Delete a file
export async function deleteFile(fileId: string): Promise<boolean> {
	return invoke<boolean>('delete_file', { fileId });
}

// Download file using native save dialog (via Tauri command)
export async function downloadFile(fileId: string, suggestedName: string): Promise<boolean> {
	return invoke<boolean>('download_file', { fileId, suggestedName });
}

// Helper to format file size
export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

// Helper to get file category from MIME type and file name
export function getFileCategory(mimeType: string, fileName?: string): FileCategory {
	// Check file extension first if provided
	if (fileName) {
		const ext = getFileExtension(fileName).toLowerCase();

		// Extension-based detection for better accuracy
		if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico'].includes(ext)) return 'image';
		if (ext === 'pdf') return 'pdf';
		if (ext === 'csv') return 'csv';
		if (['xlsx', 'xls'].includes(ext)) return 'excel';
		if (['docx', 'doc'].includes(ext)) return 'document';
		if (ext === 'md') return 'markdown';
		if (
			[
				'js',
				'ts',
				'jsx',
				'tsx',
				'py',
				'rs',
				'go',
				'java',
				'rb',
				'php',
				'c',
				'cpp',
				'h',
				'hpp',
				'cs',
				'swift',
				'kt',
				'scala',
				'html',
				'htm',
				'css',
				'scss',
				'sass',
				'less',
				'json',
				'xml',
				'yaml',
				'yml',
				'sql',
				'sh',
				'bash',
				'zsh'
			].includes(ext)
		)
			return 'code';
		if (ext === 'txt') return 'text';
	}

	// Fall back to MIME type detection
	// Images
	if (mimeType.startsWith('image/')) return 'image';

	// PDF
	if (mimeType === 'application/pdf') return 'pdf';

	// CSV
	if (mimeType === 'text/csv' || mimeType === 'application/csv') return 'csv';

	// Excel
	if (
		mimeType === 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' ||
		mimeType === 'application/vnd.ms-excel'
	)
		return 'excel';

	// Word documents
	if (
		mimeType === 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' ||
		mimeType === 'application/msword'
	)
		return 'document';

	// Markdown
	if (mimeType === 'text/markdown' || mimeType === 'text/x-markdown') return 'markdown';

	// Code files
	if (
		mimeType === 'application/json' ||
		mimeType === 'application/javascript' ||
		mimeType === 'text/javascript' ||
		mimeType === 'text/typescript' ||
		mimeType === 'text/x-python' ||
		mimeType === 'text/x-rust' ||
		mimeType === 'text/html' ||
		mimeType === 'text/css' ||
		mimeType === 'text/xml' ||
		mimeType === 'application/xml'
	)
		return 'code';

	// Plain text
	if (mimeType.startsWith('text/')) return 'text';

	return 'other';
}

// Helper to get category color class
export function getCategoryColorClass(category: FileCategory): string {
	const colorMap: Record<FileCategory, string> = {
		text: 'bg-brutal-cyan',
		code: 'bg-brutal-purple',
		markdown: 'bg-brutal-lime',
		pdf: 'bg-brutal-orange',
		image: 'bg-brutal-magenta',
		csv: 'bg-brutal-green',
		excel: 'bg-brutal-green',
		document: 'bg-brutal-yellow',
		other: 'bg-gray-400'
	};
	return colorMap[category];
}

// Helper to get file extension from name
export function getFileExtension(fileName: string): string {
	const parts = fileName.split('.');
	return parts.length > 1 ? parts.pop()?.toLowerCase() || '' : '';
}

// Helper to get language for syntax highlighting based on file extension
export function getHighlightLanguage(fileName: string, mimeType: string): string {
	const ext = getFileExtension(fileName);

	const extMap: Record<string, string> = {
		js: 'javascript',
		ts: 'typescript',
		jsx: 'javascript',
		tsx: 'typescript',
		py: 'python',
		rs: 'rust',
		go: 'go',
		java: 'java',
		rb: 'ruby',
		php: 'php',
		c: 'c',
		cpp: 'cpp',
		h: 'c',
		hpp: 'cpp',
		cs: 'csharp',
		swift: 'swift',
		kt: 'kotlin',
		scala: 'scala',
		html: 'html',
		htm: 'html',
		css: 'css',
		scss: 'scss',
		sass: 'sass',
		less: 'less',
		json: 'json',
		xml: 'xml',
		yaml: 'yaml',
		yml: 'yaml',
		md: 'markdown',
		sql: 'sql',
		sh: 'bash',
		bash: 'bash',
		zsh: 'bash',
		dockerfile: 'dockerfile',
		makefile: 'makefile'
	};

	return extMap[ext] || 'plaintext';
}

// Helper to decode base64 content
export function decodeBase64(base64: string): string {
	try {
		return atob(base64);
	} catch {
		return '';
	}
}

// Helper to decode base64 to Uint8Array (for binary files)
export function decodeBase64ToBytes(base64: string): Uint8Array {
	const binaryString = atob(base64);
	const bytes = new Uint8Array(binaryString.length);
	for (let i = 0; i < binaryString.length; i++) {
		bytes[i] = binaryString.charCodeAt(i);
	}
	return bytes;
}
