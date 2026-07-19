use std::path::{Path, PathBuf};

use lattice_commands::{CanvasNodeMove, Command as SemanticCommand, CommandEngine, Transaction};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::{Deserialize, Serialize};

use crate::commands::{command_error_to_string, resolve_within_root};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDocument {
    pub content: String,
    pub revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasMutation {
    pub revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPlaceResourceRequest {
    pub root: String,
    pub canvas_path: String,
    pub base_revision: String,
    pub resource_path: String,
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasMoveNodeRequest {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasMoveNodesRequest {
    pub root: String,
    pub canvas_path: String,
    pub base_revision: String,
    pub nodes: Vec<CanvasMoveNodeRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasRemoveNodesRequest {
    pub root: String,
    pub canvas_path: String,
    pub base_revision: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasAddEdgeRequest {
    pub root: String,
    pub canvas_path: String,
    pub base_revision: String,
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
}

#[tauri::command]
pub fn read_canvas(root: String, canvas_path: String) -> Result<CanvasDocument, String> {
    let (canonical_root, canonical_path) = resolve_within_root(&root, &canvas_path)?;
    let content = std::fs::read_to_string(&canonical_path).map_err(|error| error.to_string())?;
    let revision = NativeWorkspaceStore::new(&canonical_root)
        .metadata(Path::new(&canvas_path))
        .map_err(|error| error.to_string())?
        .revision
        .hash;
    Ok(CanvasDocument { content, revision })
}

#[tauri::command]
pub fn canvas_place_resource(
    request: CanvasPlaceResourceRequest,
) -> Result<CanvasMutation, String> {
    let (canonical_root, _) = resolve_within_root(&request.root, &request.canvas_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!(
                "Place {} on canvas {}",
                request.resource_path, request.canvas_path
            ),
            vec![SemanticCommand::CanvasPlaceResource {
                path: PathBuf::from(&request.canvas_path),
                base_revision: request.base_revision,
                resource_path: PathBuf::from(&request.resource_path),
                node_id: request.node_id,
                x: request.x,
                y: request.y,
                width: request.width,
                height: request.height,
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(CanvasMutation {
        revision: receipt_revision(receipt)?,
    })
}

#[tauri::command]
pub fn canvas_move_nodes(request: CanvasMoveNodesRequest) -> Result<CanvasMutation, String> {
    let (canonical_root, _) = resolve_within_root(&request.root, &request.canvas_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let nodes = request
        .nodes
        .into_iter()
        .map(|node| CanvasNodeMove {
            id: node.id,
            x: node.x,
            y: node.y,
        })
        .collect();
    let receipt = engine
        .apply(Transaction::new(
            format!("Move nodes on canvas {}", request.canvas_path),
            vec![SemanticCommand::CanvasMoveNodes {
                path: PathBuf::from(&request.canvas_path),
                base_revision: request.base_revision,
                nodes,
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(CanvasMutation {
        revision: receipt_revision(receipt)?,
    })
}

#[tauri::command]
pub fn canvas_remove_nodes(request: CanvasRemoveNodesRequest) -> Result<CanvasMutation, String> {
    let (canonical_root, _) = resolve_within_root(&request.root, &request.canvas_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!("Remove nodes from canvas {}", request.canvas_path),
            vec![SemanticCommand::CanvasRemoveNodes {
                path: PathBuf::from(&request.canvas_path),
                base_revision: request.base_revision,
                node_ids: request.node_ids,
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(CanvasMutation {
        revision: receipt_revision(receipt)?,
    })
}

#[tauri::command]
pub fn canvas_add_edge(request: CanvasAddEdgeRequest) -> Result<CanvasMutation, String> {
    let (canonical_root, _) = resolve_within_root(&request.root, &request.canvas_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let receipt = engine
        .apply(Transaction::new(
            format!("Connect nodes on canvas {}", request.canvas_path),
            vec![SemanticCommand::CanvasAddEdge {
                path: PathBuf::from(&request.canvas_path),
                base_revision: request.base_revision,
                edge_id: request.edge_id,
                from_node: request.from_node,
                to_node: request.to_node,
            }],
        ))
        .map_err(command_error_to_string)?;
    Ok(CanvasMutation {
        revision: receipt_revision(receipt)?,
    })
}

fn receipt_revision(receipt: lattice_commands::TransactionReceipt) -> Result<String, String> {
    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "canvas command did not produce a resulting revision".to_string())
}
