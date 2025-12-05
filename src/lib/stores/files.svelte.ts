import { getContext, setContext } from 'svelte';
import type { TaskerFile, FileContentResponse, FileCategory } from '$lib/types/file';
import * as filesService from '$lib/services/filesService';

const FILES_KEY = Symbol('files');

class FilesState {
	files = $state<TaskerFile[]>([]);
	currentFile = $state<FileContentResponse | null>(null);
	isLoading = $state(false);
	error = $state<string | null>(null);
	total = $state(0);
	limit = $state(50);
	offset = $state(0);

	// Current run filter (when viewing files for a specific run)
	runFilter = $state<string | null>(null);

	// Category filter
	categoryFilter = $state<FileCategory | null>(null);

	async loadFiles() {
		this.isLoading = true;
		this.error = null;
		try {
			if (this.runFilter) {
				// Loading files for a specific run
				const response = await filesService.listFilesForRun(this.runFilter);
				this.files = this.filterByCategory(response.files);
				this.total = response.total;
			} else {
				// Loading all files
				const response = await filesService.listFiles(this.limit, this.offset);
				this.files = this.filterByCategory(response.files);
				this.total = response.total;
			}
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load files';
			console.error('Failed to load files:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async loadFilesForRun(runId: string) {
		this.runFilter = runId;
		this.offset = 0;
		await this.loadFiles();
	}

	async loadFileContent(fileId: string) {
		this.isLoading = true;
		this.error = null;
		try {
			this.currentFile = await filesService.getFileContent(fileId);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to load file content';
			console.error('Failed to load file content:', e);
		} finally {
			this.isLoading = false;
		}
	}

	async deleteFile(fileId: string): Promise<boolean> {
		this.error = null;
		try {
			await filesService.deleteFile(fileId);
			this.files = this.files.filter((f) => f.id !== fileId);
			this.total = Math.max(0, this.total - 1);
			if (this.currentFile?.id === fileId) {
				this.currentFile = null;
			}
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to delete file';
			console.error('Failed to delete file:', e);
			return false;
		}
	}

	async downloadFile(file: TaskerFile): Promise<boolean> {
		try {
			return await filesService.downloadFile(file.id, file.file_name);
		} catch (e) {
			this.error = e instanceof Error ? e.message : 'Failed to download file';
			console.error('Failed to download file:', e);
			return false;
		}
	}

	setCategoryFilter(category: FileCategory | null) {
		this.categoryFilter = category;
		this.offset = 0;
		this.loadFiles();
	}

	setPage(page: number) {
		this.offset = (page - 1) * this.limit;
		this.loadFiles();
	}

	clearRunFilter() {
		this.runFilter = null;
		this.offset = 0;
		this.loadFiles();
	}

	clearError() {
		this.error = null;
	}

	clearCurrent() {
		this.currentFile = null;
	}

	// Filter files by category (client-side for now)
	private filterByCategory(files: TaskerFile[]): TaskerFile[] {
		if (!this.categoryFilter) return files;
		return files.filter((f) => {
			const category = filesService.getFileCategory(f.mime_type);
			return category === this.categoryFilter;
		});
	}

	// Computed property for current page
	get currentPage(): number {
		return Math.floor(this.offset / this.limit) + 1;
	}

	// Computed property for total pages
	get totalPages(): number {
		return Math.ceil(this.total / this.limit);
	}
}

export function createFilesState() {
	return setContext(FILES_KEY, new FilesState());
}

export function getFilesState(): FilesState {
	return getContext<FilesState>(FILES_KEY);
}
