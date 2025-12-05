export interface Workflow {
	id: string;
	name: string;
	steps: WorkflowStep[];
	variables: WorkflowVariable[];
	metadata: WorkflowMetadata;
	created_at: string;
	updated_at: string;
	version: number;
	/** Task description - what this workflow automates */
	task_description?: string;
	/** Optional condition - agent will NOT stop until this is met */
	stop_when?: string;
	/** Max steps override (null = use global default) */
	max_steps?: number;
}

export interface WorkflowStep {
	id: string;
	order: number;
	name: string;
	action: BrowserAction;
	description?: string;
	screenshot_path?: string;
	dom_snapshot?: DOMSnapshot;
}

export type BrowserAction =
	| ClickAction
	| TypeAction
	| NavigateAction
	| ScrollAction
	| WaitAction
	| ExtractAction
	| CustomAction
	| GoBackAction
	| SearchAction
	| SendKeysAction
	| UploadFileAction
	| SelectAction
	| NewTabAction
	| CloseTabAction
	| SwitchTabAction;

export interface ClickAction {
	type: 'click';
	selector?: ElementSelector;
	button?: 'left' | 'right' | 'middle';
	coordinates?: { x: number; y: number };
}

export interface TypeAction {
	type: 'type';
	selector?: ElementSelector;
	text: string;
	clear_first?: boolean;
}

export interface NavigateAction {
	type: 'navigate';
	url: string;
}

export interface ScrollAction {
	type: 'scroll';
	selector?: ElementSelector;
	direction: 'up' | 'down';
	amount?: number;
	scrollY?: number;
}

export interface WaitAction {
	type: 'wait';
	condition: WaitCondition;
}

export interface ExtractAction {
	type: 'extract';
	selector?: ElementSelector;
	attribute?: string;
	variable_name?: string;
	text?: string;
}

export interface CustomAction {
	type: 'custom';
	prompt: string;
	expected_outcome?: string;
}

export interface GoBackAction {
	type: 'go_back';
}

export interface SearchAction {
	type: 'search';
	query: string;
}

export interface SendKeysAction {
	type: 'send_keys';
	key: string;
	modifiers?: { ctrl?: boolean; alt?: boolean; shift?: boolean; meta?: boolean };
	selector?: ElementSelector;
}

export interface UploadFileAction {
	type: 'upload_file';
	selector?: ElementSelector;
	files?: string[];
}

export interface SelectAction {
	type: 'select';
	selector?: ElementSelector;
	value: string;
	text?: string;
}

export interface NewTabAction {
	type: 'new_tab';
	url?: string;
}

export interface CloseTabAction {
	type: 'close_tab';
}

export interface SwitchTabAction {
	type: 'switch_tab';
	tab_index?: number;
}

export interface ElementSelector {
	css?: string;
	xpath?: string;
	text?: string;
	aria_label?: string;
	visual_description?: string;
}

export interface WaitCondition {
	type: 'element_visible' | 'element_hidden' | 'url_match' | 'timeout';
	value: string | number;
	timeout_ms?: number;
}

export interface DOMSnapshot {
	html?: string;
	interactive_elements?: InteractiveElement[];
}

export interface InteractiveElement {
	index: number;
	tag: string;
	selector: ElementSelector;
	text?: string;
}

export interface WorkflowVariable {
	name: string;
	type: 'string' | 'number' | 'list';
	default_value?: unknown;
	description?: string;
}

export interface WorkflowMetadata {
	start_url?: string;
	llm_provider?: string;
	recording_source: 'manual' | 'recorded' | 'embedded' | 'text_description';
}

export interface CreateWorkflowRequest {
	name: string;
	steps?: WorkflowStep[];
	variables?: WorkflowVariable[];
	metadata?: WorkflowMetadata;
	/** Task description - what this workflow automates */
	task_description?: string;
	/** Optional condition - agent will NOT stop until this is met */
	stop_when?: string;
	/** Max steps override (null = use global default) */
	max_steps?: number;
}

export interface UpdateWorkflowRequest {
	name?: string;
	steps?: WorkflowStep[];
	variables?: WorkflowVariable[];
	metadata?: WorkflowMetadata;
	/** Task description - what this workflow automates */
	task_description?: string;
	/** Optional condition - agent will NOT stop until this is met */
	stop_when?: string;
	/** Max steps override (null = use global default) */
	max_steps?: number;
}
