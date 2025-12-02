use std::collections::HashMap;

use super::types::{
    AXNodeData, BackendNodeId, DOMRect, EnhancedDOMNode, LayoutData, RawCDPTrees,
};

/// Build enhanced DOM tree from raw CDP data
pub fn build_enhanced_tree(raw: RawCDPTrees) -> Option<EnhancedDOMNode> {
    // Build lookup tables for AX and layout data
    let ax_lookup = build_ax_lookup(&raw.ax_nodes);
    let layout_lookup = build_layout_lookup(&raw.snapshot);

    // Parse and build the DOM tree
    build_node_from_json(&raw.dom_root, &ax_lookup, &layout_lookup, &raw.viewport, false)
}

/// Build AX node lookup by backend_node_id
fn build_ax_lookup(ax_nodes: &Option<Vec<serde_json::Value>>) -> HashMap<BackendNodeId, AXNodeData> {
    let mut lookup = HashMap::new();

    if let Some(nodes) = ax_nodes {
        for node in nodes {
            if let Some(backend_id) = node.get("backendDOMNodeId").and_then(|v| v.as_i64()) {
                let ax_data = parse_ax_node(node);
                lookup.insert(backend_id, ax_data);
            }
        }
    }

    lookup
}

/// Parse an AX node from JSON
fn parse_ax_node(node: &serde_json::Value) -> AXNodeData {
    let mut data = AXNodeData::default();

    // Role
    if let Some(role) = node.get("role").and_then(|r| r.get("value")).and_then(|v| v.as_str()) {
        data.role = Some(role.to_string());
    }

    // Name
    if let Some(name) = node.get("name").and_then(|n| n.get("value")).and_then(|v| v.as_str()) {
        data.name = Some(name.to_string());
    }

    // Description
    if let Some(desc) = node.get("description").and_then(|d| d.get("value")).and_then(|v| v.as_str()) {
        data.description = Some(desc.to_string());
    }

    // Value
    if let Some(val) = node.get("value").and_then(|v| v.get("value")).and_then(|v| v.as_str()) {
        data.value = Some(val.to_string());
    }

    // Properties
    if let Some(props) = node.get("properties").and_then(|p| p.as_array()) {
        for prop in props {
            let name = prop.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let value = prop.get("value").and_then(|v| v.get("value"));

            match name {
                "checked" => data.checked = value.and_then(|v| v.as_bool()),
                "selected" => data.selected = value.and_then(|v| v.as_bool()),
                "disabled" => data.disabled = value.and_then(|v| v.as_bool()),
                "expanded" => data.expanded = value.and_then(|v| v.as_bool()),
                "focusable" => data.focusable = value.and_then(|v| v.as_bool()).unwrap_or(false),
                "focused" => data.focused = value.and_then(|v| v.as_bool()).unwrap_or(false),
                "required" => data.required = value.and_then(|v| v.as_bool()).unwrap_or(false),
                "readonly" => data.readonly = value.and_then(|v| v.as_bool()).unwrap_or(false),
                _ => {}
            }
        }
    }

    data
}

/// Build layout lookup from DOM snapshot
fn build_layout_lookup(snapshot: &Option<serde_json::Value>) -> HashMap<BackendNodeId, LayoutData> {
    let mut lookup = HashMap::new();

    let snapshot = match snapshot {
        Some(s) => s,
        None => return lookup,
    };

    // DOMSnapshot returns arrays that need to be correlated by index
    let documents = snapshot.get("documents").and_then(|d| d.as_array());

    if let Some(docs) = documents {
        for doc in docs {
            // Get the arrays we need
            let backend_node_ids = doc.get("nodes").and_then(|n| n.get("backendNodeId")).and_then(|b| b.as_array());
            let layout_indices = doc.get("nodes").and_then(|n| n.get("layoutNodeIndex")).and_then(|l| l.as_array());
            let layout_bounds = doc.get("layout").and_then(|l| l.get("bounds")).and_then(|b| b.as_array());
            let paint_orders = doc.get("layout").and_then(|l| l.get("paintOrders")).and_then(|p| p.as_array());
            let styles = doc.get("layout").and_then(|l| l.get("styles")).and_then(|s| s.as_array());
            let computed_style_strings = snapshot.get("strings").and_then(|s| s.as_array());

            if let (Some(backend_ids), Some(layout_idx)) = (backend_node_ids, layout_indices) {
                for (node_idx, backend_id_val) in backend_ids.iter().enumerate() {
                    let backend_id = backend_id_val.as_i64().unwrap_or(0);
                    if backend_id == 0 {
                        continue;
                    }

                    // Get layout index for this node
                    let layout_index = layout_idx.get(node_idx).and_then(|v| v.as_i64()).unwrap_or(-1);
                    if layout_index < 0 {
                        continue;
                    }
                    let layout_index = layout_index as usize;

                    let mut layout = LayoutData::default();

                    // Get bounds [x, y, width, height]
                    if let Some(bounds) = layout_bounds {
                        let base = layout_index * 4;
                        if base + 3 < bounds.len() {
                            layout.bounds = DOMRect {
                                x: bounds[base].as_f64().unwrap_or(0.0),
                                y: bounds[base + 1].as_f64().unwrap_or(0.0),
                                width: bounds[base + 2].as_f64().unwrap_or(0.0),
                                height: bounds[base + 3].as_f64().unwrap_or(0.0),
                            };
                        }
                    }

                    // Get paint order
                    if let Some(orders) = paint_orders {
                        if let Some(order) = orders.get(layout_index).and_then(|o| o.as_i64()) {
                            layout.paint_order = Some(order);
                        }
                    }

                    // Get computed styles
                    if let (Some(style_indices), Some(strings)) = (styles, computed_style_strings) {
                        // styles is array of style indices per layout node
                        // Each style is represented by pairs of string indices [name_idx, value_idx]
                        if let Some(style_arr) = style_indices.get(layout_index).and_then(|s| s.as_array()) {
                            for chunk in style_arr.chunks(2) {
                                if chunk.len() == 2 {
                                    let name_idx = chunk[0].as_i64().unwrap_or(-1) as usize;
                                    let value_idx = chunk[1].as_i64().unwrap_or(-1) as usize;

                                    let name = strings.get(name_idx).and_then(|s| s.as_str()).unwrap_or("");
                                    let value = strings.get(value_idx).and_then(|s| s.as_str()).unwrap_or("");

                                    match name {
                                        "display" => layout.display = Some(value.to_string()),
                                        "visibility" => layout.visibility = Some(value.to_string()),
                                        "opacity" => layout.opacity = Some(value.to_string()),
                                        "cursor" => {
                                            layout.cursor_style = Some(value.to_string());
                                            if value == "pointer" {
                                                layout.is_clickable = true;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    lookup.insert(backend_id, layout);
                }
            }
        }
    }

    lookup
}

/// Recursively build a node from JSON
fn build_node_from_json(
    json: &serde_json::Value,
    ax_lookup: &HashMap<BackendNodeId, AXNodeData>,
    layout_lookup: &HashMap<BackendNodeId, LayoutData>,
    viewport: &DOMRect,
    in_shadow: bool,
) -> Option<EnhancedDOMNode> {
    let node_type = json.get("nodeType").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    // Skip non-element nodes except text nodes (type 3) and document nodes (type 9)
    if node_type != 1 && node_type != 3 && node_type != 9 && node_type != 11 {
        return None;
    }

    let backend_node_id = json.get("backendNodeId").and_then(|v| v.as_i64()).unwrap_or(0);
    let node_id = json.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
    let node_name = json.get("nodeName").and_then(|v| v.as_str()).unwrap_or("");
    let tag_name = node_name.to_lowercase();

    // Skip non-content elements
    let skip_tags = ["script", "style", "noscript", "head", "meta", "link", "svg", "path"];
    if skip_tags.contains(&tag_name.as_str()) {
        return None;
    }

    // Parse attributes
    let mut attributes = HashMap::new();
    if let Some(attrs) = json.get("attributes").and_then(|a| a.as_array()) {
        for chunk in attrs.chunks(2) {
            if chunk.len() == 2 {
                let key = chunk[0].as_str().unwrap_or("");
                let value = chunk[1].as_str().unwrap_or("");
                attributes.insert(key.to_string(), value.to_string());
            }
        }
    }

    // Get text content for text nodes
    let text_content = if node_type == 3 {
        json.get("nodeValue")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Get AX and layout data
    let ax_data = ax_lookup.get(&backend_node_id).cloned();
    let layout = layout_lookup.get(&backend_node_id).cloned();

    // Check shadow root type
    let shadow_root_type = json
        .get("shadowRootType")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Determine visibility
    let is_visible = layout.as_ref().map(|l| l.is_visible()).unwrap_or(true);

    // Build children
    let mut children = Vec::new();

    // Regular children
    if let Some(child_nodes) = json.get("children").and_then(|c| c.as_array()) {
        for child in child_nodes {
            if let Some(child_node) = build_node_from_json(child, ax_lookup, layout_lookup, viewport, in_shadow) {
                children.push(child_node);
            }
        }
    }

    // Shadow roots
    if let Some(shadow_roots) = json.get("shadowRoots").and_then(|s| s.as_array()) {
        for shadow in shadow_roots {
            if let Some(shadow_node) = build_node_from_json(shadow, ax_lookup, layout_lookup, viewport, true) {
                children.push(shadow_node);
            }
        }
    }

    // Content document (iframes)
    let content_document_backend_id = json
        .get("contentDocument")
        .and_then(|d| d.get("backendNodeId"))
        .and_then(|v| v.as_i64());

    if let Some(content_doc) = json.get("contentDocument") {
        if let Some(doc_node) = build_node_from_json(content_doc, ax_lookup, layout_lookup, viewport, in_shadow) {
            children.push(doc_node);
        }
    }

    let mut node = EnhancedDOMNode {
        backend_node_id,
        node_id,
        node_type,
        tag_name,
        attributes,
        text_content,
        ax_data,
        layout,
        shadow_root_type,
        frame_id: json.get("frameId").and_then(|f| f.as_str()).map(|s| s.to_string()),
        content_document_backend_id,
        children,
        is_interactive: false,
        is_visible,
        is_obscured: false,
        in_shadow_dom: in_shadow,
    };

    // Compute interactivity
    node.is_interactive = node.compute_interactivity();

    Some(node)
}
