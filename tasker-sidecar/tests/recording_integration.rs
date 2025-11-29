//! Integration tests for the recording system.
//!
//! These tests launch real Chrome instances in headless mode and verify
//! that recording captures all user interactions correctly.
//!
//! Run with: cargo test --test recording_integration -- --test-threads=1

use std::time::Duration;
use tokio::time::sleep;

use tasker_sidecar::models::ActionType;
use tasker_sidecar::recording::BrowserRecorder;

/// Get file:// URL for the test page
fn test_page_url() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("file://{}/tests/fixtures/test_page.html", manifest_dir)
}

/// Wait for events to be captured (accounts for polling interval + processing)
async fn wait_for_events(ms: u64) {
    sleep(Duration::from_millis(ms)).await;
}

// ============================================================================
// Test 1: Click Capture
// ============================================================================

#[tokio::test]
async fn test_click_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    // Start recording in headless mode
    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await; // Wait for page load + script injection

    // Click the button using CDP
    recorder.browser.click("#btn-simple").await.unwrap();
    wait_for_events(800).await; // Wait for event capture (500ms poll + buffer)

    // Stop and get workflow
    let workflow = recorder.stop().await.unwrap();

    // Verify click was captured
    assert!(
        !workflow.steps.is_empty(),
        "Should capture at least one step, got {} steps",
        workflow.steps.len()
    );

    let click_step = &workflow.steps[0];
    assert_eq!(
        click_step.action.action_type,
        ActionType::Click,
        "First step should be a click"
    );
    assert!(
        click_step.action.selector.is_some(),
        "Click should have a selector"
    );
    assert!(
        click_step.name.contains("Click"),
        "Step name should contain 'Click'"
    );
}

// ============================================================================
// Test 2: Text Input Capture
// ============================================================================

#[tokio::test]
async fn test_text_input_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Type into input field (this clicks to focus, then types)
    recorder
        .browser
        .type_text("#input-text", "Hello World")
        .await
        .unwrap();
    wait_for_events(1000).await; // Wait for debounce (500ms) + capture

    let workflow = recorder.stop().await.unwrap();

    // Should have at least a type event
    let type_steps: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Type)
        .collect();

    assert!(
        !type_steps.is_empty(),
        "Should capture type event. All steps: {:?}",
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Verify the typed value
    let type_step = type_steps[0];
    assert_eq!(
        type_step.action.value.as_deref(),
        Some("Hello World"),
        "Should capture the typed text"
    );
}

// ============================================================================
// Test 3: Dropdown Selection Capture
// ============================================================================

#[tokio::test]
async fn test_dropdown_selection_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Select dropdown option
    recorder.browser.select("#dropdown", "opt2").await.unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    let select_steps: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Select)
        .collect();

    assert!(
        !select_steps.is_empty(),
        "Should capture select event. All steps: {:?}",
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    assert_eq!(
        select_steps[0].action.value.as_deref(),
        Some("opt2"),
        "Should capture selected value"
    );
}

// ============================================================================
// Test 4: Special Key Capture (Enter, Tab, Backspace)
// ============================================================================

#[tokio::test]
async fn test_special_key_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Focus input first
    recorder.browser.click("#form-input").await.unwrap();
    wait_for_events(300).await;

    // Press Enter key via JavaScript (simulates real keypress)
    recorder
        .browser
        .evaluate(
            r#"
        document.querySelector('#form-input').dispatchEvent(
            new KeyboardEvent('keydown', {key: 'Enter', bubbles: true})
        );
    "#,
        )
        .await
        .unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    // Find keypress step (stored as Custom action type)
    let key_steps: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Custom)
        .filter(|s| s.action.value.as_deref() == Some("Enter"))
        .collect();

    assert!(
        !key_steps.is_empty(),
        "Should capture Enter key. All steps: {:?}",
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

// ============================================================================
// Test 5: Session Lifecycle (Start, Pause, Resume, Stop)
// ============================================================================

#[tokio::test]
async fn test_session_lifecycle() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    // Start
    let session = recorder.start(&url, true, None).await.unwrap();
    assert_eq!(session.status, "recording", "Session should start as recording");

    wait_for_events(1000).await;

    // Pause
    recorder.pause().await.unwrap();
    let paused_session = recorder.session().await.unwrap();
    assert_eq!(paused_session.status, "paused", "Session should be paused");

    // Click while paused (should NOT be captured)
    recorder.browser.click("#btn-simple").await.unwrap();
    wait_for_events(800).await;

    // Resume
    recorder.resume().await.unwrap();
    let resumed_session = recorder.session().await.unwrap();
    assert_eq!(
        resumed_session.status, "recording",
        "Session should be recording after resume"
    );

    // Click while recording (SHOULD be captured)
    recorder.browser.click("#btn-submit").await.unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    // Should only have ONE click (the resumed one)
    let clicks: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Click)
        .collect();

    assert_eq!(
        clicks.len(),
        1,
        "Only post-resume click should be captured, got {} clicks",
        clicks.len()
    );
}

// ============================================================================
// Test 6: Multiple Events in Sequence
// ============================================================================

#[tokio::test]
async fn test_multiple_events_sequence() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Perform sequence: click -> type -> select -> click
    recorder.browser.click("#input-text").await.unwrap();
    wait_for_events(400).await;

    recorder
        .browser
        .type_text("#input-text", "test@example.com")
        .await
        .unwrap();
    wait_for_events(800).await; // Wait for debounce

    recorder.browser.select("#dropdown", "opt3").await.unwrap();
    wait_for_events(400).await;

    recorder.browser.click("#btn-submit").await.unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    // Verify we have multiple events
    assert!(
        workflow.steps.len() >= 3,
        "Should capture multiple events, got {} steps: {:?}",
        workflow.steps.len(),
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Steps should be in sequential order
    for (i, step) in workflow.steps.iter().enumerate() {
        assert_eq!(
            step.order,
            (i + 1) as i32,
            "Step {} should have order {}, got {}",
            i,
            i + 1,
            step.order
        );
    }
}

// ============================================================================
// Test 7: Cancel Recording
// ============================================================================

#[tokio::test]
async fn test_cancel_recording() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1000).await;

    // Perform some actions
    recorder.browser.click("#btn-simple").await.unwrap();
    wait_for_events(400).await;

    // Cancel instead of stop
    recorder.cancel().await.unwrap();

    // Session should be cleared
    assert!(
        recorder.session().await.is_none(),
        "Session should be cleared after cancel"
    );
}

// ============================================================================
// Test 8: Tab Key Capture
// ============================================================================

#[tokio::test]
async fn test_tab_key_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Focus input first
    recorder.browser.click("#input-text").await.unwrap();
    wait_for_events(300).await;

    // Press Tab key
    recorder
        .browser
        .evaluate(
            r#"
        document.querySelector('#input-text').dispatchEvent(
            new KeyboardEvent('keydown', {key: 'Tab', bubbles: true})
        );
    "#,
        )
        .await
        .unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    // Find Tab keypress
    let tab_steps: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Custom)
        .filter(|s| s.action.value.as_deref() == Some("Tab"))
        .collect();

    assert!(
        !tab_steps.is_empty(),
        "Should capture Tab key. All steps: {:?}",
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

// ============================================================================
// Test 9: Backspace Key Capture
// ============================================================================

#[tokio::test]
async fn test_backspace_key_capture() {
    let recorder = BrowserRecorder::new();
    let url = test_page_url();

    let _session = recorder.start(&url, true, None).await.unwrap();
    wait_for_events(1500).await;

    // Focus input first
    recorder.browser.click("#input-text").await.unwrap();
    wait_for_events(300).await;

    // Press Backspace key
    recorder
        .browser
        .evaluate(
            r#"
        document.querySelector('#input-text').dispatchEvent(
            new KeyboardEvent('keydown', {key: 'Backspace', bubbles: true})
        );
    "#,
        )
        .await
        .unwrap();
    wait_for_events(800).await;

    let workflow = recorder.stop().await.unwrap();

    // Find Backspace keypress
    let backspace_steps: Vec<_> = workflow
        .steps
        .iter()
        .filter(|s| s.action.action_type == ActionType::Custom)
        .filter(|s| s.action.value.as_deref() == Some("Backspace"))
        .collect();

    assert!(
        !backspace_steps.is_empty(),
        "Should capture Backspace key. All steps: {:?}",
        workflow.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}
