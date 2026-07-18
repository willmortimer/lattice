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
fn list_and_board_views_load_with_layout_metadata() {
    use crate::view::{LAYOUT_BOARD, LAYOUT_LIST};

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Boarded.data");
    let app = DataApp::create(&package_path, "Boarded", "records").unwrap();

    rusqlite::Connection::open(database_path(&package_path))
        .unwrap()
        .execute_batch(
            "ALTER TABLE records ADD COLUMN name TEXT;
             ALTER TABLE records ADD COLUMN status TEXT;",
        )
        .unwrap();

    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).unwrap();

    let mut list_view = ViewDef::new_grid("records");
    list_view.layout.layout_type = LAYOUT_LIST.to_string();
    std::fs::write(
        views_dir.join("List.yaml"),
        app.render_view_yaml(&list_view).unwrap(),
    )
    .unwrap();

    let mut board_view = ViewDef::new_grid("records");
    board_view.layout.layout_type = LAYOUT_BOARD.to_string();
    board_view.layout.group_by = Some("status".into());
    std::fs::write(
        views_dir.join("Board.yaml"),
        app.render_view_yaml(&board_view).unwrap(),
    )
    .unwrap();

    let loaded_list = app.load_view("List").unwrap();
    assert_eq!(loaded_list.layout.layout_type, LAYOUT_LIST);
    assert!(loaded_list.layout.group_by.is_none());

    let loaded_board = app.load_view("Board").unwrap();
    assert_eq!(loaded_board.layout.layout_type, LAYOUT_BOARD);
    assert_eq!(loaded_board.layout.group_by.as_deref(), Some("status"));

    let invalid_group_by = format!(
        "format: lattice-view\nversion: 1\nsource:\n  database: ../database.sqlite\n  table: records\nlayout:\n  type: grid\n  group_by: status\n"
    );
    std::fs::write(views_dir.join("Invalid.yaml"), invalid_group_by).unwrap();
    let err = app.load_view("Invalid").unwrap_err().to_string();
    assert!(err.contains("group_by"));
}

#[test]
fn gallery_calendar_and_form_views_load_with_layout_metadata() {
    use crate::view::{LAYOUT_CALENDAR, LAYOUT_FORM, LAYOUT_GALLERY};

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Reserved.data");
    let app = DataApp::create(&package_path, "Reserved", "records").unwrap();

    rusqlite::Connection::open(database_path(&package_path))
        .unwrap()
        .execute_batch(
            "ALTER TABLE records ADD COLUMN name TEXT;
             ALTER TABLE records ADD COLUMN photo TEXT;
             ALTER TABLE records ADD COLUMN due_date TEXT;",
        )
        .unwrap();

    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).unwrap();

    let mut gallery_view = ViewDef::new_grid("records");
    gallery_view.layout.layout_type = LAYOUT_GALLERY.to_string();
    gallery_view.layout.cover_field = Some("photo".into());
    std::fs::write(
        views_dir.join("Gallery.yaml"),
        app.render_view_yaml(&gallery_view).unwrap(),
    )
    .unwrap();

    let mut calendar_view = ViewDef::new_grid("records");
    calendar_view.layout.layout_type = LAYOUT_CALENDAR.to_string();
    calendar_view.layout.date_field = Some("due_date".into());
    std::fs::write(
        views_dir.join("Calendar.yaml"),
        app.render_view_yaml(&calendar_view).unwrap(),
    )
    .unwrap();

    let mut form_view = ViewDef::new_grid("records");
    form_view.layout.layout_type = LAYOUT_FORM.to_string();
    form_view.layout.columns = vec!["name".into(), "due_date".into()];
    std::fs::write(
        views_dir.join("Form.yaml"),
        app.render_view_yaml(&form_view).unwrap(),
    )
    .unwrap();

    let loaded_gallery = app.load_view("Gallery").unwrap();
    assert_eq!(loaded_gallery.layout.layout_type, LAYOUT_GALLERY);
    assert_eq!(loaded_gallery.layout.cover_field.as_deref(), Some("photo"));

    let loaded_calendar = app.load_view("Calendar").unwrap();
    assert_eq!(loaded_calendar.layout.layout_type, LAYOUT_CALENDAR);
    assert_eq!(
        loaded_calendar.layout.date_field.as_deref(),
        Some("due_date")
    );

    let loaded_form = app.load_view("Form").unwrap();
    assert_eq!(loaded_form.layout.layout_type, LAYOUT_FORM);
    assert_eq!(loaded_form.layout.columns, vec!["name", "due_date"]);
}

#[test]
fn layout_field_misuse_is_rejected() {
    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Misused.data");
    let app = DataApp::create(&package_path, "Misused", "records").unwrap();

    let views_dir = package_path.join("views");
    std::fs::create_dir_all(&views_dir).unwrap();

    let cover_on_grid = "format: lattice-view\nversion: 1\nsource:\n  database: ../database.sqlite\n  table: records\nlayout:\n  type: grid\n  cover_field: photo\n";
    std::fs::write(views_dir.join("CoverOnGrid.yaml"), cover_on_grid).unwrap();
    let err = app.load_view("CoverOnGrid").unwrap_err().to_string();
    assert!(err.contains("cover_field"));

    let date_on_gallery = "format: lattice-view\nversion: 1\nsource:\n  database: ../database.sqlite\n  table: records\nlayout:\n  type: gallery\n  date_field: due_date\n";
    std::fs::write(views_dir.join("DateOnGallery.yaml"), date_on_gallery).unwrap();
    let err = app.load_view("DateOnGallery").unwrap_err().to_string();
    assert!(err.contains("date_field"));

    let group_by_on_gallery = "format: lattice-view\nversion: 1\nsource:\n  database: ../database.sqlite\n  table: records\nlayout:\n  type: gallery\n  group_by: status\n";
    std::fs::write(views_dir.join("GroupByOnGallery.yaml"), group_by_on_gallery).unwrap();
    let err = app.load_view("GroupByOnGallery").unwrap_err().to_string();
    assert!(err.contains("group_by"));

    let unsupported = "format: lattice-view\nversion: 1\nsource:\n  database: ../database.sqlite\n  table: records\nlayout:\n  type: dashboard\n";
    std::fs::write(views_dir.join("Unsupported.yaml"), unsupported).unwrap();
    let err = app.load_view("Unsupported").unwrap_err().to_string();
    assert!(err.contains("unsupported view layout type"));
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

#[test]
fn relation_column_validates_target_record_ids() {
    use crate::NewColumn;

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("CRM.data");
    let mut app = DataApp::create(&package_path, "CRM", "companies").unwrap();
    app.add_table("contacts").unwrap();
    app.add_columns(
        "contacts",
        &[
            NewColumn::new("name", FieldType::Text),
            NewColumn::relation("company", "companies"),
        ],
    )
    .unwrap();

    let columns = app.columns("contacts").unwrap();
    let company_col = columns
        .iter()
        .find(|column| column.name == "company")
        .expect("company column");
    assert_eq!(company_col.field_type, FieldType::Relation);
    assert_eq!(company_col.relation_table.as_deref(), Some("companies"));
    assert_eq!(company_col.sqlite_type.to_ascii_uppercase(), "TEXT");

    let manifest_text = std::fs::read_to_string(app_manifest_path(&package_path)).unwrap();
    assert!(manifest_text.contains("type: relation"));
    assert!(manifest_text.contains("relation_table: companies"));

    let company_id = app.insert_row("companies", &BTreeMap::new()).unwrap();
    let other_company_id = app.insert_row("companies", &BTreeMap::new()).unwrap();

    let contact_id = app
        .insert_row(
            "contacts",
            &BTreeMap::from([(
                "company".into(),
                CellValue::Relation {
                    record_ids: vec![company_id.clone(), other_company_id.clone()],
                },
            )]),
        )
        .unwrap();

    let row = app.get_row("contacts", &contact_id).unwrap().unwrap();
    assert_eq!(
        row.values.get("company"),
        Some(&CellValue::Relation {
            record_ids: vec![company_id.clone(), other_company_id.clone()],
        })
    );

    // SQLite TEXT encoding is a JSON array of ids.
    let raw: String = rusqlite::Connection::open(database_path(&package_path))
        .unwrap()
        .query_row(
            "SELECT company FROM contacts WHERE id = ?1",
            rusqlite::params![contact_id],
            |row| row.get(0),
        )
        .unwrap();
    let decoded: Vec<String> = serde_json::from_str(&raw).unwrap();
    assert_eq!(decoded, vec![company_id.clone(), other_company_id.clone()]);

    let invalid = app.insert_row(
        "contacts",
        &BTreeMap::from([(
            "company".into(),
            CellValue::Relation {
                record_ids: vec!["missing-id".into()],
            },
        )]),
    );
    assert!(invalid
        .unwrap_err()
        .to_string()
        .contains("not found in table companies"));

    let update_ok = app.update_row(
        "contacts",
        &contact_id,
        &BTreeMap::from([(
            "company".into(),
            CellValue::Relation {
                record_ids: vec![other_company_id.clone()],
            },
        )]),
    );
    assert!(update_ok.is_ok());

    let update_bad = app.update_row(
        "contacts",
        &contact_id,
        &BTreeMap::from([(
            "company".into(),
            CellValue::Relation {
                record_ids: vec!["still-missing".into()],
            },
        )]),
    );
    assert!(update_bad
        .unwrap_err()
        .to_string()
        .contains("not found in table companies"));

    // Wrong cell shape for a relation column is rejected.
    let wrong_shape = app.update_row(
        "contacts",
        &contact_id,
        &BTreeMap::from([("company".into(), CellValue::Text("nope".into()))]),
    );
    assert!(wrong_shape
        .unwrap_err()
        .to_string()
        .contains("expects a relation value"));
}

#[test]
fn relation_column_requires_relation_table_metadata() {
    use crate::NewColumn;

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("BrokenRel.data");
    let mut app = DataApp::create(&package_path, "BrokenRel", "items").unwrap();
    let err = app
        .add_columns(
            "items",
            &[NewColumn {
                name: "parent",
                field_type: FieldType::Relation,
                relation_table: None,
            }],
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("requires relation_table"));
}

#[test]
fn delete_row_strips_orphan_relation_ids() {
    use crate::NewColumn;

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("Org.data");
    let mut app = DataApp::create(&package_path, "Org", "contacts").unwrap();
    app.add_columns(
        "contacts",
        &[
            NewColumn::new("name", FieldType::Text),
            NewColumn::relation("reports_to", "contacts"),
        ],
    )
    .unwrap();

    let ada_id = app
        .insert_row(
            "contacts",
            &BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
        )
        .unwrap();
    let grace_id = app
        .insert_row(
            "contacts",
            &BTreeMap::from([
                ("name".into(), CellValue::Text("Grace".into())),
                (
                    "reports_to".into(),
                    CellValue::Relation {
                        record_ids: vec![ada_id.clone()],
                    },
                ),
            ]),
        )
        .unwrap();

    let strips = app.delete_row("contacts", &ada_id).unwrap();
    assert_eq!(strips.len(), 1);
    assert_eq!(strips[0].table, "contacts");
    assert_eq!(strips[0].row_id, grace_id);
    assert_eq!(strips[0].column, "reports_to");
    assert_eq!(strips[0].prior_record_ids, vec![ada_id.clone()]);

    let grace = app.get_row("contacts", &grace_id).unwrap().unwrap();
    assert_eq!(
        grace.values.get("reports_to"),
        Some(&CellValue::Relation { record_ids: vec![] })
    );
    assert!(app.get_row("contacts", &ada_id).unwrap().is_none());

    // Honest undo: restore Ada, then re-apply the captured inbound link.
    app.restore_row(
        "contacts",
        &crate::Row {
            id: ada_id.clone(),
            values: BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
        },
    )
    .unwrap();
    app.restore_relation_strips(&strips).unwrap();
    let grace = app.get_row("contacts", &grace_id).unwrap().unwrap();
    assert_eq!(
        grace.values.get("reports_to"),
        Some(&CellValue::Relation {
            record_ids: vec![ada_id],
        })
    );
}

#[test]
fn delete_row_strips_cross_table_relation_ids() {
    use crate::NewColumn;

    let dir = tempdir().unwrap();
    let package_path = dir.path().join("CRM.data");
    let mut app = DataApp::create(&package_path, "CRM", "companies").unwrap();
    app.add_table("contacts").unwrap();
    app.add_columns(
        "contacts",
        &[
            NewColumn::new("name", FieldType::Text),
            NewColumn::relation("company", "companies"),
        ],
    )
    .unwrap();

    let company_id = app.insert_row("companies", &BTreeMap::new()).unwrap();
    let other_id = app.insert_row("companies", &BTreeMap::new()).unwrap();
    let contact_id = app
        .insert_row(
            "contacts",
            &BTreeMap::from([(
                "company".into(),
                CellValue::Relation {
                    record_ids: vec![company_id.clone(), other_id.clone()],
                },
            )]),
        )
        .unwrap();

    app.delete_row("companies", &company_id).unwrap();
    let contact = app.get_row("contacts", &contact_id).unwrap().unwrap();
    assert_eq!(
        contact.values.get("company"),
        Some(&CellValue::Relation {
            record_ids: vec![other_id],
        })
    );
}
