//! Tauri wiring for unified relationship / lineage edges.

use std::path::Path;

use lattice_commands::{list_relationship_edges, RelationshipEdge, RelationshipKind};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRelationshipEdgesRequest {
    pub root: String,
    #[serde(default)]
    pub focus_path: Option<String>,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
}

fn parse_kinds(kinds: Option<&[String]>) -> Result<Option<Vec<RelationshipKind>>, String> {
    let Some(raw) = kinds else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    let mut parsed = Vec::with_capacity(raw.len());
    for kind in raw {
        let Some(value) = RelationshipKind::parse(kind) else {
            return Err(format!("unsupported relationship kind: {kind}"));
        };
        parsed.push(value);
    }
    Ok(Some(parsed))
}

/// List relationship edges for Inspect graph (1-hop when `focusPath` is set).
#[tauri::command]
pub fn list_relationship_edges_cmd(
    request: ListRelationshipEdgesRequest,
) -> Result<Vec<RelationshipEdge>, String> {
    let kinds = parse_kinds(request.kinds.as_deref())?;
    list_relationship_edges(
        Path::new(&request.root),
        request.focus_path.as_deref(),
        kinds.as_deref(),
    )
}
