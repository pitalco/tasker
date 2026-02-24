use crate::tools::Memory;

/// Builds the user message for each LLM turn
pub struct UserMessageBuilder {
    custom_instructions: Option<String>,
    memories: Vec<Memory>,
    display_info: String,
    active_window: String,
    elements_text: String,
    step_number: Option<usize>,
    max_steps: Option<usize>,
}

impl UserMessageBuilder {
    pub fn new() -> Self {
        Self {
            custom_instructions: None,
            memories: Vec::new(),
            display_info: String::new(),
            active_window: String::new(),
            elements_text: String::new(),
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

    /// Set the current desktop state
    pub fn with_desktop_state(
        mut self,
        display_info: &str,
        active_window: &str,
        elements_text: &str,
    ) -> Self {
        self.display_info = display_info.to_string();
        self.active_window = active_window.to_string();
        self.elements_text = elements_text.to_string();
        self
    }

    /// Build the user message text (without screenshot - that's added separately)
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

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

        // Add desktop state (always present)
        parts.push(format_desktop_state(
            &self.display_info,
            &self.active_window,
            &self.elements_text,
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

/// Format desktop state with indexed elements
fn format_desktop_state(
    display_info: &str,
    active_window: &str,
    elements_text: &str,
    step_number: Option<usize>,
    max_steps: Option<usize>,
) -> String {
    let step_info = match (step_number, max_steps) {
        (Some(step), Some(max)) => format!("\nStep: {}/{}", step, max),
        _ => String::new(),
    };
    format!(
        "<desktop_state>\nScreen: {}\nActive window: \"{}\"{}\n\nInteractive Elements:\n{}\n\nUse click_element(index) for precise clicks. Use desktop_click(x, y) only if the element is not in the list above.\n</desktop_state>",
        display_info, active_window, step_info, elements_text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_builder() {
        let msg = UserMessageBuilder::new().build();
        assert!(msg.contains("<desktop_state>"));
        assert!(msg.contains("Interactive Elements:"));
    }

    #[test]
    fn test_with_custom_instructions() {
        let msg = UserMessageBuilder::new()
            .with_custom_instructions(Some("Open notepad and type hello"))
            .build();
        assert!(msg.contains("<custom_instructions>"));
        assert!(msg.contains("Open notepad and type hello"));
    }

    #[test]
    fn test_with_desktop_state() {
        let msg = UserMessageBuilder::new()
            .with_desktop_state(
                "1280x720 (scaled from 1920x1080)",
                "Notepad - readme.txt",
                "[1] Button \"File\" at (20, 5)\n[2] Button \"Edit\" at (70, 5)",
            )
            .with_step_info(3, 50)
            .build();
        assert!(msg.contains("Screen: 1280x720"));
        assert!(msg.contains("Notepad - readme.txt"));
        assert!(msg.contains("Step: 3/50"));
        assert!(msg.contains("[1] Button \"File\""));
    }
}
