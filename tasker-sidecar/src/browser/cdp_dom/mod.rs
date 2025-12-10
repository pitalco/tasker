mod builder;
mod extractor;
mod filter;
mod serializer;
pub mod types;

use anyhow::Result;
use chromiumoxide::Page;
use std::collections::HashMap;

pub use types::{
    BackendNodeId, DOMExtractionResult, DOMRect, ElementIndex, SelectorMap, SimplifiedElement,
};

/// Extract DOM using CDP and return structured result for LLM
pub async fn extract_dom(page: &Page) -> Result<DOMExtractionResult> {
    // Get page info
    let (url, title) = extractor::get_page_info(page).await?;
    tracing::debug!("DOM extraction: url={}, title={}", url, title);

    // Extract raw CDP trees
    let raw_trees = extractor::extract_trees(page).await?;
    let viewport = raw_trees.viewport.clone();
    tracing::debug!("DOM extraction: viewport={:?}", viewport);

    // Build enhanced tree
    let tree = builder::build_enhanced_tree(raw_trees);

    let mut selector_map = if let Some(mut tree) = tree {
        tracing::debug!("DOM extraction: tree built successfully");

        // Apply filters
        filter::filter_to_viewport(&mut tree, &viewport);
        filter::filter_by_paint_order(&mut tree, &viewport);
        filter::filter_contained_children(&mut tree, 0.99);
        filter::prune_tree(&mut tree);

        // Extract interactive elements
        let map = serializer::extract_interactive_elements(&tree);
        tracing::debug!("DOM extraction: found {} interactive elements", map.len());
        map
    } else {
        tracing::warn!("DOM extraction: tree building returned None");
        SelectorMap::new()
    };

    // Resolve actual bounding rects using CDP (DOMSnapshot layout data often missing)
    if let Err(e) = resolve_bounds_via_cdp(page, &mut selector_map, &viewport).await {
        tracing::warn!("Failed to resolve bounds via CDP: {}", e);
    }

    // Format for LLM
    let llm_representation = serializer::format_for_llm(&selector_map);

    Ok(DOMExtractionResult {
        selector_map,
        llm_representation,
        viewport,
        url,
        title,
    })
}

/// Resolve element bounds using CDP DOM.getBoxModel
/// This is more reliable than CDP's DOMSnapshot bounds
async fn resolve_bounds_via_cdp(
    page: &Page,
    selector_map: &mut SelectorMap,
    viewport: &DOMRect,
) -> Result<()> {
    if selector_map.ordered_elements.is_empty() {
        return Ok(());
    }

    // Check how many elements have valid bounds already
    let valid_bounds_count = selector_map
        .ordered_elements
        .iter()
        .filter(|e| e.bounds.width > 0.0 && e.bounds.height > 0.0)
        .count();

    tracing::debug!(
        "Bounds check: {}/{} elements have valid bounds",
        valid_bounds_count,
        selector_map.ordered_elements.len()
    );

    // If no elements have valid bounds, resolve them via CDP
    if valid_bounds_count == 0 {
        tracing::debug!("No valid bounds found, resolving via CDP DOM.getBoxModel...");

        let mut bounds_map: HashMap<BackendNodeId, DOMRect> = HashMap::new();
        let mut success_count = 0;

        // Resolve bounds for all elements (this may be slow for large pages)
        for elem in selector_map.ordered_elements.iter() {
            if let Ok(rect) = get_element_rect_via_cdp(page, elem.backend_node_id).await {
                if rect.width > 0.0 && rect.height > 0.0 {
                    bounds_map.insert(elem.backend_node_id, rect);
                    success_count += 1;
                }
            }
        }

        // Apply resolved bounds
        for elem in &mut selector_map.ordered_elements {
            if let Some(rect) = bounds_map.get(&elem.backend_node_id) {
                elem.bounds = rect.clone();
            }
        }

        tracing::debug!("Resolved {} element bounds via CDP", success_count);
    }

    // Now filter to only elements in viewport with valid bounds
    let before_count = selector_map.ordered_elements.len();
    selector_map.ordered_elements.retain(|elem| {
        let has_bounds = elem.bounds.width > 0.0 && elem.bounds.height > 0.0;
        if !has_bounds {
            return false; // Remove elements without valid bounds (hidden/not rendered)
        }

        // Check if element is in viewport
        let in_viewport = elem.bounds.y < viewport.height
            && elem.bounds.y + elem.bounds.height > 0.0
            && elem.bounds.x < viewport.width
            && elem.bounds.x + elem.bounds.width > 0.0;

        in_viewport
    });

    let after_count = selector_map.ordered_elements.len();
    tracing::info!(
        "Viewport filter: {} -> {} elements ({} removed)",
        before_count,
        after_count,
        before_count - after_count
    );

    // Rebuild the index maps
    selector_map.index_to_backend_id.clear();
    selector_map.backend_id_to_element.clear();
    for (i, elem) in selector_map.ordered_elements.iter_mut().enumerate() {
        elem.index = (i + 1) as ElementIndex;
        selector_map
            .index_to_backend_id
            .insert(elem.index, elem.backend_node_id);
        selector_map
            .backend_id_to_element
            .insert(elem.backend_node_id, elem.clone());
    }

    Ok(())
}

/// Get element bounding rect using CDP DOM.getBoxModel
async fn get_element_rect_via_cdp(page: &Page, backend_node_id: BackendNodeId) -> Result<DOMRect> {
    use chromiumoxide::cdp::browser_protocol::dom::GetBoxModelParams;

    let params = GetBoxModelParams {
        node_id: None,
        backend_node_id: Some(chromiumoxide::cdp::browser_protocol::dom::BackendNodeId::new(backend_node_id)),
        object_id: None,
    };

    let result = page.execute(params).await?;
    let model = result.result.model;

    // content quad inner values - Quad is a Vec<f64> with 8 values [x1,y1, x2,y2, x3,y3, x4,y4]
    let content = model.content.inner();
    if content.len() >= 8 {
        let x = content[0];
        let y = content[1];
        let width = content[2] - content[0];
        let height = content[5] - content[1];

        Ok(DOMRect {
            x,
            y,
            width,
            height,
        })
    } else {
        Ok(DOMRect::default())
    }
}
