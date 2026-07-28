use serde::{Deserialize, Serialize};

/// Default element kind for chunk-level search rows.
pub const DEFAULT_ELEMENT_KIND: &str = "chunk";

/// Stable identifier for the workspace search-elements dataset.
pub const SEARCH_ELEMENTS_DATASET_ID: &str = "search-elements";

/// Pipeline identity for search-elements vector projection writes.
pub const SEARCH_ELEMENTS_PIPELINE_VERSION: &str = "search-elements-v1";

/// Reference to a multimodal dataset within a workspace workload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetRef {
    pub id: String,
    pub name: String,
}

impl DatasetRef {
    /// The canonical search-elements dataset for a workspace.
    pub fn search_elements() -> Self {
        Self {
            id: SEARCH_ELEMENTS_DATASET_ID.to_string(),
            name: SEARCH_ELEMENTS_DATASET_ID.to_string(),
        }
    }
}

/// One searchable element row aligned with the search-elements schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchElementRow {
    /// Stable chunk identity; equals `chunk_id` in `lattice-index`.
    pub element_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub resource_version_id: Option<String>,
    #[serde(default = "default_element_kind")]
    pub element_kind: String,
    pub ordinal: i64,
    pub text: String,
    pub embedding: Vec<f32>,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub content_hash: String,
    pub embedding_model: String,
    pub embedding_version: String,
    pub namespace_key: String,
    pub dims: u32,
    pub created_at_ms: i64,
}

fn default_element_kind() -> String {
    DEFAULT_ELEMENT_KIND.to_string()
}

impl SearchElementRow {
    pub fn new(
        element_id: impl Into<String>,
        workspace_id: impl Into<String>,
        resource_id: impl Into<String>,
        ordinal: i64,
        embedding: Vec<f32>,
        namespace_key: impl Into<String>,
        dims: u32,
        created_at_ms: i64,
    ) -> Self {
        Self {
            element_id: element_id.into(),
            workspace_id: workspace_id.into(),
            resource_id: resource_id.into(),
            resource_version_id: None,
            element_kind: DEFAULT_ELEMENT_KIND.to_string(),
            ordinal,
            text: String::new(),
            embedding,
            source_start_byte: 0,
            source_end_byte: 0,
            content_hash: String::new(),
            embedding_model: String::new(),
            embedding_version: String::new(),
            namespace_key: namespace_key.into(),
            dims,
            created_at_ms,
        }
    }
}

/// Batch of search-element rows for append/upsert.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SearchElementBatch {
    pub rows: Vec<SearchElementRow>,
}

impl SearchElementBatch {
    pub fn new(rows: Vec<SearchElementRow>) -> Self {
        Self { rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl From<Vec<SearchElementRow>> for SearchElementBatch {
    fn from(rows: Vec<SearchElementRow>) -> Self {
        Self { rows }
    }
}

/// Vector search request scoped to one embedding namespace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub namespace_key: String,
    pub query_vector: Vec<f32>,
    pub limit: usize,
}

/// One vector search hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub element_id: String,
    pub score: f32,
    pub row: Option<SearchElementRow>,
}

/// Vector search results for a single request.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
}

/// Snapshot metadata for a dataset at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSnapshot {
    pub dataset_id: String,
    /// Lance dataset version placeholder until T2 wires real metadata.
    pub lance_version: u64,
    pub created_at_ms: i64,
}

/// Fingerprinted snapshot tying Lance vectors to source chunk identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedDatasetSnapshot {
    pub dataset_id: String,
    pub lance_version: u64,
    pub namespace_key: String,
    /// Stable fingerprint of source chunk identities+hashes at snapshot time.
    pub source_fingerprint: String,
    pub pipeline_version: String,
    pub created_at_ms: i64,
}

/// Opaque commit token returned after mutating operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_element_row_serde_round_trip() {
        let row = SearchElementRow {
            element_id: "chunk-1".to_string(),
            workspace_id: "local".to_string(),
            resource_id: "42".to_string(),
            resource_version_id: Some("abc123".to_string()),
            element_kind: DEFAULT_ELEMENT_KIND.to_string(),
            ordinal: 0,
            text: "hello".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            source_start_byte: 10,
            source_end_byte: 15,
            content_hash: "deadbeef".to_string(),
            embedding_model: "qwen3-embedding".to_string(),
            embedding_version: "rev1".to_string(),
            namespace_key: "default".to_string(),
            dims: 3,
            created_at_ms: 1_700_000_000_000,
        };

        let json = serde_json::to_string(&row).expect("serialize row");
        let decoded: SearchElementRow = serde_json::from_str(&json).expect("deserialize row");
        assert_eq!(decoded, row);
    }

    #[test]
    fn search_element_row_defaults_element_kind() {
        let json = r#"{
            "element_id": "chunk-1",
            "workspace_id": "local",
            "resource_id": "42",
            "resource_version_id": null,
            "ordinal": 0,
            "text": "",
            "embedding": [1.0],
            "source_start_byte": 0,
            "source_end_byte": 0,
            "content_hash": "",
            "embedding_model": "",
            "embedding_version": "",
            "namespace_key": "default",
            "dims": 1,
            "created_at_ms": 0
        }"#;

        let row: SearchElementRow = serde_json::from_str(json).expect("deserialize row");
        assert_eq!(row.element_kind, DEFAULT_ELEMENT_KIND);
    }
}
