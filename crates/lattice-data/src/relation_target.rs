//! Parse `relation_table` specs for same-package and cross-package targets.
//!
//! Syntax:
//! - Same package: bare SQL identifier (`companies`)
//! - Cross-package (read-only): workspace-relative `.data` path + `#` + table
//!   (`Directory.data#companies`, `team/Directory.data#companies`)

use std::path::{Component, Path};

/// Parsed [`crate::types::ColumnMeta::relation_table`] value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationTarget<'a> {
    /// Table in the same `.data` package.
    Local { table: &'a str },
    /// Table in another `.data` package under the workspace root (read-only).
    CrossPackage {
        /// Workspace-relative path to the foreign package (ends with `.data`).
        package_rel: &'a str,
        table: &'a str,
    },
}

impl<'a> RelationTarget<'a> {
    pub fn is_cross_package(self) -> bool {
        matches!(self, Self::CrossPackage { .. })
    }

    pub fn table(self) -> &'a str {
        match self {
            Self::Local { table } | Self::CrossPackage { table, .. } => table,
        }
    }
}

/// Parse a `relation_table` string into a local or cross-package target.
pub fn parse_relation_target(spec: &str) -> Result<RelationTarget<'_>, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err("relation_table must not be empty".into());
    }

    if let Some((package_rel, table)) = trimmed.split_once('#') {
        let package_rel = package_rel.trim();
        let table = table.trim();
        if package_rel.is_empty() || table.is_empty() {
            return Err(
                "cross-package relation_table must be `Path/To/Package.data#table`".into(),
            );
        }
        if table.contains('#') {
            return Err("cross-package relation_table must contain exactly one '#'".into());
        }
        validate_table_identifier(table)?;
        validate_package_rel(package_rel)?;
        Ok(RelationTarget::CrossPackage { package_rel, table })
    } else {
        validate_table_identifier(trimmed)?;
        Ok(RelationTarget::Local { table: trimmed })
    }
}

fn validate_table_identifier(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid relation target table {name:?}; use letters, digits, and underscores"
        ))
    }
}

fn validate_package_rel(package_rel: &str) -> Result<(), String> {
    let path = Path::new(package_rel);
    if path.is_absolute() {
        return Err(format!(
            "cross-package relation target {package_rel:?} must be relative to the workspace root"
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "cross-package relation target {package_rel:?} escapes the workspace root"
        ));
    }
    if path.components().count() == 0
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir))
    {
        return Err(format!(
            "cross-package relation target {package_rel:?} is not a valid package path"
        ));
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(format!(
            "cross-package relation target {package_rel:?} is not a valid package path"
        ));
    };
    if !name.ends_with(".data") || name == ".data" {
        return Err(format!(
            "cross-package relation target {package_rel:?} must end with a `.data` package name"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_and_cross_package_targets() {
        assert_eq!(
            parse_relation_target("companies").unwrap(),
            RelationTarget::Local { table: "companies" }
        );
        assert_eq!(
            parse_relation_target("Directory.data#companies").unwrap(),
            RelationTarget::CrossPackage {
                package_rel: "Directory.data",
                table: "companies",
            }
        );
        assert_eq!(
            parse_relation_target("team/Directory.data#orgs").unwrap(),
            RelationTarget::CrossPackage {
                package_rel: "team/Directory.data",
                table: "orgs",
            }
        );
    }

    #[test]
    fn rejects_invalid_cross_package_specs() {
        assert!(parse_relation_target("").is_err());
        assert!(parse_relation_target("#companies").is_err());
        assert!(parse_relation_target("Directory.data#").is_err());
        assert!(parse_relation_target("Directory#companies").is_err());
        assert!(parse_relation_target("../Other.data#companies").is_err());
        assert!(parse_relation_target("/abs/Other.data#companies").is_err());
        assert!(parse_relation_target("1bad").is_err());
    }
}
