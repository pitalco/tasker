use anyhow::{anyhow, Result};

use super::manager::BrowserManager;
use crate::models::{ActionType, BrowserAction, ElementSelector, SelectorStrategy, StepResult};

/// Execute a browser action
pub async fn execute_action(
    browser: &BrowserManager,
    action: &BrowserAction,
    step_id: &str,
) -> Result<StepResult> {
    let start = std::time::Instant::now();

    let result: Result<()> = match action.action_type {
        ActionType::Navigate => {
            let url = action
                .url
                .as_ref()
                .ok_or_else(|| anyhow!("Navigate action requires URL"))?;
            browser.navigate(url).await
        }
        ActionType::Click => {
            let selector = build_selector(&action.selector)?;
            browser.click(&selector).await
        }
        ActionType::Type => {
            let selector = build_selector(&action.selector)?;
            let value = action
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("Type action requires value"))?;
            browser.type_text(&selector, value).await
        }
        ActionType::Scroll => {
            let (x, y) = if let Some(coords) = &action.coordinates {
                (coords.x, coords.y)
            } else {
                (0, 500) // Default scroll amount
            };
            browser.scroll(x, y).await
        }
        ActionType::Hover => {
            let selector = build_selector(&action.selector)?;
            browser.hover(&selector).await
        }
        ActionType::Select => {
            let selector = build_selector(&action.selector)?;
            let value = action
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("Select action requires value"))?;
            browser.select(&selector, value).await
        }
        ActionType::Wait => {
            let duration = action
                .options
                .get("duration_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(1000);
            browser.wait(duration).await
        }
        ActionType::Screenshot => {
            // Screenshot is handled separately, just return success
            Ok(())
        }
        ActionType::Extract => {
            // Extract is handled by the calling code
            Ok(())
        }
        ActionType::Custom => {
            // Custom actions are handled by AI agent
            Ok(())
        }
    };

    let duration_ms = start.elapsed().as_millis() as i32;

    match result {
        Ok(()) => Ok(StepResult::success(step_id.to_string(), duration_ms)),
        Err(e) => Ok(StepResult::failure(step_id.to_string(), e.to_string())),
    }
}

/// Build CSS selector from ElementSelector
fn build_selector(selector: &Option<ElementSelector>) -> Result<String> {
    let sel = selector
        .as_ref()
        .ok_or_else(|| anyhow!("Action requires selector"))?;

    match sel.strategy {
        SelectorStrategy::Css => Ok(sel.value.clone()),
        SelectorStrategy::Xpath => {
            // Chromiumoxide doesn't directly support XPath, convert to JS evaluation
            Ok(format!(
                r#"document.evaluate("{}", document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue"#,
                sel.value.replace('"', r#"\""#)
            ))
        }
        SelectorStrategy::Text => {
            // Find by visible text content
            Ok(format!(
                r#"//*[contains(text(), "{}")]"#,
                sel.value.replace('"', r#"\""#)
            ))
        }
        SelectorStrategy::AriaLabel => {
            Ok(format!(r#"[aria-label="{}"]"#, sel.value))
        }
        SelectorStrategy::TestId => {
            Ok(format!(r#"[data-testid="{}"]"#, sel.value))
        }
    }
}

/// Try multiple selectors until one works
pub async fn execute_with_fallback(
    browser: &BrowserManager,
    action: &BrowserAction,
    step_id: &str,
) -> Result<StepResult> {
    // Try primary selector first
    let result = execute_action(browser, action, step_id).await?;
    if result.success {
        return Ok(result);
    }

    // Try fallback selectors if primary failed
    if let Some(ref selector) = action.selector {
        for fallback in &selector.fallback_selectors {
            let mut fallback_action = action.clone();
            fallback_action.selector = Some(ElementSelector {
                strategy: match fallback.strategy.as_str() {
                    "css" => SelectorStrategy::Css,
                    "xpath" => SelectorStrategy::Xpath,
                    "text" => SelectorStrategy::Text,
                    "aria_label" => SelectorStrategy::AriaLabel,
                    "test_id" => SelectorStrategy::TestId,
                    _ => continue,
                },
                value: fallback.value.clone(),
                fallback_selectors: vec![],
            });

            let fallback_result = execute_action(browser, &fallback_action, step_id).await?;
            if fallback_result.success {
                return Ok(fallback_result);
            }
        }
    }

    // Return the original failure
    Ok(result)
}
