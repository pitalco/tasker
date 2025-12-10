use std::collections::HashSet;

use super::types::{BackendNodeId, DOMRect, EnhancedDOMNode};

/// Filter tree to mark obscured elements based on paint order
pub fn filter_by_paint_order(tree: &mut EnhancedDOMNode, viewport: &DOMRect) {
    // Collect all interactive elements with paint order and bounds
    let mut interactive_elements: Vec<(BackendNodeId, DOMRect, i64)> = Vec::new();
    collect_interactive_with_paint_order(tree, &mut interactive_elements);

    if interactive_elements.is_empty() {
        return;
    }

    // Sort by paint order descending (highest = on top)
    interactive_elements.sort_by(|a, b| b.2.cmp(&a.2));

    // Find obscured elements
    let mut obscured: HashSet<BackendNodeId> = HashSet::new();

    for i in 0..interactive_elements.len() {
        let (id, bounds, paint_order) = &interactive_elements[i];

        // Skip elements outside viewport
        if !viewport.intersects(bounds) {
            continue;
        }

        // Check if this element is significantly covered by higher paint order elements
        for (_, other_bounds, other_paint) in interactive_elements.iter().take(i) {

            if other_paint > paint_order {
                let overlap = bounds.intersection_area(other_bounds);
                let area = bounds.area();

                // If more than 80% covered, mark as obscured
                if area > 0.0 && overlap / area > 0.8 {
                    obscured.insert(*id);
                    break;
                }
            }
        }
    }

    // Mark obscured nodes in tree
    mark_obscured(tree, &obscured);
}

/// Collect interactive elements with their paint order
fn collect_interactive_with_paint_order(
    node: &EnhancedDOMNode,
    elements: &mut Vec<(BackendNodeId, DOMRect, i64)>,
) {
    if node.is_interactive && node.is_visible {
        if let Some(layout) = &node.layout {
            if let Some(paint_order) = layout.paint_order {
                elements.push((node.backend_node_id, layout.bounds.clone(), paint_order));
            }
        }
    }

    for child in &node.children {
        collect_interactive_with_paint_order(child, elements);
    }
}

/// Mark nodes as obscured
fn mark_obscured(node: &mut EnhancedDOMNode, obscured: &HashSet<BackendNodeId>) {
    if obscured.contains(&node.backend_node_id) {
        node.is_obscured = true;
    }

    for child in &mut node.children {
        mark_obscured(child, obscured);
    }
}

/// Filter out non-visible, non-interactive branches
pub fn prune_tree(node: &mut EnhancedDOMNode) -> bool {
    // Recursively prune children
    node.children.retain_mut(prune_tree);

    // Keep if:
    // 1. Interactive and visible and not obscured
    // 2. Has kept children
    // 3. Is a shadow host (may contain interactive content)
    // 4. Has text content (for context)
    let dominated_tags = ["style", "script", "noscript", "head", "meta", "link"];
    if dominated_tags.contains(&node.tag_name.as_str()) {
        return false;
    }

    let has_meaningful_text = node
        .text_content
        .as_ref()
        .map(|t| t.len() > 1)
        .unwrap_or(false);

    (node.is_interactive && node.is_visible && !node.is_obscured)
        || !node.children.is_empty()
        || node.shadow_root_type.is_some()
        || has_meaningful_text
}

/// Apply bounding box containment filtering
/// Removes interactive children that are fully contained within an interactive parent
pub fn filter_contained_children(node: &mut EnhancedDOMNode, threshold: f64) {
    filter_contained_recursive(node, None, threshold);
}

fn filter_contained_recursive(
    node: &mut EnhancedDOMNode,
    parent_bounds: Option<&DOMRect>,
    threshold: f64,
) {
    // Check if this node should be excluded because it's contained in parent
    if node.is_interactive && !is_exception_element(node) {
        if let (Some(parent), Some(layout)) = (parent_bounds, &node.layout) {
            let overlap = layout.bounds.intersection_area(parent);
            let area = layout.bounds.area();

            if area > 0.0 && overlap / area >= threshold {
                // This element is contained - mark as not interactive
                // (parent will handle the click)
                node.is_interactive = false;
            }
        }
    }

    // Determine bounds to propagate to children
    let propagate_bounds = if should_propagate_bounds(node) {
        node.layout.as_ref().map(|l| &l.bounds)
    } else {
        parent_bounds
    };

    // Process children
    for child in &mut node.children {
        filter_contained_recursive(child, propagate_bounds, threshold);
    }
}

/// Check if element should propagate bounds to children
fn should_propagate_bounds(node: &EnhancedDOMNode) -> bool {
    if !node.is_interactive {
        return false;
    }

    // Tags that typically wrap clickable content
    let propagating_tags = ["a", "button", "summary", "label"];
    // Roles that act as container interactions
    let propagating_roles = ["button", "link", "combobox", "menuitem", "tab", "listitem"];

    if propagating_tags.contains(&node.tag_name.as_str()) {
        return true;
    }

    if let Some(role) = node.attributes.get("role") {
        if propagating_roles.contains(&role.as_str()) {
            return true;
        }
    }

    false
}

/// Check if element is an exception that shouldn't be filtered
fn is_exception_element(node: &EnhancedDOMNode) -> bool {
    // Form elements need individual interaction
    let form_tags = ["input", "select", "textarea", "label", "option"];
    if form_tags.contains(&node.tag_name.as_str()) {
        return true;
    }

    // Interactive ARIA roles should be preserved
    let interactive_roles = [
        "button", "link", "checkbox", "radio", "menuitem",
        "tab", "textbox", "combobox", "slider", "switch",
        "searchbox", "spinbutton", "option",
    ];
    if let Some(role) = node.attributes.get("role") {
        if interactive_roles.contains(&role.as_str()) {
            return true;
        }
    }

    // Elements with explicit event handlers
    if node.attributes.contains_key("onclick")
        || node.attributes.contains_key("onmousedown")
        || node.attributes.contains_key("onmouseup")
        || node.attributes.contains_key("data-action")
        || node.attributes.contains_key("data-click")
    {
        return true;
    }

    // Elements with meaningful aria-label
    if node.attributes.get("aria-label").map(|a| !a.is_empty()).unwrap_or(false) {
        return true;
    }

    // Elements with aria-haspopup (dropdowns, menus)
    if node.attributes.contains_key("aria-haspopup") {
        return true;
    }

    false
}

/// Filter elements to only those in viewport
/// This filters to only elements with valid bounds that are within the viewport
pub fn filter_to_viewport(node: &mut EnhancedDOMNode, viewport: &DOMRect) {
    let (total, filtered) = mark_out_of_viewport(node, viewport, 0, 0);
    tracing::debug!(
        "Viewport filter: {} total interactive, {} filtered out, {} remaining",
        total,
        filtered,
        total - filtered
    );
}

fn mark_out_of_viewport(
    node: &mut EnhancedDOMNode,
    viewport: &DOMRect,
    mut total: usize,
    mut filtered: usize,
) -> (usize, usize) {
    if node.is_interactive {
        total += 1;

        if let Some(layout) = &node.layout {
            let bounds = &layout.bounds;

            // Filter out if:
            // 1. Has zero/invalid bounds
            // 2. Is completely outside viewport
            // 3. Is below the viewport fold (y >= viewport.height)
            let has_valid_bounds = bounds.width > 0.0 && bounds.height > 0.0;
            let in_viewport = viewport.intersects(bounds);
            let above_fold = bounds.y < viewport.height;

            if !has_valid_bounds || !in_viewport || !above_fold {
                node.is_visible = false;
                filtered += 1;
            }
        }
        // If no layout, keep the element (we can't determine position)
    }

    for child in &mut node.children {
        let (t, f) = mark_out_of_viewport(child, viewport, total, filtered);
        total = t;
        filtered = f;
    }

    (total, filtered)
}
