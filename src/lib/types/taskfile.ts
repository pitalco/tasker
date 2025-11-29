// Taskfile YAML schema types

export interface Taskfile {
	taskfile: string;
	metadata: TaskfileMetadata;
	triggers: Triggers;
	dependencies: Dependencies;
	limits: Limits;
	variables: Variable[];
	execution: ExecutionConfig;
	steps: TaskfileStep[];
	output: Output;
}

export interface TaskfileMetadata {
	name: string;
	description?: string;
	version: string;
	author?: string;
	tags: string[];
}

// === TRIGGERS ===

export interface Triggers {
	manual: ManualTrigger;
	cron?: CronTrigger;
	http?: HttpTrigger;
}

export interface ManualTrigger {
	enabled: boolean;
}

export interface CronTrigger {
	enabled: boolean;
	expression: string;
	timezone: string;
}

export interface HttpTrigger {
	enabled: boolean;
	path: string;
	method: string;
	auth?: HttpAuth;
}

export interface HttpAuth {
	type: 'none' | 'api_key' | 'bearer' | 'hmac';
	header?: string;
	secret_env?: string;
}

// === DEPENDENCIES ===

export interface Dependencies {
	browser: BrowserDependency;
	env: EnvDependency[];
	accounts: string[];
}

export interface BrowserDependency {
	type: 'chromium' | 'firefox' | 'webkit';
	headless: boolean;
}

export interface EnvDependency {
	name: string;
	required: boolean;
	sensitive: boolean;
	default?: string;
	description?: string;
}

// === LIMITS ===

export interface Limits {
	timeout_seconds: number;
	max_steps: number;
	network?: NetworkLimits;
}

export interface NetworkLimits {
	allowed_domains: string[];
}

// === VARIABLES ===

export interface Variable {
	name: string;
	type: 'string' | 'number' | 'boolean';
	required: boolean;
	default?: unknown;
	description?: string;
}

// === EXECUTION ===

export interface ExecutionConfig {
	mode: 'direct' | 'ai_assisted';
	llm?: LLMExecutionConfig;
	retry: RetryConfig;
}

export interface LLMExecutionConfig {
	provider: string;
	model: string;
	api_key_env?: string;
}

export interface RetryConfig {
	max_attempts: number;
	delay_ms: number;
}

// === STEPS ===

export interface TaskfileStep {
	id: string;
	action: TaskfileAction;
	description?: string;
	condition?: StepCondition;
}

export type TaskfileAction =
	| NavigateAction
	| ClickAction
	| TypeAction
	| WaitAction
	| ExtractAction
	| ScreenshotAction
	| ScrollAction
	| SelectAction
	| HoverAction
	| CustomAction;

export interface NavigateAction {
	type: 'navigate';
	url: string;
}

export interface ClickAction {
	type: 'click';
	selector: Selector;
}

export interface TypeAction {
	type: 'type';
	selector: Selector;
	text: string;
	clear_first?: boolean;
}

export interface WaitAction {
	type: 'wait';
	condition: WaitCondition;
}

export interface ExtractAction {
	type: 'extract';
	selector: Selector;
	attribute: string;
	variable: string;
}

export interface ScreenshotAction {
	type: 'screenshot';
	full_page?: boolean;
	variable?: string;
}

export interface ScrollAction {
	type: 'scroll';
	direction?: string;
	amount?: number;
}

export interface SelectAction {
	type: 'select';
	selector: Selector;
	value: string;
}

export interface HoverAction {
	type: 'hover';
	selector: Selector;
}

export interface CustomAction {
	type: 'custom';
	prompt: string;
}

export interface Selector {
	css?: string;
	xpath?: string;
	text?: string;
	aria_label?: string;
}

export type WaitCondition =
	| UrlMatchCondition
	| ElementVisibleCondition
	| ElementHiddenCondition
	| DelayCondition;

export interface UrlMatchCondition {
	type: 'url_match';
	value: string;
	timeout_ms: number;
}

export interface ElementVisibleCondition {
	type: 'element_visible';
	selector: Selector;
	timeout_ms: number;
}

export interface ElementHiddenCondition {
	type: 'element_hidden';
	selector: Selector;
	timeout_ms: number;
}

export interface DelayCondition {
	type: 'delay';
	ms: number;
}

export interface StepCondition {
	variable: string;
	operator: 'eq' | 'ne' | 'contains' | 'exists';
	value?: unknown;
}

// === OUTPUT ===

export interface Output {
	variables: string[];
	screenshots: ScreenshotOutput;
}

export interface ScreenshotOutput {
	include: boolean;
	format: 'png' | 'jpeg';
}

// === VALIDATION ===

export interface ValidationResult {
	valid: boolean;
	errors: ValidationError[];
	warnings: string[];
}

export interface ValidationError {
	path: string;
	message: string;
}

// === IMPORT/EXPORT RESULTS ===

export interface ImportResult {
	workflow_id: string;
	name: string;
	validation: ValidationResult;
}

export interface ExportResult {
	yaml: string;
	filename: string;
}
