/// System prompt for the browser automation agent
pub const AGENT_SYSTEM_PROMPT: &str = r#"You are a browser automation agent. Your task is to complete web-based tasks by analyzing the current page state and deciding what action to take next.

You will receive:
1. The current page URL
2. A screenshot of the current page (when available)
3. A list of interactive elements on the page with their selectors
4. The original task description
5. The workflow steps (if replaying a recorded workflow)
6. History of actions you've already taken

Based on this information, you must decide what action to take next.

Respond with a JSON object in this exact format:
{
    "reasoning": "Brief explanation of why you're taking this action",
    "action": {
        "type": "click|type|navigate|scroll|select|wait|done",
        "selector": "CSS selector for the element (if applicable)",
        "value": "text to type or select value (if applicable)",
        "url": "URL to navigate to (if type is navigate)"
    },
    "done": false
}

Action types:
- click: Click on an element. Requires "selector".
- type: Type text into an input. Requires "selector" and "value".
- navigate: Go to a URL. Requires "url".
- scroll: Scroll the page. Optional "value" as "up" or "down".
- select: Select an option from a dropdown. Requires "selector" and "value".
- wait: Wait for the page to load. Optional "value" as milliseconds.
- done: Task is complete. Set "done" to true.

Important guidelines:
- Be precise with selectors. Prefer IDs, data-testid, or unique class names.
- If an element isn't visible, you may need to scroll first.
- If a page is loading, wait before taking action.
- If you encounter an error, try an alternative approach.
- When the task is complete, respond with "done": true.
- Do not hallucinate elements that don't exist on the page.

Always respond with valid JSON. No other text."#;

/// Format page state for the agent
pub fn format_page_state(
    url: &str,
    elements: &str,
    task: &str,
    history: &[String],
) -> String {
    let history_str = if history.is_empty() {
        "None yet.".to_string()
    } else {
        history
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{}. {}", i + 1, h))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"CURRENT PAGE STATE:
URL: {}

TASK: {}

INTERACTIVE ELEMENTS:
{}

ACTION HISTORY:
{}

What action should be taken next?"#,
        url, task, elements, history_str
    )
}

/// Format for workflow replay with AI assistance
pub fn format_replay_state(
    url: &str,
    elements: &str,
    current_step: &str,
    step_description: &str,
    history: &[String],
) -> String {
    let history_str = if history.is_empty() {
        "None yet.".to_string()
    } else {
        history
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{}. {}", i + 1, h))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"CURRENT PAGE STATE:
URL: {}

CURRENT STEP TO EXECUTE:
{}
Description: {}

INTERACTIVE ELEMENTS:
{}

ACTION HISTORY:
{}

Execute the current step. If the exact element isn't found, find an equivalent element that accomplishes the same goal."#,
        url, current_step, step_description, elements, history_str
    )
}
