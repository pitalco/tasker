import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type ActionType =
	// Navigation
	| 'navigate'
	| 'go_back'
	| 'search'
	| 'wait'
	// Page Interaction
	| 'click'
	| 'input'
	| 'scroll'
	| 'send_keys'
	| 'upload_file'
	| 'find_text'
	// Tab Management
	| 'switch_tab'
	| 'close_tab'
	| 'new_tab'
	// Form Controls
	| 'select_dropdown'
	| 'dropdown_options'
	// Content
	| 'extract'
	| 'screenshot'
	// JavaScript
	| 'evaluate'
	// Internal
	| 'script_loaded'
	| 'page_loaded'
	| 'context_menu';

export interface ElementInfo {
	index: number;
	selector: string;
	xpath: string;
	tagName: string;
	text: string;
	attributes: {
		id: string | null;
		name: string | null;
		class: string | null;
		type: string | null;
		placeholder: string | null;
		ariaLabel: string | null;
		role: string | null;
		href: string | null;
	};
	rect: DOMRect;
}

export interface RecordingEvent {
	actionType: ActionType;
	data: {
		// Common
		url: string;
		timestamp: number;

		// Element info (for click, input, etc.)
		element?: ElementInfo;
		coordinates?: { x: number; y: number };

		// Input/text
		text?: string;
		query?: string;

		// Scroll
		direction?: 'up' | 'down';
		amount?: number;
		scrollY?: number;

		// Keys
		key?: string;
		modifiers?: { ctrl: boolean; alt: boolean; shift: boolean; meta: boolean };

		// File upload
		files?: { name: string; size: number; type: string }[];

		// Dropdown
		value?: string;
		selectedText?: string;
		options?: { value: string; text: string; selected: boolean }[];

		// Tab
		target?: string;
		visible?: boolean;

		// Page
		title?: string;
		readyState?: string;
	};
}

// Debug: get window position info
export interface WindowPositionInfo {
	outer_position: { x: number; y: number };
	inner_position: { x: number; y: number };
	outer_size: { width: number; height: number };
	inner_size: { width: number; height: number };
	scale_factor: number;
}

export async function getWindowPosition(): Promise<WindowPositionInfo> {
	return invoke<WindowPositionInfo>('get_window_position');
}

// Tab management
export async function createBrowserTab(
	url: string,
	label: string,
	bounds: [number, number, number, number]
): Promise<string> {
	console.log('Creating browser tab with bounds:', bounds);
	return invoke<string>('create_browser_tab', { url, label, bounds });
}

export async function closeBrowserTab(label: string): Promise<boolean> {
	return invoke<boolean>('close_browser_tab', { label });
}

export async function navigateTab(label: string, url: string): Promise<boolean> {
	return invoke<boolean>('navigate_tab', { label, url });
}

export async function resizeTab(label: string, bounds: [number, number, number, number]): Promise<boolean> {
	return invoke<boolean>('resize_tab', { label, bounds });
}

export async function setTabVisible(label: string, visible: boolean): Promise<boolean> {
	return invoke<boolean>('set_tab_visible', { label, visible });
}

export async function evalInTab(label: string, script: string): Promise<boolean> {
	return invoke<boolean>('eval_in_tab', { label, script });
}

// Recording control
export async function pauseRecording(label: string): Promise<boolean> {
	return invoke<boolean>('pause_recording', { label });
}

export async function resumeRecording(label: string): Promise<boolean> {
	return invoke<boolean>('resume_recording', { label });
}

// Navigation
export async function tabGoBack(label: string): Promise<boolean> {
	return invoke<boolean>('tab_go_back', { label });
}

export async function tabGoForward(label: string): Promise<boolean> {
	return invoke<boolean>('tab_go_forward', { label });
}

export async function tabReload(label: string): Promise<boolean> {
	return invoke<boolean>('tab_reload', { label });
}

// Event listening
export function onRecordingEvent(callback: (event: RecordingEvent) => void): Promise<UnlistenFn> {
	return listen<RecordingEvent>('recording_event', (event) => callback(event.payload));
}
