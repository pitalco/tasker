mod builder;
mod extractor;
mod filter;
mod serializer;
pub mod types;

use anyhow::Result;
use chromiumoxide::Page;

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

    let selector_map = if let Some(mut tree) = tree {
        tracing::debug!("DOM extraction: tree built successfully");

        // Apply filters
        filter::filter_to_viewport(&mut tree, &viewport);
        filter::filter_by_paint_order(&mut tree, &viewport);
        filter::filter_contained_children(&mut tree, 0.95);
        filter::prune_tree(&mut tree);

        // Extract interactive elements
        let map = serializer::extract_interactive_elements(&tree);
        tracing::debug!("DOM extraction: found {} interactive elements", map.len());
        map
    } else {
        tracing::warn!("DOM extraction: tree building returned None");
        SelectorMap::new()
    };

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
