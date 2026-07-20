//! DuckDB-backed relation profiling (`SUMMARIZE` + row count).

use serde::{Deserialize, Serialize};

use crate::batch::{RecordBatch, ScalarValue};
use crate::error::Error;
use crate::DuckDbEngine;
use crate::Result;

/// Per-column analytical profile derived from DuckDB `SUMMARIZE`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnProfile {
    pub name: String,
    pub data_type: String,
    pub row_count: Option<u64>,
    pub null_percentage: Option<f64>,
    pub approx_distinct: Option<u64>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub avg: Option<f64>,
    pub std: Option<f64>,
    pub q25: Option<String>,
    pub q50: Option<String>,
    pub q75: Option<String>,
}

/// Compact profile for a DuckDB relation (subquery or table function).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationProfile {
    pub row_count: u64,
    pub columns: Vec<ColumnProfile>,
    /// SQL used for profiling (relation subquery only; excludes `SUMMARIZE`).
    pub relation_sql: String,
}

impl DuckDbEngine {
    /// Profile a relation defined by `relation_sql` (e.g. `read_parquet(...)` or `SELECT …`).
    ///
    /// Uses DuckDB `SUMMARIZE` for per-column null %, approximate distinct counts, and
    /// min/max/quantiles where the engine can compute them cheaply.
    pub fn profile_relation(&self, relation_sql: &str) -> Result<RelationProfile> {
        let trimmed = relation_sql.trim();
        if trimmed.is_empty() {
            return Err(Error::message("relation SQL must not be empty"));
        }

        let row_count = self.relation_row_count(trimmed)?;
        if row_count == 0 {
            return Ok(RelationProfile {
                row_count: 0,
                columns: Vec::new(),
                relation_sql: trimmed.to_string(),
            });
        }

        let summarize_sql = format!(
            "SUMMARIZE SELECT * FROM ({trimmed}) AS _lattice_rel"
        );
        let batch = self.query(&summarize_sql)?;
        let columns = summarize_batch_to_columns(&batch)?;

        Ok(RelationProfile {
            row_count,
            columns,
            relation_sql: trimmed.to_string(),
        })
    }

    fn relation_row_count(&self, relation_sql: &str) -> Result<u64> {
        let sql = format!("SELECT COUNT(*) AS n FROM ({relation_sql}) AS _lattice_rel");
        let batch = self.query(&sql)?;
        if batch.num_rows == 0 {
            return Ok(0);
        }
        match &batch.columns[0][0] {
            ScalarValue::Int64(n) if *n >= 0 => Ok(*n as u64),
            ScalarValue::Float64(n) if *n >= 0.0 => Ok(*n as u64),
            ScalarValue::Utf8(text) => text
                .parse::<u64>()
                .map_err(|_| Error::message(format!("unexpected count value: {text}"))),
            other => Err(Error::message(format!("unexpected count value: {other:?}"))),
        }
    }
}

fn summarize_batch_to_columns(batch: &RecordBatch) -> Result<Vec<ColumnProfile>> {
    if batch.num_rows == 0 {
        return Ok(Vec::new());
    }

    let column_index = |name: &str| -> Result<usize> {
        batch
            .schema
            .fields
            .iter()
            .position(|field| field.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::message(format!("SUMMARIZE missing column {name:?}")))
    };

    let name_idx = column_index("column_name")?;
    let type_idx = column_index("column_type")?;
    let min_idx = column_index("min")?;
    let max_idx = column_index("max")?;
    let approx_idx = column_index("approx_unique")?;
    let avg_idx = column_index("avg")?;
    let std_idx = column_index("std")?;
    let q25_idx = column_index("q25")?;
    let q50_idx = column_index("q50")?;
    let q75_idx = column_index("q75")?;
    let count_idx = column_index("count")?;
    let null_pct_idx = column_index("null_percentage")?;

    let mut columns = Vec::with_capacity(batch.num_rows);
    for row in 0..batch.num_rows {
        columns.push(ColumnProfile {
            name: scalar_to_string(&batch.columns[name_idx][row])?,
            data_type: scalar_to_string(&batch.columns[type_idx][row])?,
            row_count: scalar_to_u64(&batch.columns[count_idx][row]),
            null_percentage: scalar_to_f64(&batch.columns[null_pct_idx][row]),
            approx_distinct: scalar_to_u64(&batch.columns[approx_idx][row]),
            min: scalar_to_optional_string(&batch.columns[min_idx][row]),
            max: scalar_to_optional_string(&batch.columns[max_idx][row]),
            avg: scalar_to_f64(&batch.columns[avg_idx][row]),
            std: scalar_to_f64(&batch.columns[std_idx][row]),
            q25: scalar_to_optional_string(&batch.columns[q25_idx][row]),
            q50: scalar_to_optional_string(&batch.columns[q50_idx][row]),
            q75: scalar_to_optional_string(&batch.columns[q75_idx][row]),
        });
    }

    Ok(columns)
}

fn scalar_to_string(value: &ScalarValue) -> Result<String> {
    match value {
        ScalarValue::Null => Ok(String::new()),
        ScalarValue::Boolean(v) => Ok(v.to_string()),
        ScalarValue::Int64(v) => Ok(v.to_string()),
        ScalarValue::Float64(v) => Ok(v.to_string()),
        ScalarValue::Utf8(v) => Ok(v.clone()),
    }
}

fn scalar_to_optional_string(value: &ScalarValue) -> Option<String> {
    match value {
        ScalarValue::Null => None,
        other => scalar_to_string(other).ok(),
    }
}

fn scalar_to_u64(value: &ScalarValue) -> Option<u64> {
    match value {
        ScalarValue::Null => None,
        ScalarValue::Int64(v) if *v >= 0 => Some(*v as u64),
        ScalarValue::Float64(v) if *v >= 0.0 => Some(*v as u64),
        ScalarValue::Utf8(text) => text.parse().ok(),
        _ => None,
    }
}

fn scalar_to_f64(value: &ScalarValue) -> Option<f64> {
    match value {
        ScalarValue::Null => None,
        ScalarValue::Int64(v) => Some(*v as f64),
        ScalarValue::Float64(v) => Some(*v),
        ScalarValue::Utf8(text) => text.parse().ok(),
        ScalarValue::Boolean(v) => Some(f64::from(*v)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::DuckDbEngine;

    fn fixture_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        fs::create_dir_all(root.join("facts")).unwrap();
        let csv = root.join("facts/sample.csv");
        fs::write(
            &csv,
            "id,name,score,note\n\
1,alpha,91.2,first\n\
2,beta,82.1,\n\
3,gamma,88.0,third\n\
4,gamma,88.0,fourth\n",
        )
        .unwrap();
        (dir, root)
    }

    fn csv_relation_sql(root: &std::path::Path) -> String {
        format!(
            "SELECT * FROM read_csv_auto('{}')",
            root.join("facts/sample.csv").display()
        )
    }

    #[test]
    fn profile_relation_reports_row_and_column_stats() {
        let (_dir, root) = fixture_workspace();
        let engine = DuckDbEngine::open_in_memory(&root).unwrap();
        let relation_sql = csv_relation_sql(&root);

        let profile = engine.profile_relation(&relation_sql).unwrap();

        assert_eq!(profile.row_count, 4);
        assert_eq!(profile.columns.len(), 4);

        let id = profile
            .columns
            .iter()
            .find(|col| col.name == "id")
            .expect("id column");
        assert_eq!(id.data_type, "BIGINT");
        assert_eq!(id.null_percentage, Some(0.0));
        assert_eq!(id.approx_distinct, Some(4));
        assert_eq!(id.min.as_deref(), Some("1"));
        assert_eq!(id.max.as_deref(), Some("4"));

        let note = profile
            .columns
            .iter()
            .find(|col| col.name == "note")
            .expect("note column");
        assert!(note.null_percentage.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn profile_relation_empty_relation_returns_zero_rows() {
        let (_dir, root) = fixture_workspace();
        let engine = DuckDbEngine::open_in_memory(&root).unwrap();
        let relation_sql = format!("{} WHERE 1 = 0", csv_relation_sql(&root));

        let profile = engine.profile_relation(&relation_sql).unwrap();
        assert_eq!(profile.row_count, 0);
        assert!(profile.columns.is_empty());
    }

    #[test]
    fn profile_relation_rejects_empty_sql() {
        let (_dir, root) = fixture_workspace();
        let engine = DuckDbEngine::open_in_memory(&root).unwrap();
        let err = engine.profile_relation("   ").unwrap_err().to_string();
        assert!(err.contains("empty"), "{err}");
    }
}
