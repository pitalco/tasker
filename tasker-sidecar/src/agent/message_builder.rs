use crate::browser::DOMExtractionResult;
use crate::models::RecordedAction;
use crate::tools::Memory;

/// Builds the user message for each LLM turn
pub struct UserMessageBuilder {
    recorded_workflow: Option<Vec<RecordedAction>>,
    custom_instructions: Option<String>,
    memories: Vec<Memory>,
    url: String,
    title: String,
    elements_repr: String,
    step_number: Option<usize>,
    max_steps: Option<usize>,
}

impl UserMessageBuilder {
    pub fn new() -> Self {
        Self {
            recorded_workflow: None,
            custom_instructions: None,
            memories: Vec::new(),
            url: String::new(),
            title: String::new(),
            elements_repr: String::new(),
            step_number: None,
            max_steps: None,
        }
    }

    /// Set the current step number and max steps
    pub fn with_step_info(mut self, step_number: usize, max_steps: usize) -> Self {
        self.step_number = Some(step_number);
        self.max_steps = Some(max_steps);
        self
    }

    /// Set the recorded workflow as hints
    pub fn with_recorded_workflow(mut self, workflow: Option<&[RecordedAction]>) -> Self {
        self.recorded_workflow = workflow.map(|w| w.to_vec());
        self
    }

    /// Set custom instructions
    pub fn with_custom_instructions(mut self, instructions: Option<&str>) -> Self {
        self.custom_instructions = instructions.map(|s| s.to_string());
        self
    }

    /// Set the memories for this run
    pub fn with_memories(mut self, memories: &[Memory]) -> Self {
        self.memories = memories.to_vec();
        self
    }

    /// Set the current browser state
    pub fn with_browser_state(mut self, url: &str, title: &str, dom_result: &DOMExtractionResult) -> Self {
        self.url = url.to_string();
        self.title = title.to_string();
        self.elements_repr = dom_result.llm_representation.clone();
        self
    }

    /// Build the user message text (without screenshot - that's added separately)
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // Add recorded workflow section if present
        if let Some(ref workflow) = self.recorded_workflow {
            if !workflow.is_empty() {
                parts.push(format_recorded_workflow(workflow));
            }
        }

        // Add custom instructions if present
        if let Some(ref instructions) = self.custom_instructions {
            if !instructions.is_empty() {
                parts.push(format!(
                    "<custom_instructions>\n{}\n</custom_instructions>",
                    instructions
                ));
            }
        }

        // Add memories section if present
        if !self.memories.is_empty() {
            parts.push(format_memories(&self.memories));
        }

        // Add browser state (always present)
        parts.push(format_browser_state(
            &self.url,
            &self.title,
            &self.elements_repr,
            self.step_number,
            self.max_steps,
        ));

        parts.join("\n\n")
    }
}

impl Default for UserMessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Format recorded workflow steps as hints
fn format_recorded_workflow(steps: &[RecordedAction]) -> String {
    let mut lines = Vec::new();
    lines.push("<recorded_workflow>".to_string());
    lines.push("The user previously recorded these steps as a guide:".to_string());

    for step in steps {
        lines.push(step.to_hint_string());
    }

    lines.push("</recorded_workflow>".to_string());
    lines.join("\n")
}

/// Format memories as context for the LLM
fn format_memories(memories: &[Memory]) -> String {
    let mut lines = Vec::new();
    lines.push("<memories>".to_string());
    lines.push("Your saved notes for this run:".to_string());

    for memory in memories {
        let key_part = memory
            .key
            .as_ref()
            .map(|k| format!(" [{}]", k))
            .unwrap_or_default();
        let cat_part = memory
            .category
            .as_ref()
            .map(|c| format!(" ({})", c))
            .unwrap_or_default();
        lines.push(format!("- {}{}{}", memory.content, key_part, cat_part));
    }

    lines.push("</memories>".to_string());
    lines.join("\n")
}

/// Format browser state with indexed elements
fn format_browser_state(
    url: &str,
    title: &str,
    elements_repr: &str,
    step_number: Option<usize>,
    max_steps: Option<usize>,
) -> String {
    let step_info = match (step_number, max_steps) {
        (Some(step), Some(max)) => format!("\nStep: {}/{}", step, max),
        _ => String::new(),
    };
    format!(
        "<browser_state>\nURL: {}\nTitle: {}{}\n\nInteractive Elements:\n{}</browser_state>",
        url, title, step_info, elements_repr
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_builder() {
        let msg = UserMessageBuilder::new().build();
        assert!(msg.contains("<browser_state>"));
        assert!(msg.contains("Interactive Elements:"));
    }

    #[test]
    fn test_with_custom_instructions() {
        let msg = UserMessageBuilder::new()
            .with_custom_instructions(Some("Search for 'rust programming'"))
            .build();
        assert!(msg.contains("<custom_instructions>"));
        assert!(msg.contains("Search for 'rust programming'"));
    }
}
