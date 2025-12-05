// File category based on MIME type
export type FileCategory =
	| 'text'
	| 'code'
	| 'markdown'
	| 'pdf'
	| 'image'
	| 'csv'
	| 'excel'
	| 'document'
	| 'other';

// File metadata from API
export interface TaskerFile {
	id: string;
	run_id: string;
	workflow_id?: string;
	file_name: string;
	file_path: string;
	mime_type: string;
	file_size: number;
	created_at: string;
	updated_at: string;
	// Denormalized for display
	run_name?: string;
	workflow_name?: string;
}

// File content response (with base64 encoded content)
export interface FileContentResponse {
	id: string;
	file_name: string;
	file_path: string;
	mime_type: string;
	file_size: number;
	content_base64: string;
}

// Response for listing files
export interface FileListResponse {
	files: TaskerFile[];
	total: number;
}

// Query parameters for listing files
export interface FileListQuery {
	limit?: number;
	offset?: number;
	run_id?: string;
}
