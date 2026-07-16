use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::tempdir;

use crate::app::{
    app_manifest_path, database_path, default_view_path, schema_path, DATABASE_FILENAME,
};
use crate::{
    CellValue, DataApp, FieldType, FilterOperator, SortDirection, ViewDef, ViewFilter, ViewSort,
};

#[test]
fn create_open_and_crud_round_trip() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("CRM.data");

    let app = DataApp::create(&package_path, "CRM", "companies").unwrap();
    assert_eq!(app.title(), "CRM");
    assert_eq!(app.default_table(), "companies");
    assert!(app_manifest_path(&package_path).is_file());
    assert!(schema_path(&package_path).is_file());
    assert!(database_path(&package_path).is_file());
    assert!(default_view_path(&package_path).is_file());

    let tables = app.list_tables().unwrap();
    assert_eq!(tables, vec!["companies".to_string()]);

    let columns = app.columns("companies").unwrap();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name, "id");
    assert_eq!(columns[0].field_type, FieldType::Text);

    let revision_before = app.package_revision().unwrap();
    assert!(revision_before.starts_with("sha256:"));

    let mut values = BTreeMap::new();
    values.insert("id".to_string(), CellValue::Text("ignored".to_string()));
    let row_id = app.insert_row("companies", &values).unwrap();
    assert_ne!(row_id, "ignored");

    let row = app.get_row("companies", &row_id).unwrap().unwrap();
    assert_eq!(row.id, row_id);

    let revision_after_insert = app.package_revision().unwrap();
    assert_ne!(revision_before, revision_after_insert);

    let mut update = BTreeMap::new();
    update.insert("id".to_string(), CellValue::Text("nope".to_string()));
    assert!(app.update_row("companies", &row_id, &update).is_err());

    let rows = app.list_rows("companies", 10, 0).unwrap();
    assert_eq!(rows.len(), 1);

    app.delete_row("companies", &row_id).unwrap();
    assert!(app.get_row("companies", &row_id).unwrap().is_none());

    let reopened = DataApp::open(&package_path).unwrap();
    assert_eq!(reopened.title(), "CRM");
    assert!(reopened.list_rows("companies", 10, 0).unwrap().is_empty());
}

#[test]
fn package_revision_is_stable_for_same_bytes() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Stable.data");
    let app = DataApp::create(&package_path, "Stable", "items").unwrap();

    let first = app.package_revision().unwrap();
    let second = app.package_revision().unwrap();
    assert_eq!(first, second);

    let db_path = database_path(&package_path);
    let bytes = std::fs::read(&db_path).unwrap();
    let digest = Sha256::digest(&bytes);
    let expected = format!("sha256:{}", hex::encode(digest));
    assert_eq!(first, expected);
}

#[test]
fn open_rejects_missing_required_files() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Broken.data");
    std::fs::create_dir_all(&package_path).unwrap();

    match DataApp::open(&package_path) {
        Err(error) => assert!(error.to_string().contains("missing required file")),
        Ok(_) => panic!("expected open to fail"),
    }
}

#[test]
fn create_rejects_existing_path() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Exists.data");
    DataApp::create(&package_path, "Exists", "items").unwrap();

    match DataApp::create(&package_path, "Exists", "items") {
        Err(error) => assert!(error.to_string().contains("already exists")),
        Ok(_) => panic!("expected create to fail"),
    }
}

#[test]
fn default_view_references_database_and_table() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Viewed.data");
    DataApp::create(&package_path, "Viewed", "records").unwrap();

    let view_text = std::fs::read_to_string(default_view_path(&package_path)).unwrap();
    assert!(view_text.contains(DATABASE_FILENAME));
    assert!(view_text.contains("table: records"));
}

#[test]
fn schema_sql_contains_primary_key() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Schema.data");
    DataApp::create(&package_path, "Schema", "widgets").unwrap();

    let schema = std::fs::read_to_string(schema_path(&package_path)).unwrap();
    assert!(schema.contains("CREATE TABLE widgets"));
    assert!(schema.contains("id TEXT PRIMARY KEY"));
}

#[test]
fn app_yaml_includes_default_table_metadata() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Meta.data");
    DataApp::create(&package_path, "Meta", "entries").unwrap();

    let manifest_text = std::fs::read_to_string(app_manifest_path(&package_path)).unwrap();
    assert!(manifest_text.contains("default_table: entries"));
    assert!(manifest_text.contains("default_view: All"));
    assert!(manifest_text.contains("type: text"));
}

#[allow(dead_code)]
fn package_layout_reference() -> &'static Path {
    Path::new("Name.data")
}

#[test]
fn view_round_trip_and_list_rows_with_view() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Viewed.data");
    let app = DataApp::create(&package_path, "Viewed", "records").unwrap();

    rusqlite::Connection::open(database_path(&package_path))
        .unwrap()
        .execute_batch("ALTER TABLE records ADD COLUMN name TEXT;")
        .unwrap();

    app.insert_row(
        "records",
        &BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
    )
    .unwrap();
    app.insert_row(
        "records",
        &BTreeMap::from([("name".into(), CellValue::Text("Grace".into()))]),
    )
    .unwrap();

    let mut view = ViewDef::new_grid("records");
    view.layout.columns = vec!["id".into(), "name".into()];
    view.sort = Some(ViewSort {
        field: "name".into(),
        direction: SortDirection::Desc,
    });
    view.filter = vec![ViewFilter {
        field: "name".into(),
        operator: FilterOperator::Contains,
        value: "a".into(),
    }];

    let yaml = app.render_view_yaml(&view).unwrap();
    assert!(yaml.contains("format: lattice-view"));
    assert!(yaml.contains("operator: contains"));

    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).unwrap();
    let custom_path = views_dir.join("Filtered.yaml");
    std::fs::write(&custom_path, &yaml).unwrap();

    let views = app.list_views().unwrap();
    assert!(views.iter().any(|name| name == "All"));
    assert!(views.iter().any(|name| name == "Filtered"));

    let loaded = app.load_view("Filtered").unwrap();
    assert_eq!(loaded, view);

    let (columns, rows) = app.list_rows_with_view("records", &loaded, 10, 0).unwrap();
    assert_eq!(columns.len(), 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].values.get("name"),
        Some(&CellValue::Text("Grace".into()))
    );
}

#[test]
fn csv_parse_and_import_columns() {
    use crate::csv::{infer_field_type, parse_csv_file, sanitize_column_name};

    assert_eq!(sanitize_column_name("Full Name"), "full_name");
    assert_eq!(
        infer_field_type(&["10".into(), "20".into()]),
        FieldType::Integer
    );

    let dir = tempdir().unwrap();
    let csv_path = dir.path().join("people.csv");
    std::fs::write(&csv_path, "name,active,count\nAda,true,1\nGrace,false,2\n").unwrap();

    let parsed = parse_csv_file(&csv_path).unwrap();
    assert_eq!(parsed.headers, vec!["name", "active", "count"]);
    assert_eq!(parsed.field_types[0], FieldType::Text);
    assert_eq!(parsed.field_types[1], FieldType::Boolean);
    assert_eq!(parsed.field_types[2], FieldType::Integer);

    let package_path = dir.path().join("People.data");
    let mut app = DataApp::create(&package_path, "People", "records").unwrap();
    app.add_columns_from_csv("records", &parsed).unwrap();
    let inserted = app.insert_csv_rows("records", &parsed).unwrap();
    assert_eq!(inserted, 2);

    let rows = app.list_rows("records", 10, 0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].values.get("name"),
        Some(&CellValue::Text("Ada".into()))
    );
    assert_eq!(
        rows[0].values.get("active"),
        Some(&CellValue::Boolean(true))
    );
    assert_eq!(rows[0].values.get("count"), Some(&CellValue::Integer(1)));
}
