use serde::{Deserialize, Serialize};

/// Lightweight schema summary for JSON control messages alongside IPC bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaMeta {
    pub fields: Vec<FieldMeta>,
}

/// One column in [`SchemaMeta`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}
