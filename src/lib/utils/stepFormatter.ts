import type { WorkflowStep, BrowserAction, ElementSelector } from '$lib/types/workflow';

// Backend action format (from sidecar)
interface BackendAction {
	action_type?: string;
	type?: string;
	selector?: BackendSelector | ElementSelector;
	value?: string;
	url?: string;
	text?: string;
	coordinates?: { x: number; y: number };
	options?: Record<string, unknown>;
}

interface BackendSelector {
	strategy?: string;
	value?: string;
	css?: string;
	xpath?: string;
	text?: string;
	aria_label?: string;
}

/**
 * Formats a step into a human-readable description
 */
export function formatStepDescription(step: WorkflowStep): string {
	// If there's already a custom description, use it
	if (step.description && step.description.trim()) {
		return step.description;
	}

	return formatActionDescription(step.action);
}

/**
 * Gets the action type from either frontend or backend format
 */
function getActionType(action: BrowserAction | BackendAction): string {
	// Frontend format uses 'type', backend uses 'action_type'
	const actionAny = action as BackendAction;
	return actionAny.type || actionAny.action_type || '';
}

/**
 * Gets the selector from either frontend or backend format
 */
function normalizeSelector(selector: BackendSelector | ElementSelector | undefined): ElementSelector | undefined {
	if (!selector) return undefined;

	const sel = selector as BackendSelector & ElementSelector;

	// If it's already in frontend format
	if (sel.css || sel.xpath || sel.text || sel.aria_label) {
		return sel as ElementSelector;
	}

	// Convert from backend format (strategy + value)
	if (sel.strategy && sel.value) {
		switch (sel.strategy) {
			case 'css':
				return { css: sel.value };
			case 'xpath':
				return { xpath: sel.value };
			case 'text':
				return { text: sel.value };
			case 'aria_label':
				return { aria_label: sel.value };
			default:
				return { css: sel.value };
		}
	}

	return undefined;
}

/**
 * Formats an action into a human-readable description
 */
export function formatActionDescription(action: BrowserAction | BackendAction): string {
	const actionType = getActionType(action);
	const actionAny = action as BackendAction;
	const selector = normalizeSelector(actionAny.selector);

	switch (actionType) {
		case 'click': {
			const target = getTargetDescription(selector);
			return `Click ${target}`;
		}

		case 'type': {
			const text = truncateText(actionAny.value || (action as { text?: string }).text || '', 30);
			const target = getTargetDescription(selector);
			if (text) {
				return `Type "${text}" ${target ? `in ${target}` : ''}`.trim();
			}
			return `Type in ${target}`;
		}

		case 'navigate': {
			const url = truncateText(actionAny.url || '', 40);
			return `Navigate to ${url}`;
		}

		case 'scroll': {
			const direction = (action as { direction?: string }).direction === 'up' ? 'up' : 'down';
			const target = selector ? getTargetDescription(selector) : 'page';
			return `Scroll ${direction} on ${target}`;
		}

		case 'hover': {
			const target = getTargetDescription(selector);
			return `Hover over ${target}`;
		}

		case 'wait': {
			const condition = (action as { condition?: { type?: string; value?: string | number } }).condition;
			if (condition?.type === 'element_visible') {
				return `Wait for element to appear`;
			} else if (condition?.type === 'element_hidden') {
				return `Wait for element to hide`;
			} else if (condition?.type === 'url_match') {
				return `Wait for URL to match`;
			} else if (condition?.type === 'timeout') {
				return `Wait ${condition.value}ms`;
			}
			return 'Wait';
		}

		case 'extract':
		case 'screenshot': {
			const target = getTargetDescription(selector);
			const varName = (action as { variable_name?: string }).variable_name;
			if (varName) {
				return `Extract ${target} into "${varName}"`;
			}
			return actionType === 'screenshot' ? 'Take screenshot' : `Extract ${target}`;
		}

		case 'custom': {
			const prompt = truncateText((action as { prompt?: string }).prompt || '', 40);
			return `Custom: ${prompt}`;
		}

		case 'go_back':
			return 'Go back';

		case 'search': {
			const query = truncateText((action as { query?: string }).query || '', 30);
			return `Search for "${query}"`;
		}

		case 'send_keys': {
			const mods = (action as { modifiers?: { ctrl?: boolean; alt?: boolean; shift?: boolean; meta?: boolean } }).modifiers;
			const modifiers = [];
			if (mods?.ctrl) modifiers.push('Ctrl');
			if (mods?.alt) modifiers.push('Alt');
			if (mods?.shift) modifiers.push('Shift');
			if (mods?.meta) modifiers.push('Cmd');

			const key = (action as { key?: string }).key || '';
			const keyCombo = [...modifiers, key].join('+');
			return `Press ${keyCombo}`;
		}

		case 'upload_file': {
			const fileCount = (action as { files?: string[] }).files?.length || 0;
			return `Upload ${fileCount} file${fileCount !== 1 ? 's' : ''}`;
		}

		case 'select': {
			const value = (action as { text?: string }).text || actionAny.value || '';
			const target = getTargetDescription(selector);
			return `Select "${truncateText(value, 20)}" ${target ? `in ${target}` : ''}`.trim();
		}

		case 'new_tab': {
			if (actionAny.url) {
				return `Open new tab: ${truncateText(actionAny.url, 30)}`;
			}
			return 'Open new tab';
		}

		case 'close_tab':
			return 'Close tab';

		case 'switch_tab': {
			const tabIndex = (action as { tab_index?: number }).tab_index;
			if (tabIndex !== undefined) {
				return `Switch to tab ${tabIndex + 1}`;
			}
			return 'Switch tab';
		}

		default:
			return actionType ? `${actionType.charAt(0).toUpperCase()}${actionType.slice(1)}` : 'Unknown action';
	}
}

/**
 * Gets a human-readable description of the target element
 */
function getTargetDescription(selector?: ElementSelector): string {
	if (!selector) return 'element';

	// Prefer text content if available
	if (selector.text) {
		return `"${truncateText(selector.text, 25)}"`;
	}

	// Use aria label if available
	if (selector.aria_label) {
		return `"${truncateText(selector.aria_label, 25)}"`;
	}

	// Use visual description if available
	if (selector.visual_description) {
		return truncateText(selector.visual_description, 30);
	}

	// Try to extract meaningful info from CSS selector
	if (selector.css) {
		return describeCssSelector(selector.css);
	}

	return 'element';
}

/**
 * Extracts a human-readable description from a CSS selector
 */
function describeCssSelector(css: string): string {
	// Extract tag name
	const tagMatch = css.match(/^(\w+)/);
	const tag = tagMatch ? tagMatch[1].toLowerCase() : '';

	// Extract id
	const idMatch = css.match(/#([\w-]+)/);
	const id = idMatch ? idMatch[1] : '';

	// Extract classes
	const classMatches = css.match(/\.([\w-]+)/g);
	const classes = classMatches ? classMatches.map(c => c.slice(1)) : [];

	// Build description
	if (id) {
		return `#${id}`;
	}

	// Common element types with friendly names
	const friendlyNames: Record<string, string> = {
		button: 'button',
		input: 'input field',
		textarea: 'text area',
		select: 'dropdown',
		a: 'link',
		img: 'image',
		form: 'form',
		div: 'element',
		span: 'element',
		p: 'paragraph',
		h1: 'heading',
		h2: 'heading',
		h3: 'heading',
		li: 'list item',
		ul: 'list',
		table: 'table',
		tr: 'row',
		td: 'cell'
	};

	const friendlyTag = friendlyNames[tag] || tag || 'element';

	// Look for meaningful class names
	const meaningfulClasses = classes.filter(c =>
		!c.match(/^(css-|sc-|styled-|_|[a-z]{5,}[A-Z])/) && // Skip generated class names
		c.length > 2
	);

	if (meaningfulClasses.length > 0) {
		return `${friendlyTag} (.${meaningfulClasses[0]})`;
	}

	return friendlyTag;
}

/**
 * Truncates text to a maximum length, adding ellipsis if needed
 */
function truncateText(text: string, maxLength: number): string {
	if (!text) return '';
	if (text.length <= maxLength) return text;
	return text.slice(0, maxLength - 1) + '…';
}
