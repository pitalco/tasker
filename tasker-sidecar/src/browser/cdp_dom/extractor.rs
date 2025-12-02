use anyhow::{anyhow, Result};
use chromiumoxide::cdp::browser_protocol::dom::GetDocumentParams;
use chromiumoxide::cdp::browser_protocol::dom_snapshot::CaptureSnapshotParams;
use chromiumoxide::cdp::browser_protocol::accessibility::GetFullAxTreeParams;
use chromiumoxide::cdp::browser_protocol::page::GetLayoutMetricsParams;
use chromiumoxide::Page;
use std::time::Duration;
use tokio::time::timeout;

use super::types::{DOMRect, RawCDPTrees};

const CDP_TIMEOUT: Duration = Duration::from_secs(10);

/// Extract DOM, AX tree, and snapshot in parallel
pub async fn extract_trees(page: &Page) -> Result<RawCDPTrees> {
    // Execute all CDP calls in parallel
    let (dom_result, snapshot_result, ax_result, metrics_result) = tokio::join!(
        get_dom_tree(page),
        get_dom_snapshot(page),
        get_ax_tree(page),
        get_layout_metrics(page),
    );

    // DOM tree is required
    let dom = dom_result?;

    // Snapshot and AX are optional (fallback gracefully)
    let snapshot = snapshot_result.ok();
    let ax = ax_result.ok();

    // Get viewport
    let viewport = match metrics_result {
        Ok(metrics) => {
            let rect = &metrics.css_visual_viewport;
            DOMRect {
                x: rect.page_x,
                y: rect.page_y,
                width: rect.client_width,
                height: rect.client_height,
            }
        }
        Err(_) => DOMRect::default_viewport(),
    };

    Ok(RawCDPTrees {
        dom_root: serde_json::to_value(&dom.root)?,
        snapshot: snapshot.map(|s| serde_json::to_value(&s)).transpose()?,
        ax_nodes: ax.map(|a| {
            a.nodes
                .into_iter()
                .filter_map(|n| serde_json::to_value(&n).ok())
                .collect()
        }),
        viewport,
    })
}

/// Get full DOM tree with shadow DOM piercing
async fn get_dom_tree(page: &Page) -> Result<chromiumoxide::cdp::browser_protocol::dom::GetDocumentReturns> {
    let params = GetDocumentParams {
        depth: Some(-1), // Full tree depth
        pierce: Some(true), // Pierce through shadow DOM
    };

    let result = timeout(CDP_TIMEOUT, page.execute(params))
        .await
        .map_err(|_| anyhow!("DOM.getDocument timeout"))?
        .map_err(|e| anyhow!("DOM.getDocument failed: {}", e))?;

    Ok(result.result)
}

/// Capture DOM snapshot with layout and paint order info
async fn get_dom_snapshot(page: &Page) -> Result<chromiumoxide::cdp::browser_protocol::dom_snapshot::CaptureSnapshotReturns> {
    let params = CaptureSnapshotParams {
        computed_styles: vec![
            "display".to_string(),
            "visibility".to_string(),
            "opacity".to_string(),
            "pointer-events".to_string(),
            "cursor".to_string(),
            "position".to_string(),
            "z-index".to_string(),
            "overflow".to_string(),
        ],
        include_paint_order: Some(true),
        include_dom_rects: Some(true),
        include_blended_background_colors: None,
        include_text_color_opacities: None,
    };

    let result = timeout(CDP_TIMEOUT, page.execute(params))
        .await
        .map_err(|_| anyhow!("DOMSnapshot.captureSnapshot timeout"))?
        .map_err(|e| anyhow!("DOMSnapshot.captureSnapshot failed: {}", e))?;

    Ok(result.result)
}

/// Get full accessibility tree
async fn get_ax_tree(page: &Page) -> Result<chromiumoxide::cdp::browser_protocol::accessibility::GetFullAxTreeReturns> {
    let params = GetFullAxTreeParams {
        depth: Some(-1),
        frame_id: None,
    };

    let result = timeout(CDP_TIMEOUT, page.execute(params))
        .await
        .map_err(|_| anyhow!("Accessibility.getFullAXTree timeout"))?
        .map_err(|e| anyhow!("Accessibility.getFullAXTree failed: {}", e))?;

    Ok(result.result)
}

/// Get layout metrics for viewport info
async fn get_layout_metrics(page: &Page) -> Result<chromiumoxide::cdp::browser_protocol::page::GetLayoutMetricsReturns> {
    let params = GetLayoutMetricsParams {};

    let result = timeout(CDP_TIMEOUT, page.execute(params))
        .await
        .map_err(|_| anyhow!("Page.getLayoutMetrics timeout"))?
        .map_err(|e| anyhow!("Page.getLayoutMetrics failed: {}", e))?;

    Ok(result.result)
}

/// Get URL and title from the page
pub async fn get_page_info(page: &Page) -> Result<(String, String)> {
    let url = page.url().await?.unwrap_or_default();

    // Get title via evaluate since there's no direct CDP method
    let title_result = page.evaluate("document.title").await;
    let title = match title_result {
        Ok(val) => val.into_value::<String>().unwrap_or_default(),
        Err(_) => String::new(),
    };

    Ok((url, title))
}
