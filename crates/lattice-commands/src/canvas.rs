//! JSON Canvas semantic patching.
//!
//! The command engine owns revision checks, journaling, and history. This
//! module only understands the open JSON Canvas shape and returns a complete
//! replacement payload after changing the requested semantic fields. Working
//! on `serde_json::Value` is deliberate: fields introduced by other JSON
//! Canvas producers remain in the document and on individual nodes/edges.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use crate::command::{CanvasNodeMove, CanvasNodeResize};
use crate::{Error, Result};

pub(crate) enum CanvasEdit {
    Place {
        resource_path: PathBuf,
        node_id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Move {
        nodes: Vec<CanvasNodeMove>,
    },
    Resize {
        nodes: Vec<CanvasNodeResize>,
    },
    Remove {
        node_ids: Vec<String>,
    },
    AddEdge {
        edge_id: String,
        from_node: String,
        to_node: String,
        from_side: Option<String>,
        to_side: Option<String>,
    },
    RemoveEdges {
        edge_ids: Vec<String>,
    },
    AddText {
        node_id: String,
        text: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    UpdateText {
        node_id: String,
        text: String,
    },
}

pub(crate) fn validate_canvas_path(path: &Path) -> Result<()> {
    validate_path(path, "canvas path")?;
    if path.extension().and_then(|extension| extension.to_str()) != Some("canvas") {
        return invalid(path, "canvas path must have a .canvas extension");
    }
    Ok(())
}

pub(crate) fn validate_path(path: &Path, label: &str) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return invalid(
            path,
            format!("{label} must be relative and stay within the workspace"),
        );
    }
    Ok(())
}

pub(crate) fn validate_edit(path: &Path, original: &[u8], edit: &CanvasEdit) -> Result<()> {
    validate_canvas_path(path)?;
    let document = parse_document(path, original)?;
    let nodes = canvas_nodes(path, &document)?;
    validate_existing_nodes(path, nodes)?;

    match edit {
        CanvasEdit::Place {
            resource_path,
            node_id,
            x,
            y,
            width,
            height,
        } => {
            validate_path(resource_path, "resource path")?;
            validate_id(path, node_id)?;
            validate_position(path, *x, *y, *width, *height)?;
            if nodes.iter().any(|node| node_id_of(node) == Some(node_id)) {
                return invalid(path, format!("node id {:?} already exists", node_id));
            }
        }
        CanvasEdit::Move { nodes: moves } => {
            if moves.is_empty() {
                return invalid(path, "at least one node is required");
            }
            let mut ids = HashSet::new();
            for node in moves {
                validate_id(path, &node.id)?;
                validate_finite(path, "x", node.x)?;
                validate_finite(path, "y", node.y)?;
                if !ids.insert(node.id.as_str()) {
                    return invalid(path, format!("duplicate node id {:?}", node.id));
                }
                if !nodes
                    .iter()
                    .any(|candidate| node_id_of(candidate) == Some(node.id.as_str()))
                {
                    return invalid(path, format!("node id {:?} does not exist", node.id));
                }
            }
        }
        CanvasEdit::Remove { node_ids } => {
            if node_ids.is_empty() {
                return invalid(path, "at least one node id is required");
            }
            let mut ids = HashSet::new();
            for id in node_ids {
                validate_id(path, id)?;
                if !ids.insert(id.as_str()) {
                    return invalid(path, format!("duplicate node id {:?}", id));
                }
                if !nodes.iter().any(|node| node_id_of(node) == Some(id)) {
                    return invalid(path, format!("node id {:?} does not exist", id));
                }
            }
        }
        CanvasEdit::AddEdge {
            edge_id,
            from_node,
            to_node,
            from_side,
            to_side,
        } => {
            validate_id(path, edge_id)?;
            validate_id(path, from_node)?;
            validate_id(path, to_node)?;
            validate_side(path, from_side)?;
            validate_side(path, to_side)?;
            if from_node == to_node {
                return invalid(path, "edge endpoints must be distinct nodes");
            }
            if !nodes
                .iter()
                .any(|node| node_id_of(node) == Some(from_node.as_str()))
            {
                return invalid(path, format!("fromNode {:?} does not exist", from_node));
            }
            if !nodes
                .iter()
                .any(|node| node_id_of(node) == Some(to_node.as_str()))
            {
                return invalid(path, format!("toNode {:?} does not exist", to_node));
            }
            if canvas_edges(&document)
                .into_iter()
                .flatten()
                .any(|edge| edge_id_of(edge) == Some(edge_id.as_str()))
            {
                return invalid(path, format!("edge id {:?} already exists", edge_id));
            }
        }
        CanvasEdit::Resize { nodes: resizes } => {
            if resizes.is_empty() {
                return invalid(path, "at least one node is required");
            }
            let mut ids = HashSet::new();
            for node in resizes {
                validate_id(path, &node.id)?;
                validate_finite(path, "width", node.width)?;
                validate_finite(path, "height", node.height)?;
                if node.width <= 0.0 || node.height <= 0.0 {
                    return invalid(path, "node width and height must be positive");
                }
                if !ids.insert(node.id.as_str()) {
                    return invalid(path, format!("duplicate node id {:?}", node.id));
                }
                if !nodes
                    .iter()
                    .any(|candidate| node_id_of(candidate) == Some(node.id.as_str()))
                {
                    return invalid(path, format!("node id {:?} does not exist", node.id));
                }
            }
        }
        CanvasEdit::RemoveEdges { edge_ids } => {
            if edge_ids.is_empty() {
                return invalid(path, "at least one edge id is required");
            }
            let mut ids = HashSet::new();
            let edges = canvas_edges(&document).map(Vec::as_slice).unwrap_or(&[]);
            for id in edge_ids {
                validate_id(path, id)?;
                if !ids.insert(id.as_str()) {
                    return invalid(path, format!("duplicate edge id {:?}", id));
                }
                if !edges.iter().any(|edge| edge_id_of(edge) == Some(id)) {
                    return invalid(path, format!("edge id {:?} does not exist", id));
                }
            }
        }
        CanvasEdit::AddText {
            node_id,
            text,
            x,
            y,
            width,
            height,
        } => {
            validate_id(path, node_id)?;
            validate_position(path, *x, *y, *width, *height)?;
            if text.trim().is_empty() {
                return invalid(path, "text nodes must not be empty");
            }
            if nodes.iter().any(|node| node_id_of(node) == Some(node_id)) {
                return invalid(path, format!("node id {:?} already exists", node_id));
            }
        }
        CanvasEdit::UpdateText { node_id, text } => {
            validate_id(path, node_id)?;
            if text.trim().is_empty() {
                return invalid(path, "text nodes must not be empty");
            }
            let Some(node) = nodes.iter().find(|node| node_id_of(node) == Some(node_id)) else {
                return invalid(path, format!("node id {:?} does not exist", node_id));
            };
            if node.get("type").and_then(Value::as_str) != Some("text") {
                return invalid(path, format!("node id {:?} is not a text node", node_id));
            }
        }
    }
    Ok(())
}

pub(crate) fn patch(path: &Path, original: &[u8], edit: &CanvasEdit) -> Result<Vec<u8>> {
    validate_edit(path, original, edit)?;
    let mut document = parse_document(path, original)?;

    match edit {
        CanvasEdit::Place {
            resource_path,
            node_id,
            x,
            y,
            width,
            height,
        } => {
            let file = relative_resource_path(path, resource_path)?;
            canvas_nodes_mut(path, &mut document)?.push(json!({
                "id": node_id,
                "type": "file",
                "file": file,
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }));
        }
        CanvasEdit::Move { nodes: moves } => {
            let nodes = canvas_nodes_mut(path, &mut document)?;
            for CanvasNodeMove { id, x, y } in moves {
                let node = nodes
                    .iter_mut()
                    .find(|node| node_id_of(node) == Some(id.as_str()))
                    .ok_or_else(|| {
                        invalid_value(path, format!("node id {:?} does not exist", id))
                    })?;
                let object = node
                    .as_object_mut()
                    .ok_or_else(|| invalid_value(path, "canvas node must be an object"))?;
                object.insert("x".into(), json!(x));
                object.insert("y".into(), json!(y));
            }
        }
        CanvasEdit::Remove { node_ids } => {
            let ids: HashSet<&str> = node_ids.iter().map(String::as_str).collect();
            {
                let nodes = canvas_nodes_mut(path, &mut document)?;
                nodes.retain(|node| node_id_of(node).is_none_or(|id| !ids.contains(id)));
            }
            if let Some(edges) = document.get_mut("edges").and_then(Value::as_array_mut) {
                edges.retain(|edge| {
                    let from = edge.get("fromNode").and_then(Value::as_str);
                    let to = edge.get("toNode").and_then(Value::as_str);
                    !from.is_some_and(|id| ids.contains(id))
                        && !to.is_some_and(|id| ids.contains(id))
                });
            }
        }
        CanvasEdit::AddEdge {
            edge_id,
            from_node,
            to_node,
            from_side,
            to_side,
        } => {
            let edges = canvas_edges_mut(path, &mut document)?;
            let mut edge = json!({
                "id": edge_id,
                "fromNode": from_node,
                "toNode": to_node,
            });
            if let Some(side) = from_side {
                edge.as_object_mut()
                    .expect("edge object")
                    .insert("fromSide".into(), json!(side));
            }
            if let Some(side) = to_side {
                edge.as_object_mut()
                    .expect("edge object")
                    .insert("toSide".into(), json!(side));
            }
            edges.push(edge);
        }
        CanvasEdit::Resize { nodes: resizes } => {
            let nodes = canvas_nodes_mut(path, &mut document)?;
            for CanvasNodeResize { id, width, height } in resizes {
                let node = nodes
                    .iter_mut()
                    .find(|node| node_id_of(node) == Some(id.as_str()))
                    .ok_or_else(|| {
                        invalid_value(path, format!("node id {:?} does not exist", id))
                    })?;
                let object = node
                    .as_object_mut()
                    .ok_or_else(|| invalid_value(path, "canvas node must be an object"))?;
                object.insert("width".into(), json!(width));
                object.insert("height".into(), json!(height));
            }
        }
        CanvasEdit::RemoveEdges { edge_ids } => {
            let ids: HashSet<&str> = edge_ids.iter().map(String::as_str).collect();
            if let Some(edges) = document.get_mut("edges").and_then(Value::as_array_mut) {
                edges.retain(|edge| edge_id_of(edge).is_none_or(|id| !ids.contains(id)));
            }
        }
        CanvasEdit::AddText {
            node_id,
            text,
            x,
            y,
            width,
            height,
        } => {
            canvas_nodes_mut(path, &mut document)?.push(json!({
                "id": node_id,
                "type": "text",
                "text": text,
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }));
        }
        CanvasEdit::UpdateText { node_id, text } => {
            let nodes = canvas_nodes_mut(path, &mut document)?;
            let node = nodes
                .iter_mut()
                .find(|node| node_id_of(node) == Some(node_id.as_str()))
                .ok_or_else(|| {
                    invalid_value(path, format!("node id {:?} does not exist", node_id))
                })?;
            let object = node
                .as_object_mut()
                .ok_or_else(|| invalid_value(path, "canvas node must be an object"))?;
            object.insert("text".into(), json!(text));
        }
    }

    serde_json::to_vec_pretty(&document).map_err(Error::from)
}

fn parse_document(path: &Path, bytes: &[u8]) -> Result<Value> {
    let document: Value = serde_json::from_slice(bytes).map_err(|source| Error::InvalidCanvas {
        path: path.to_path_buf(),
        reason: format!("invalid JSON: {source}"),
    })?;
    if !document.is_object() {
        return invalid(path, "document must be a JSON object");
    }
    if !document.get("nodes").is_some_and(Value::is_array) {
        return invalid(path, "nodes must be an array");
    }
    if document.get("edges").is_some_and(|edges| !edges.is_array()) {
        return invalid(path, "edges must be an array when present");
    }
    Ok(document)
}

fn canvas_nodes<'a>(path: &Path, document: &'a Value) -> Result<&'a Vec<Value>> {
    document
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_value(path, "nodes must be an array"))
}

fn canvas_nodes_mut<'a>(path: &Path, document: &'a mut Value) -> Result<&'a mut Vec<Value>> {
    document
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid_value(path, "nodes must be an array"))
}

fn canvas_edges<'a>(document: &'a Value) -> Option<&'a Vec<Value>> {
    document.get("edges").and_then(Value::as_array)
}

fn canvas_edges_mut<'a>(path: &Path, document: &'a mut Value) -> Result<&'a mut Vec<Value>> {
    if document.get("edges").is_none() {
        document
            .as_object_mut()
            .ok_or_else(|| invalid_value(path, "document must be a JSON object"))?
            .insert("edges".into(), json!([]));
    }
    document
        .get_mut("edges")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid_value(path, "edges must be an array"))
}

fn validate_existing_nodes(path: &Path, nodes: &[Value]) -> Result<()> {
    let mut ids = HashSet::new();
    for node in nodes {
        let object = node
            .as_object()
            .ok_or_else(|| invalid_value(path, "every node must be an object"))?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_value(path, "every node must have a string id"))?;
        validate_id(path, id)?;
        if !ids.insert(id) {
            return invalid(path, format!("duplicate node id {:?}", id));
        }
        for field in ["x", "y", "width", "height"] {
            let value = object
                .get(field)
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid_value(path, format!("node {id:?} has invalid {field}")))?;
            validate_finite(path, field, value)?;
        }
    }
    Ok(())
}

fn validate_position(path: &Path, x: f64, y: f64, width: f64, height: f64) -> Result<()> {
    validate_finite(path, "x", x)?;
    validate_finite(path, "y", y)?;
    validate_finite(path, "width", width)?;
    validate_finite(path, "height", height)?;
    if width <= 0.0 || height <= 0.0 {
        return invalid(path, "node width and height must be positive");
    }
    Ok(())
}

fn validate_id(path: &Path, id: &str) -> Result<()> {
    if id.trim().is_empty() {
        invalid(path, "node ids must not be empty")
    } else {
        Ok(())
    }
}

fn validate_side(path: &Path, side: &Option<String>) -> Result<()> {
    match side.as_deref() {
        None => Ok(()),
        Some("top" | "right" | "bottom" | "left") => Ok(()),
        Some(other) => invalid(
            path,
            format!("side {other:?} must be one of top/right/bottom/left"),
        ),
    }
}

fn validate_finite(path: &Path, field: &str, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        invalid(path, format!("{field} must be finite"))
    }
}

fn node_id_of(node: &Value) -> Option<&str> {
    node.get("id").and_then(Value::as_str)
}

fn edge_id_of(edge: &Value) -> Option<&str> {
    edge.get("id").and_then(Value::as_str)
}

fn relative_resource_path(canvas: &Path, resource: &Path) -> Result<String> {
    let from = canvas.parent().unwrap_or_else(|| Path::new(""));
    let from = from.components().collect::<Vec<_>>();
    let to = resource.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &to[common..] {
        let Component::Normal(value) = component else {
            return invalid(Path::new("(resource)"), "resource path is not relative");
        };
        result.push(value);
    }
    Ok(result.to_string_lossy().replace('\\', "/"))
}

fn invalid<T>(path: &Path, reason: impl Into<String>) -> Result<T> {
    Err(Error::InvalidCanvas {
        path: path.to_path_buf(),
        reason: reason.into(),
    })
}

fn invalid_value(path: &Path, reason: impl Into<String>) -> Error {
    Error::InvalidCanvas {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}
