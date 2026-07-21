#!/usr/bin/env node

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TEMPLATE_ROOT = join(ROOT, "templates", "workspaces");
const RUST_OUT = join(ROOT, "crates", "lattice-core", "src", "template_catalog.generated.rs");
const TS_OUT = join(ROOT, "apps", "desktop", "src", "templateCatalog.generated.ts");
const DEMO_OUT = join(ROOT, "apps", "desktop", "src", "demoWorkspace.generated.ts");
const DEMO_TEMPLATE_ID = "demo";
const DEMO_WORKSPACE_ID = "0198-demo";
const MAX_FILES = 128;
const MAX_BYTES = 2 * 1024 * 1024;
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "tiff"]);
const CODE_EXTENSIONS = new Set([
  "js",
  "jsx",
  "ts",
  "tsx",
  "rs",
  "py",
  "go",
  "java",
  "c",
  "cpp",
  "h",
  "css",
  "html",
  "sql",
  "sh",
]);
const TEXT_BODY_EXTENSIONS = new Set([
  "txt",
  "md",
  "markdown",
  "log",
  "json",
  "yaml",
  "yml",
  "csv",
  "tsv",
  "svg",
  ...CODE_EXTENSIONS,
]);
const SQLITE_TYPES = {
  text: "TEXT",
  long_text: "TEXT",
  integer: "INTEGER",
  decimal: "REAL",
  boolean: "INTEGER",
  date: "TEXT",
  relation: "TEXT",
  lookup: "TEXT",
  rollup: "TEXT",
};
const TEMPLATE_CATEGORIES = [
  "Everyday",
  "Work",
  "Knowledge & Research",
  "Data & Advanced",
  "Sample",
];
const WORKSPACE_DEFAULT_KEYS = [
  "quickNoteDirectory",
  "dailyNoteDirectory",
  "attachmentsDirectory",
  "templateDirectory",
  "archiveDirectory",
];
const FIELD_TYPES = new Set([
  "text",
  "long_text",
  "integer",
  "decimal",
  "boolean",
  "date",
  "relation",
  "lookup",
  "rollup",
]);
const ROLLUP_AGGREGATES = new Set(["count", "sum", "min", "max"]);
const VIEW_LAYOUTS = new Set(["grid", "list", "board", "gallery", "calendar", "form"]);
// Flat seed files (including binaries) are embedded via include_bytes!; declarative
// dataPackages are JSON-only and materialized to SQLite at provision time.

export function compileTemplates(root = TEMPLATE_ROOT) {
  const templates = readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => loadTemplate(root, entry.name))
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));
  const ids = new Set();
  for (const template of templates) {
    if (ids.has(template.id)) throw new Error(`duplicate template id ${template.id}`);
    ids.add(template.id);
  }
  return templates;
}

function loadTemplate(root, directoryName) {
  const directory = join(root, directoryName);
  const manifestPath = join(directory, "template.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  for (const key of [
    "format",
    "version",
    "id",
    "order",
    "name",
    "category",
    "description",
    "visibility",
    "recommendedTitle",
    "directories",
    "preview",
    "files",
    "capabilities",
    "workspaceDefaults",
  ]) {
    if (manifest[key] === undefined) throw new Error(`${directoryName}: missing ${key}`);
  }
  if (manifest.format !== "lattice-workspace-template" || ![1, 2].includes(manifest.version)) {
    throw new Error(`${directoryName}: unsupported template format/version`);
  }
  if (manifest.id !== directoryName) {
    throw new Error(`${directoryName}: id must match directory`);
  }
  if (!Number.isInteger(manifest.order) || manifest.order < 0) {
    throw new Error(`${directoryName}: order must be a non-negative integer`);
  }
  if (!["gallery", "legacy", "sample"].includes(manifest.visibility)) {
    throw new Error(`${directoryName}: invalid visibility`);
  }
  if (!TEMPLATE_CATEGORIES.includes(manifest.category)) {
    throw new Error(
      `${directoryName}: invalid category ${JSON.stringify(manifest.category)}; expected one of ${TEMPLATE_CATEGORIES.join(", ")}`,
    );
  }
  if (manifest.category === "Sample" && manifest.visibility !== "sample") {
    throw new Error(`${directoryName}: category Sample is reserved for visibility: sample`);
  }
  if (manifest.visibility === "sample" && manifest.category !== "Sample") {
    throw new Error(`${directoryName}: visibility sample requires category Sample`);
  }
  const dataPackages = normalizeDataPackages(manifest.dataPackages, directoryName);
  if (
    !Array.isArray(manifest.files) ||
    !Array.isArray(manifest.directories) ||
    manifest.files.length + manifest.directories.length + dataPackages.length > MAX_FILES
  ) {
    throw new Error(`${directoryName}: too many files`);
  }

  const directories = manifest.directories.map((entry) => normalizeDirectory(entry, directoryName));
  const workspaceDefaults = normalizeWorkspaceDefaults(manifest.workspaceDefaults, directoryName);
  const openOnCreate =
    manifest.openOnCreate === undefined
      ? undefined
      : normalizeOptionalPath(manifest.openOnCreate, directoryName, "openOnCreate");

  const destinations = new Set(["lattice.yaml"]);
  for (const path of [
    ...directories.map((entry) => entry.path),
    ...manifest.files,
    ...dataPackages.map((entry) => entry.path),
  ]) {
    assertSafePath(path, directoryName);
    const normalized = posix.normalize(path);
    if (destinations.has(normalized)) {
      throw new Error(`${directoryName}: duplicate destination ${normalized}`);
    }
    destinations.add(normalized);
  }
  if (openOnCreate !== undefined && !destinations.has(posix.normalize(openOnCreate))) {
    throw new Error(`${directoryName}: openOnCreate ${JSON.stringify(openOnCreate)} is not a seeded path`);
  }

  let totalBytes = 0;
  const files = manifest.files.map((path) => {
    const source = join(directory, "files", ...path.split("/"));
    const stat = statSync(source);
    if (!stat.isFile()) throw new Error(`${directoryName}: missing file ${path}`);
    totalBytes += stat.size;
    return { path, source };
  });
  // Declarative dataPackages JSON counts toward the same 2MiB seed budget as flat files.
  totalBytes += Buffer.byteLength(JSON.stringify(dataPackages), "utf8");
  if (totalBytes > MAX_BYTES) throw new Error(`${directoryName}: template exceeds size bound`);
  warnMissingDefaultDirectories(directoryName, directories, workspaceDefaults);
  validateLinks(
    directoryName,
    directories.map((entry) => entry.path),
    files,
  );

  return {
    ...manifest,
    directories,
    workspaceDefaults,
    openOnCreate,
    recommended: Boolean(manifest.recommended),
    files,
    dataPackages,
  };
}

function normalizeDataPackages(raw, template) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: dataPackages must be an array`);
  }
  return raw.map((entry, index) => normalizeDataPackage(entry, template, index));
}

function normalizeSeedColumn(column, template, columnLabel) {
  if (!column || typeof column !== "object" || Array.isArray(column)) {
    throw new Error(`${template}: ${columnLabel} must be an object`);
  }
  if (typeof column.name !== "string" || !isSqlIdentifier(column.name)) {
    throw new Error(`${template}: ${columnLabel}.name must be a valid SQL identifier`);
  }
  if (column.name === "id") {
    throw new Error(`${template}: ${columnLabel}.name cannot be id (reserved)`);
  }
  if (typeof column.type !== "string" || !FIELD_TYPES.has(column.type)) {
    throw new Error(
      `${template}: ${columnLabel}.type must be one of ${[...FIELD_TYPES].join(", ")}`,
    );
  }

  let relationTable;
  let junctionTable;
  if (column.type === "relation") {
    if (typeof column.relation_table !== "string" || !isSqlIdentifier(column.relation_table)) {
      throw new Error(
        `${template}: ${columnLabel}.relation_table must be a valid SQL identifier for relation columns`,
      );
    }
    relationTable = column.relation_table;
    if (column.junction_table !== undefined) {
      if (typeof column.junction_table !== "string" || !isSqlIdentifier(column.junction_table)) {
        throw new Error(
          `${template}: ${columnLabel}.junction_table must be a valid SQL identifier for relation columns`,
        );
      }
      junctionTable = column.junction_table;
    }
  } else if (column.relation_table !== undefined) {
    throw new Error(
      `${template}: ${columnLabel}.relation_table is only supported for relation columns`,
    );
  } else if (column.junction_table !== undefined) {
    throw new Error(
      `${template}: ${columnLabel}.junction_table is only supported for relation columns`,
    );
  }

  let lookupRelation;
  let lookupField;
  if (column.type === "lookup") {
    if (typeof column.lookup_relation !== "string" || !isSqlIdentifier(column.lookup_relation)) {
      throw new Error(
        `${template}: ${columnLabel}.lookup_relation must be a valid SQL identifier for lookup columns`,
      );
    }
    if (typeof column.lookup_field !== "string" || !isSqlIdentifier(column.lookup_field)) {
      throw new Error(
        `${template}: ${columnLabel}.lookup_field must be a valid SQL identifier for lookup columns`,
      );
    }
    lookupRelation = column.lookup_relation;
    lookupField = column.lookup_field;
  } else if (column.lookup_relation !== undefined || column.lookup_field !== undefined) {
    throw new Error(
      `${template}: ${columnLabel}.lookup_relation / lookup_field are only supported for lookup columns`,
    );
  }

  let rollupRelation;
  let rollupAggregate;
  let rollupField;
  if (column.type === "rollup") {
    if (typeof column.rollup_relation !== "string" || !isSqlIdentifier(column.rollup_relation)) {
      throw new Error(
        `${template}: ${columnLabel}.rollup_relation must be a valid SQL identifier for rollup columns`,
      );
    }
    if (
      typeof column.rollup_aggregate !== "string" ||
      !ROLLUP_AGGREGATES.has(column.rollup_aggregate)
    ) {
      throw new Error(
        `${template}: ${columnLabel}.rollup_aggregate must be one of ${[...ROLLUP_AGGREGATES].join(", ")}`,
      );
    }
    rollupRelation = column.rollup_relation;
    rollupAggregate = column.rollup_aggregate;
    if (column.rollup_field !== undefined) {
      if (typeof column.rollup_field !== "string" || !isSqlIdentifier(column.rollup_field)) {
        throw new Error(
          `${template}: ${columnLabel}.rollup_field must be a valid SQL identifier for rollup columns`,
        );
      }
      rollupField = column.rollup_field;
    }
  } else if (
    column.rollup_relation !== undefined ||
    column.rollup_aggregate !== undefined ||
    column.rollup_field !== undefined
  ) {
    throw new Error(
      `${template}: ${columnLabel}.rollup_relation / rollup_aggregate / rollup_field are only supported for rollup columns`,
    );
  }

  return {
    name: column.name,
    type: column.type,
    ...(relationTable === undefined ? {} : { relation_table: relationTable }),
    ...(junctionTable === undefined ? {} : { junction_table: junctionTable }),
    ...(lookupRelation === undefined ? {} : { lookup_relation: lookupRelation }),
    ...(lookupField === undefined ? {} : { lookup_field: lookupField }),
    ...(rollupRelation === undefined ? {} : { rollup_relation: rollupRelation }),
    ...(rollupAggregate === undefined ? {} : { rollup_aggregate: rollupAggregate }),
    ...(rollupField === undefined ? {} : { rollup_field: rollupField }),
  };
}

function normalizeDataPackage(entry, template, index) {
  const label = `dataPackages[${index}]`;
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  for (const key of ["path", "title", "table", "columns", "rows"]) {
    if (entry[key] === undefined) {
      throw new Error(`${template}: ${label} missing ${key}`);
    }
  }
  if (typeof entry.path !== "string" || !entry.path.endsWith(".data")) {
    throw new Error(`${template}: ${label}.path must end with .data`);
  }
  assertSafePath(entry.path, template);
  if (typeof entry.title !== "string" || entry.title.trim().length === 0) {
    throw new Error(`${template}: ${label}.title must be a non-empty string`);
  }
  if (typeof entry.table !== "string" || !isSqlIdentifier(entry.table)) {
    throw new Error(`${template}: ${label}.table must be a valid SQL identifier`);
  }
  if (!Array.isArray(entry.columns) || entry.columns.length === 0) {
    throw new Error(`${template}: ${label}.columns must be a non-empty array`);
  }
  if (!Array.isArray(entry.rows)) {
    throw new Error(`${template}: ${label}.rows must be an array`);
  }

  const columns = [];
  const columnNames = new Set();
  for (const [columnIndex, column] of entry.columns.entries()) {
    const columnLabel = `${label}.columns[${columnIndex}]`;
    if (columnNames.has(column.name)) {
      throw new Error(`${template}: ${columnLabel} duplicate column ${column.name}`);
    }
    const normalized = normalizeSeedColumn(column, template, columnLabel);
    columnNames.add(normalized.name);
    columns.push(normalized);
  }

  const columnTypes = new Map(columns.map((column) => [column.name, column.type]));
  const rows = entry.rows.map((row, rowIndex) => {
    const rowLabel = `${label}.rows[${rowIndex}]`;
    if (!row || typeof row !== "object" || Array.isArray(row)) {
      throw new Error(`${template}: ${rowLabel} must be an object`);
    }
    for (const key of Object.keys(row)) {
      if (!columnNames.has(key)) {
        throw new Error(`${template}: ${rowLabel} unknown column ${key}`);
      }
      assertJsonCell(row[key], template, `${rowLabel}.${key}`, columnTypes.get(key));
    }
    return row;
  });

  const views = normalizeDataPackageViews(entry.views, template, label, columnNames);
  const forms = normalizeDataPackageForms(
    entry.forms,
    template,
    label,
    entry.table,
    columnNames,
  );
  const actions = normalizeDataPackageActions(
    entry.actions,
    template,
    label,
    entry.table,
    columnNames,
    forms,
  );
  const interfaces = normalizeDataPackageInterfaces(
    entry.interfaces,
    template,
    label,
    views,
    forms,
  );

  const extraTables = normalizeExtraTables(
    entry.extraTables,
    template,
    label,
    entry.table,
    new Set([entry.table]),
  );

  return {
    path: entry.path,
    title: entry.title,
    table: entry.table,
    columns,
    rows,
    extraTables,
    views,
    forms,
    actions,
    interfaces,
  };
}

function normalizeExtraTables(raw, template, label, mainTable, reservedTables) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label}.extraTables must be an array`);
  }
  const tables = new Set(reservedTables);
  return raw.map((entry, index) =>
    normalizeExtraTable(entry, template, `${label}.extraTables[${index}]`, mainTable, tables),
  );
}

function normalizeExtraTable(entry, template, label, mainTable, reservedTables) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  for (const key of ["table", "columns", "rows"]) {
    if (entry[key] === undefined) {
      throw new Error(`${template}: ${label} missing ${key}`);
    }
  }
  if (typeof entry.table !== "string" || !isSqlIdentifier(entry.table)) {
    throw new Error(`${template}: ${label}.table must be a valid SQL identifier`);
  }
  if (entry.table === mainTable) {
    throw new Error(`${template}: ${label}.table cannot match the package default table`);
  }
  if (reservedTables.has(entry.table)) {
    throw new Error(`${template}: ${label} duplicate table ${entry.table}`);
  }
  reservedTables.add(entry.table);
  if (!Array.isArray(entry.columns) || entry.columns.length === 0) {
    throw new Error(`${template}: ${label}.columns must be a non-empty array`);
  }
  if (!Array.isArray(entry.rows)) {
    throw new Error(`${template}: ${label}.rows must be an array`);
  }

  const columns = [];
  const columnNames = new Set();
  for (const [columnIndex, column] of entry.columns.entries()) {
    const columnLabel = `${label}.columns[${columnIndex}]`;
    if (columnNames.has(column.name)) {
      throw new Error(`${template}: ${columnLabel} duplicate column ${column.name}`);
    }
    const normalized = normalizeSeedColumn(column, template, columnLabel);
    columnNames.add(normalized.name);
    columns.push(normalized);
  }

  const columnTypes = new Map(columns.map((column) => [column.name, column.type]));
  const rows = entry.rows.map((row, rowIndex) => {
    const rowLabel = `${label}.rows[${rowIndex}]`;
    if (!row || typeof row !== "object" || Array.isArray(row)) {
      throw new Error(`${template}: ${rowLabel} must be an object`);
    }
    for (const key of Object.keys(row)) {
      if (!columnNames.has(key)) {
        throw new Error(`${template}: ${rowLabel} unknown column ${key}`);
      }
      assertJsonCell(row[key], template, `${rowLabel}.${key}`, columnTypes.get(key));
    }
    return row;
  });

  return {
    table: entry.table,
    columns,
    rows,
  };
}

function normalizeDataPackageViews(raw, template, label, columnNames) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label}.views must be an array`);
  }
  const allowedColumns = new Set([...columnNames, "id"]);
  const viewNames = new Set();
  return raw.map((entry, index) =>
    normalizeDataPackageView(entry, template, `${label}.views[${index}]`, allowedColumns, viewNames),
  );
}

function normalizeDataPackageView(entry, template, label, allowedColumns, viewNames) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  if (typeof entry.name !== "string" || !isSqlIdentifier(entry.name)) {
    throw new Error(`${template}: ${label}.name must be a valid SQL identifier`);
  }
  if (viewNames.has(entry.name)) {
    throw new Error(`${template}: ${label} duplicate view name ${entry.name}`);
  }
  viewNames.add(entry.name);
  if (typeof entry.layout !== "string" || !VIEW_LAYOUTS.has(entry.layout)) {
    throw new Error(
      `${template}: ${label}.layout must be one of ${[...VIEW_LAYOUTS].join(", ")}`,
    );
  }

  const layoutFields = {
    group_by: "board",
    cover_field: "gallery",
    date_field: "calendar",
  };
  for (const [field, layout] of Object.entries(layoutFields)) {
    if (entry[field] === undefined) continue;
    if (typeof entry[field] !== "string" || !isSqlIdentifier(entry[field])) {
      throw new Error(`${template}: ${label}.${field} must be a valid SQL identifier`);
    }
    if (!allowedColumns.has(entry[field])) {
      throw new Error(`${template}: ${label}.${field} references unknown column ${entry[field]}`);
    }
    if (entry.layout !== layout) {
      throw new Error(`${template}: ${label}.${field} is only supported for ${layout} views`);
    }
  }

  let columns = [];
  if (entry.columns !== undefined) {
    if (!Array.isArray(entry.columns)) {
      throw new Error(`${template}: ${label}.columns must be an array`);
    }
    const seen = new Set();
    columns = entry.columns.map((column, columnIndex) => {
      const columnLabel = `${label}.columns[${columnIndex}]`;
      if (typeof column !== "string" || !isSqlIdentifier(column)) {
        throw new Error(`${template}: ${columnLabel} must be a valid SQL identifier`);
      }
      if (!allowedColumns.has(column)) {
        throw new Error(`${template}: ${columnLabel} references unknown column ${column}`);
      }
      if (seen.has(column)) {
        throw new Error(`${template}: ${columnLabel} duplicate column ${column}`);
      }
      seen.add(column);
      return column;
    });
  }

  return {
    name: entry.name,
    layout: entry.layout,
    group_by: entry.group_by,
    cover_field: entry.cover_field,
    date_field: entry.date_field,
    columns,
  };
}

function normalizeDataPackageForms(raw, template, label, defaultTable, columnNames) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label}.forms must be an array`);
  }
  const allowedColumns = new Set([...columnNames, "id"]);
  const formNames = new Set();
  return raw.map((entry, index) =>
    normalizeDataPackageForm(
      entry,
      template,
      `${label}.forms[${index}]`,
      defaultTable,
      allowedColumns,
      formNames,
    ),
  );
}

function normalizeDataPackageForm(entry, template, label, defaultTable, allowedColumns, formNames) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  if (typeof entry.name !== "string" || !isSqlIdentifier(entry.name)) {
    throw new Error(`${template}: ${label}.name must be a valid SQL identifier`);
  }
  if (formNames.has(entry.name)) {
    throw new Error(`${template}: ${label} duplicate form name ${entry.name}`);
  }
  formNames.add(entry.name);
  const table =
    entry.table === undefined
      ? defaultTable
      : typeof entry.table === "string" && isSqlIdentifier(entry.table)
        ? entry.table
        : null;
  if (!table) {
    throw new Error(`${template}: ${label}.table must be a valid SQL identifier`);
  }
  if (table !== defaultTable) {
    throw new Error(
      `${template}: ${label}.table must match the package default table (${defaultTable})`,
    );
  }
  if (!Array.isArray(entry.fields) || entry.fields.length === 0) {
    throw new Error(`${template}: ${label}.fields must be a non-empty array`);
  }
  const seen = new Set();
  const fields = entry.fields.map((field, fieldIndex) => {
    const fieldLabel = `${label}.fields[${fieldIndex}]`;
    if (typeof field !== "string" || !isSqlIdentifier(field)) {
      throw new Error(`${template}: ${fieldLabel} must be a valid SQL identifier`);
    }
    if (!allowedColumns.has(field)) {
      throw new Error(`${template}: ${fieldLabel} references unknown column ${field}`);
    }
    if (seen.has(field)) {
      throw new Error(`${template}: ${fieldLabel} duplicate column ${field}`);
    }
    seen.add(field);
    return field;
  });
  if (entry.title !== undefined && (typeof entry.title !== "string" || entry.title.trim() === "")) {
    throw new Error(`${template}: ${label}.title must be a non-empty string`);
  }
  if (
    entry.description !== undefined &&
    (typeof entry.description !== "string" || entry.description.trim() === "")
  ) {
    throw new Error(`${template}: ${label}.description must be a non-empty string`);
  }
  return {
    name: entry.name,
    table,
    fields,
    title: entry.title,
    description: entry.description,
  };
}

const ACTION_TYPES = new Set(["insert_record", "update_field", "open_url"]);
const ACTION_SCOPES = new Set(["toolbar", "row"]);

function normalizeDataPackageActions(raw, template, label, defaultTable, columnNames, forms) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label}.actions must be an array`);
  }
  const allowedColumns = new Set([...columnNames, "id"]);
  const formNames = new Set(forms.map((form) => form.name));
  const actionNames = new Set();
  return raw.map((entry, index) =>
    normalizeDataPackageAction(
      entry,
      template,
      `${label}.actions[${index}]`,
      defaultTable,
      allowedColumns,
      formNames,
      actionNames,
    ),
  );
}

function normalizeDataPackageInterfaces(raw, template, label, views, forms) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label}.interfaces must be an array`);
  }
  const knownViews = new Set(["All", ...views.map((view) => view.name)]);
  const knownForms = new Set(forms.map((form) => form.name));
  const interfaceNames = new Set();
  return raw.map((entry, index) =>
    normalizeDataPackageInterface(
      entry,
      template,
      `${label}.interfaces[${index}]`,
      knownViews,
      knownForms,
      interfaceNames,
    ),
  );
}

function normalizeDataPackageAction(
  entry,
  template,
  label,
  defaultTable,
  allowedColumns,
  formNames,
  actionNames,
) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  if (typeof entry.name !== "string" || !isSqlIdentifier(entry.name)) {
    throw new Error(`${template}: ${label}.name must be a valid SQL identifier`);
  }
  if (actionNames.has(entry.name)) {
    throw new Error(`${template}: ${label} duplicate action name ${entry.name}`);
  }
  actionNames.add(entry.name);
  if (typeof entry.label !== "string" || entry.label.trim() === "") {
    throw new Error(`${template}: ${label}.label must be a non-empty string`);
  }
  const table =
    entry.table === undefined
      ? defaultTable
      : typeof entry.table === "string" && isSqlIdentifier(entry.table)
        ? entry.table
        : null;
  if (!table) {
    throw new Error(`${template}: ${label}.table must be a valid SQL identifier`);
  }
  const scope =
    entry.scope === undefined
      ? "toolbar"
      : typeof entry.scope === "string" && ACTION_SCOPES.has(entry.scope)
        ? entry.scope
        : null;
  if (!scope) {
    throw new Error(`${template}: ${label}.scope must be toolbar or row`);
  }
  if (!entry.action || typeof entry.action !== "object" || Array.isArray(entry.action)) {
    throw new Error(`${template}: ${label}.action must be an object`);
  }
  const actionType =
    typeof entry.action.type === "string" && ACTION_TYPES.has(entry.action.type)
      ? entry.action.type
      : null;
  if (!actionType) {
    throw new Error(
      `${template}: ${label}.action.type must be one of ${[...ACTION_TYPES].join(", ")}`,
    );
  }
  const action = { type: actionType };
  if (actionType === "insert_record") {
    if (entry.action.form !== undefined) {
      if (typeof entry.action.form !== "string" || !formNames.has(entry.action.form)) {
        throw new Error(`${template}: ${label}.action.form must reference a package form`);
      }
      action.form = entry.action.form;
    }
    if (entry.action.defaults !== undefined) {
      if (!entry.action.defaults || typeof entry.action.defaults !== "object" || Array.isArray(entry.action.defaults)) {
        throw new Error(`${template}: ${label}.action.defaults must be an object`);
      }
      const defaults = {};
      for (const [field, value] of Object.entries(entry.action.defaults)) {
        if (!isSqlIdentifier(field) || !allowedColumns.has(field)) {
          throw new Error(`${template}: ${label}.action.defaults references unknown column ${field}`);
        }
        if (typeof value !== "string") {
          throw new Error(`${template}: ${label}.action.defaults.${field} must be a string`);
        }
        defaults[field] = value;
      }
      action.defaults = defaults;
    }
    if (!action.form && (!action.defaults || Object.keys(action.defaults).length === 0)) {
      throw new Error(
        `${template}: ${label}.action insert_record requires form or non-empty defaults`,
      );
    }
  } else if (actionType === "update_field") {
    if (typeof entry.action.field !== "string" || !allowedColumns.has(entry.action.field) || entry.action.field === "id") {
      throw new Error(`${template}: ${label}.action.field must be a writable table column`);
    }
    if (typeof entry.action.value !== "string") {
      throw new Error(`${template}: ${label}.action.value must be a string`);
    }
    action.field = entry.action.field;
    action.value = entry.action.value;
  } else if (actionType === "open_url") {
    if (typeof entry.action.url !== "string" || entry.action.url.trim() === "") {
      throw new Error(`${template}: ${label}.action.url must be a non-empty string`);
    }
    const url = entry.action.url.trim();
    if (
      !/^https?:\/\//i.test(url) &&
      (url.startsWith("/") || url.split("/").some((part) => part === ".."))
    ) {
      throw new Error(
        `${template}: ${label}.action.url must be http(s) or a workspace-relative path`,
      );
    }
    action.url = url;
  }
  return {
    name: entry.name,
    label: entry.label.trim(),
    table,
    scope,
    action,
  };
}

function normalizeDataPackageInterface(
  entry,
  template,
  label,
  knownViews,
  knownForms,
  interfaceNames,
) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: ${label} must be an object`);
  }
  if (typeof entry.name !== "string" || !isSqlIdentifier(entry.name)) {
    throw new Error(`${template}: ${label}.name must be a valid SQL identifier`);
  }
  if (interfaceNames.has(entry.name)) {
    throw new Error(`${template}: ${label} duplicate interface name ${entry.name}`);
  }
  interfaceNames.add(entry.name);

  const views = normalizeInterfaceNameList(entry.views, template, `${label}.views`, knownViews, "view");
  const forms = normalizeInterfaceNameList(entry.forms, template, `${label}.forms`, knownForms, "form");
  if (views.length === 0 && forms.length === 0) {
    throw new Error(`${template}: ${label} must bind at least one view or form`);
  }
  if (entry.title !== undefined && (typeof entry.title !== "string" || entry.title.trim() === "")) {
    throw new Error(`${template}: ${label}.title must be a non-empty string`);
  }
  if (
    entry.description !== undefined &&
    (typeof entry.description !== "string" || entry.description.trim() === "")
  ) {
    throw new Error(`${template}: ${label}.description must be a non-empty string`);
  }
  return {
    name: entry.name,
    views,
    forms,
    title: entry.title,
    description: entry.description,
  };
}

function normalizeInterfaceNameList(raw, template, label, allowed, kind) {
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new Error(`${template}: ${label} must be an array`);
  }
  const seen = new Set();
  return raw.map((name, index) => {
    const itemLabel = `${label}[${index}]`;
    if (typeof name !== "string" || !isSqlIdentifier(name)) {
      throw new Error(`${template}: ${itemLabel} must be a valid SQL identifier`);
    }
    if (!allowed.has(name)) {
      throw new Error(`${template}: ${itemLabel} references unknown ${kind} ${name}`);
    }
    if (seen.has(name)) {
      throw new Error(`${template}: ${itemLabel} duplicate ${kind} ${name}`);
    }
    seen.add(name);
    return name;
  });
}

function assertJsonCell(value, template, label, fieldType) {
  const kind = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
  if (fieldType === "relation") {
    if (kind !== "array") {
      throw new Error(`${template}: ${label} must be a JSON array of record ids for relation columns`);
    }
    for (const [index, item] of value.entries()) {
      if (typeof item !== "string" || item.length === 0) {
        throw new Error(
          `${template}: ${label}[${index}] must be a non-empty string record id for relation columns`,
        );
      }
    }
    return;
  }
  if (fieldType === "lookup" || fieldType === "rollup") {
    if (kind !== "null") {
      throw new Error(`${template}: ${label} must be null for ${fieldType} columns`);
    }
    return;
  }
  if (kind === "object" || kind === "array" || kind === "undefined" || kind === "function") {
    throw new Error(`${template}: ${label} must be a JSON primitive or null`);
  }
}

function isSqlIdentifier(name) {
  return typeof name === "string" && /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}

function normalizeDirectory(entry, template) {
  if (typeof entry === "string") {
    assertSafePath(entry, template);
    return { path: entry };
  }
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error(`${template}: directory entry must be a string or object`);
  }
  if (typeof entry.path !== "string") {
    throw new Error(`${template}: directory object requires path`);
  }
  assertSafePath(entry.path, template);
  const directory = { path: entry.path };
  for (const key of ["purpose", "defaultKind", "icon"]) {
    if (entry[key] === undefined) continue;
    if (typeof entry[key] !== "string" || entry[key].trim().length === 0) {
      throw new Error(`${template}: directory ${key} must be a non-empty string`);
    }
    directory[key] = entry[key];
  }
  return directory;
}

function normalizeWorkspaceDefaults(defaults, template) {
  if (!defaults || typeof defaults !== "object" || Array.isArray(defaults)) {
    throw new Error(`${template}: workspaceDefaults must be an object`);
  }
  for (const key of Object.keys(defaults)) {
    if (!WORKSPACE_DEFAULT_KEYS.includes(key)) {
      throw new Error(`${template}: unknown workspaceDefaults key ${key}`);
    }
  }
  if (typeof defaults.quickNoteDirectory !== "string") {
    throw new Error(`${template}: workspaceDefaults.quickNoteDirectory is required`);
  }
  assertSafePath(defaults.quickNoteDirectory, template);
  const normalized = { quickNoteDirectory: defaults.quickNoteDirectory };
  for (const key of WORKSPACE_DEFAULT_KEYS.slice(1)) {
    if (defaults[key] === undefined) continue;
    normalized[key] = normalizeOptionalPath(defaults[key], template, `workspaceDefaults.${key}`);
  }
  return normalized;
}

function warnMissingDefaultDirectories(template, directories, workspaceDefaults) {
  // Blank (and similar) templates intentionally declare defaults without folders.
  if (directories.length === 0) return;
  const known = new Set(directories.map((entry) => entry.path));
  for (const key of WORKSPACE_DEFAULT_KEYS) {
    const value = workspaceDefaults[key];
    if (typeof value !== "string" || value.length === 0) continue;
    if (!known.has(value)) {
      console.warn(
        `${template}: workspaceDefaults.${key}=${JSON.stringify(value)} is not a seeded directory`,
      );
    }
  }
}

function normalizeOptionalPath(value, template, label) {
  if (typeof value !== "string") {
    throw new Error(`${template}: ${label} must be a string`);
  }
  assertSafePath(value, template);
  return value;
}

function assertSafePath(path, template) {
  if (
    typeof path !== "string" ||
    path.length === 0 ||
    path.includes("\\") ||
    path.startsWith("/") ||
    path.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new Error(`${template}: unsafe path ${JSON.stringify(path)}`);
  }
}

function validateLinks(template, directories, files) {
  const resources = new Set(directories.map((path) => `${path}/`));
  for (const file of files) {
    resources.add(file.path);
    if (/\.md$/i.test(file.path)) resources.add(file.path.replace(/\.md$/i, ""));
  }
  for (const file of files.filter((entry) => /\.md$/i.test(entry.path))) {
    const source = readFileSync(file.source, "utf8");
    for (const match of source.matchAll(/\[\[([^\]|#]+)(?:#[^\]|]+)?(?:\|[^\]]+)?\]\]/g)) {
      const target = match[1].trim();
      if (!seededTargetExists(resources, file.path, target, false)) {
        throw new Error(`${template}: unresolved link [[${target}]] in ${file.path}`);
      }
    }
    for (const match of source.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      const target = match[1].trim().split("#", 1)[0];
      if (
        target &&
        !/^(?:https?:|mailto:|data:)/i.test(target) &&
        !seededTargetExists(resources, file.path, target, true)
      ) {
        throw new Error(`${template}: unresolved Markdown link (${target}) in ${file.path}`);
      }
    }
  }
}

function seededTargetExists(resources, sourcePath, rawTarget, relative) {
  let target = rawTarget.replace(/^<|>$/g, "").replace(/\\/g, "/");
  if (relative && !target.startsWith("/")) {
    target = posix.normalize(posix.join(posix.dirname(sourcePath), target));
  } else {
    target = target.replace(/^\/+/, "");
  }
  if (resources.has(target)) return true;
  if (!target.endsWith("/") && posix.extname(target) === "") {
    return resources.has(`${target}.md`);
  }
  return false;
}

function rustString(value) {
  return JSON.stringify(value);
}

function rustOptionString(value) {
  return value === undefined || value === null ? "None" : `Some(${rustString(value)})`;
}

function rustDirectory(directory) {
  return `SeedDirectory { path: ${rustString(directory.path)}, purpose: ${rustOptionString(directory.purpose)}, default_kind: ${rustOptionString(directory.defaultKind)}, icon: ${rustOptionString(directory.icon)} }`;
}

function rustDataColumn(column) {
  return `SeedDataColumn { name: ${rustString(column.name)}, field_type: ${rustString(column.type)}, relation_table: ${rustOptionString(column.relation_table)}, junction_table: ${rustOptionString(column.junction_table)}, lookup_relation: ${rustOptionString(column.lookup_relation)}, lookup_field: ${rustOptionString(column.lookup_field)}, rollup_relation: ${rustOptionString(column.rollup_relation)}, rollup_aggregate: ${rustOptionString(column.rollup_aggregate)}, rollup_field: ${rustOptionString(column.rollup_field)} }`;
}

function rustDataForm(form) {
  const fields = form.fields.map((field) => rustString(field)).join(",\n                    ");
  return `SeedDataForm {
                name: ${rustString(form.name)},
                table: ${rustString(form.table)},
                fields: &[${fields ? `\n                    ${fields}\n                ` : ""}],
                title: ${rustOptionString(form.title)},
                description: ${rustOptionString(form.description)},
            }`;
}

function rustActionDefaults(defaults) {
  if (!defaults || Object.keys(defaults).length === 0) {
    return "&[]";
  }
  const pairs = Object.entries(defaults)
    .map(([field, value]) => `(${rustString(field)}, ${rustString(value)})`)
    .join(",\n                    ");
  return `&[\n                    ${pairs}\n                ]`;
}

function rustDataAction(action) {
  const actionDef = action.action;
  return `SeedDataAction {
                name: ${rustString(action.name)},
                label: ${rustString(action.label)},
                table: ${rustString(action.table)},
                scope: ${rustString(action.scope)},
                action_type: ${rustString(actionDef.type)},
                form: ${rustOptionString(actionDef.form)},
                field: ${rustOptionString(actionDef.field)},
                value: ${rustOptionString(actionDef.value)},
                url: ${rustOptionString(actionDef.url)},
                defaults: ${rustActionDefaults(actionDef.defaults)},
            }`;
}

function rustDataInterface(iface) {
  const views = iface.views.map((view) => rustString(view)).join(",\n                    ");
  const forms = iface.forms.map((form) => rustString(form)).join(",\n                    ");
  return `SeedDataInterface {
                name: ${rustString(iface.name)},
                views: &[${views ? `\n                    ${views}\n                ` : ""}],
                forms: &[${forms ? `\n                    ${forms}\n                ` : ""}],
                title: ${rustOptionString(iface.title)},
                description: ${rustOptionString(iface.description)},
            }`;
}

function rustDataView(view) {
  const columns = view.columns.map((column) => rustString(column)).join(",\n                    ");
  return `SeedDataView {
                name: ${rustString(view.name)},
                layout: ${rustString(view.layout)},
                group_by: ${rustOptionString(view.group_by)},
                cover_field: ${rustOptionString(view.cover_field)},
                date_field: ${rustOptionString(view.date_field)},
                columns: &[${columns ? `\n                    ${columns}\n                ` : ""}],
            }`;
}

function rustExtraTable(tableDef) {
  const columns = tableDef.columns.map(rustDataColumn).join(",\n                ");
  const rows = tableDef.rows.map((row) => rustString(JSON.stringify(row))).join(",\n                ");
  return `SeedDataExtraTable {
                table: ${rustString(tableDef.table)},
                columns: &[
                    ${columns}
                ],
                rows_json: &[
                    ${rows}
                ],
            }`;
}

function rustDataPackage(packageDef) {
  const columns = packageDef.columns.map(rustDataColumn).join(",\n                ");
  const rows = packageDef.rows.map((row) => rustString(JSON.stringify(row))).join(",\n                ");
  const views = packageDef.views.map(rustDataView).join(",\n                ");
  const forms = packageDef.forms.map(rustDataForm).join(",\n                ");
  const actions = (packageDef.actions ?? []).map(rustDataAction).join(",\n                ");
  const interfaces = (packageDef.interfaces ?? []).map(rustDataInterface).join(",\n                ");
  const extraTables = packageDef.extraTables.map(rustExtraTable).join(",\n                ");
  return `SeedDataPackage {
            path: ${rustString(packageDef.path)},
            title: ${rustString(packageDef.title)},
            table: ${rustString(packageDef.table)},
            columns: &[
                ${columns}
            ],
            rows_json: &[
                ${rows}
            ],
            extra_tables: &[${extraTables ? `\n                ${extraTables}\n            ` : ""}],
            views: &[${views ? `\n                ${views}\n            ` : ""}],
            forms: &[${forms ? `\n                ${forms}\n            ` : ""}],
            actions: &[${actions ? `\n                ${actions}\n            ` : ""}],
            interfaces: &[${interfaces ? `\n                ${interfaces}\n            ` : ""}],
        }`;
}

function emitRust(templates) {
  const entries = templates.map((template) => {
    const files = template.files
      .map(
        (file) =>
          `SeedFile { path: ${rustString(file.path)}, bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates/workspaces/${template.id}/files/${file.path}")) }`,
      )
      .join(",\n            ");
    const directories = template.directories.map(rustDirectory).join(",\n            ");
    const dataPackages = template.dataPackages.map(rustDataPackage).join(",\n            ");
    const defaults = template.workspaceDefaults;
    return `GeneratedTemplate {
        id: ${rustString(template.id)},
        order: ${template.order},
        name: ${rustString(template.name)},
        category: ${rustString(template.category)},
        description: ${rustString(template.description)},
        visibility: ${rustString(template.visibility)},
        recommended: ${template.recommended},
        recommended_title: ${rustString(template.recommendedTitle)},
        directories: &[
            ${directories}
        ],
        preview: &[${template.preview.map(rustString).join(", ")}],
        capabilities: &[${template.capabilities.map(rustString).join(", ")}],
        quick_note_directory: ${rustString(defaults.quickNoteDirectory ?? "Inbox")},
        daily_note_directory: ${rustOptionString(defaults.dailyNoteDirectory)},
        attachments_directory: ${rustOptionString(defaults.attachmentsDirectory)},
        template_directory: ${rustOptionString(defaults.templateDirectory)},
        archive_directory: ${rustOptionString(defaults.archiveDirectory)},
        open_on_create: ${rustOptionString(template.openOnCreate)},
        files: &[
            ${files}
        ],
        data_packages: &[
            ${dataPackages}
        ],
    }`;
  });
  return `// GENERATED by scripts/compile-templates.mjs — do not edit.
pub(crate) static GENERATED_TEMPLATES: &[GeneratedTemplate] = &[
    ${entries.join(",\n    ")}
];
`;
}

function emitTypeScript(templates) {
  const catalog = templates.map((template) => ({
    id: template.id,
    order: template.order,
    name: template.name,
    category: template.category,
    description: template.description,
    visibility: template.visibility,
    recommended: template.recommended,
    recommendedTitle: template.recommendedTitle,
    directories: template.directories,
    preview: template.preview,
    capabilities: template.capabilities,
    workspaceDefaults: template.workspaceDefaults,
    ...(template.openOnCreate !== undefined ? { openOnCreate: template.openOnCreate } : {}),
    dataPackages: template.dataPackages,
  }));
  return `// GENERATED by scripts/compile-templates.mjs — do not edit.
export const GENERATED_TEMPLATE_CATALOG = ${JSON.stringify(catalog, null, 2)} as const;
`;
}

function fileExtension(path) {
  const base = path.split("/").pop() ?? path;
  const dot = base.lastIndexOf(".");
  return dot >= 0 ? base.slice(dot + 1).toLowerCase() : "";
}

function resourceKindForPath(path) {
  const extension = fileExtension(path);
  if (extension === "md" || extension === "markdown") return "page";
  if (extension === "canvas") return "canvas";
  if (extension === "ipynb") return "notebook";
  return "file";
}

function formatIdForPath(path) {
  const extension = fileExtension(path);
  if (extension === "svg" || IMAGE_EXTENSIONS.has(extension)) return "file:image";
  if (extension === "pdf") return "file:pdf";
  if (["txt", "md", "markdown", "log", "csv", "tsv"].includes(extension)) return "file:text";
  if (CODE_EXTENSIONS.has(extension)) return "file:code";
  if (extension === "json") return "file:json";
  if (["yaml", "yml"].includes(extension)) return "file:yaml";
  return "file:unknown";
}

function isUtf8TextSeed(path) {
  return TEXT_BODY_EXTENSIONS.has(fileExtension(path));
}

function demoRowId(row, index, table) {
  if (typeof row.name === "string") {
    const slug = row.name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    if (slug) {
      return table === undefined ? `${DEMO_WORKSPACE_ID}-${slug}` : `${DEMO_WORKSPACE_ID}-${table}-${slug}`;
    }
  }
  const suffix = table === undefined ? `row-${index}` : `${table}-row-${index}`;
  return `${DEMO_WORKSPACE_ID}-${suffix}`;
}

function buildDemoTableSnapshot(tableName, columns, rows, rowIdsByTable, legacyIds = false) {
  const demoRows = rows.map((row, index) => {
    const id = demoRowId(row, index, legacyIds ? undefined : tableName);
    const values = { id: { Text: id } };
    for (const column of columns) {
      if (column.type === "relation" || column.type === "lookup" || column.type === "rollup") {
        continue;
      }
      const raw = Object.prototype.hasOwnProperty.call(row, column.name) ? row[column.name] : null;
      values[column.name] = cellValueForField(raw, column.type);
    }
    return { id, values, row };
  });
  const rowIdsByName = new Map();
  for (const entry of demoRows) {
    if (typeof entry.row.name === "string") {
      rowIdsByName.set(entry.row.name, entry.id);
    } else if (columns.some((column) => column.type === "relation")) {
      throw new Error(
        `${DEMO_TEMPLATE_ID}: demo relation seeds require a name column on table ${tableName}`,
      );
    }
  }
  rowIdsByTable.set(tableName, rowIdsByName);
  for (const entry of demoRows) {
    for (const column of columns) {
      if (column.type === "relation") {
        const raw = Object.prototype.hasOwnProperty.call(entry.row, column.name)
          ? entry.row[column.name]
          : null;
        if (raw === null) {
          entry.values[column.name] = { Null: null };
          continue;
        }
        if (!Array.isArray(raw)) {
          throw new Error(
            `${DEMO_TEMPLATE_ID}: relation column ${column.name} must be a JSON array of record ids`,
          );
        }
        const recordIds = resolveDemoRelationRefs(raw, column.relation_table, rowIdsByTable);
        entry.values[column.name] = { Relation: { record_ids: recordIds } };
        continue;
      }
      if (column.type === "lookup") {
        entry.values[column.name] = { Lookup: { values: [] } };
        continue;
      }
      if (column.type === "rollup") {
        entry.values[column.name] = { Rollup: { value: null } };
      }
    }
  }
  return demoRows.map(({ id, values }) => ({ id, values }));
}

function resolveDemoRelationRefs(references, targetTable, rowIdsByTable) {
  const rowIdsByName = rowIdsByTable.get(targetTable) ?? new Map();
  const targetIds = new Set(rowIdsByName.values());
  return references.map((reference) => {
    if (targetIds.has(reference)) return reference;
    const resolved = rowIdsByName.get(reference);
    if (!resolved) {
      throw new Error(
        `${DEMO_TEMPLATE_ID}: relation reference ${JSON.stringify(reference)} not found in table ${targetTable}`,
      );
    }
    return resolved;
  });
}

function packageTableColumns(packageDef) {
  const tables = (packageDef.extraTables ?? []).map((table) => ({
    table: table.table,
    columns: table.columns,
  }));
  tables.push({ table: packageDef.table, columns: packageDef.columns });
  return tables;
}

function findReciprocalRelationColumn(tableColumns, sourceTable, targetTable) {
  const entry = tableColumns.find((table) => table.table === sourceTable);
  if (!entry) return undefined;
  return entry.columns.find(
    (column) => column.type === "relation" && column.relation_table === targetTable,
  )?.name;
}

function syncDemoBacklinkRelations(packageDef, tableSnapshots) {
  const tableColumns = packageTableColumns(packageDef);
  for (const { table, columns } of tableColumns) {
    for (const column of columns) {
      if (column.type !== "relation" || column.relation_table === undefined) continue;
      const backlink = findReciprocalRelationColumn(
        tableColumns,
        column.relation_table,
        table,
      );
      if (!backlink) continue;
      const targetRows = tableSnapshots.get(table) ?? [];
      const sourceRows = tableSnapshots.get(column.relation_table) ?? [];
      const links = new Map(targetRows.map((row) => [row.id, []]));
      for (const sourceRow of sourceRows) {
        const relation = sourceRow.values[backlink];
        if (!relation || !("Relation" in relation)) continue;
        for (const targetId of relation.Relation.record_ids) {
          const linked = links.get(targetId);
          if (!linked) {
            throw new Error(
              `${DEMO_TEMPLATE_ID}: backlink ${backlink} on ${column.relation_table} references unknown row in ${table}`,
            );
          }
          linked.push(sourceRow.id);
        }
      }
      for (const row of targetRows) {
        const recordIds = links.get(row.id) ?? [];
        if (recordIds.length === 0) {
          row.values[column.name] = { Null: null };
        } else {
          row.values[column.name] = { Relation: { record_ids: recordIds } };
        }
      }
    }
  }
}

function demoCellAsText(value) {
  if (!value) return null;
  if ("Text" in value) return value.Text;
  if ("Date" in value) return value.Date;
  if ("Integer" in value) return String(value.Integer);
  if ("Decimal" in value) return String(value.Decimal);
  return null;
}

function resolveDemoComputedColumns(tableName, columns, rows, tableSnapshots) {
  for (const row of rows) {
    for (const column of columns) {
      if (column.type === "lookup") {
        const relationColumn = columns.find((entry) => entry.name === column.lookup_relation);
        const relation = row.values[column.lookup_relation];
        const values = [];
        if (relationColumn && relation && "Relation" in relation) {
          const targetRows = tableSnapshots.get(relationColumn.relation_table) ?? [];
          for (const recordId of relation.Relation.record_ids) {
            const targetRow = targetRows.find((entry) => entry.id === recordId);
            const text = demoCellAsText(targetRow?.values[column.lookup_field]);
            if (text !== null) values.push(text);
          }
        }
        row.values[column.name] = { Lookup: { values } };
        continue;
      }
      if (column.type === "rollup") {
        const relation = row.values[column.rollup_relation];
        let count = 0;
        if (relation && "Relation" in relation) {
          if (column.rollup_aggregate === "count" && !column.rollup_field) {
            count = relation.Relation.record_ids.length;
          } else {
            const relationColumn = columns.find((entry) => entry.name === column.rollup_relation);
            const targetRows = tableSnapshots.get(relationColumn?.relation_table);
            for (const recordId of relation.Relation.record_ids) {
              const targetRow = targetRows?.find((entry) => entry.id === recordId);
              const projected = targetRow?.values[column.rollup_field];
              if (projected && !("Null" in projected)) {
                count += 1;
              }
            }
          }
        }
        row.values[column.name] = { Rollup: { value: count } };
      }
    }
  }
}

function demoColumnMetadata(column) {
  return {
    name: column.name,
    field_type: column.type,
    sqlite_type: SQLITE_TYPES[column.type],
    ...(column.relation_table === undefined ? {} : { relation_table: column.relation_table }),
    ...(column.junction_table === undefined ? {} : { junction_table: column.junction_table }),
    ...(column.lookup_relation === undefined ? {} : { lookup_relation: column.lookup_relation }),
    ...(column.lookup_field === undefined ? {} : { lookup_field: column.lookup_field }),
    ...(column.rollup_relation === undefined ? {} : { rollup_relation: column.rollup_relation }),
    ...(column.rollup_aggregate === undefined
      ? {}
      : { rollup_aggregate: column.rollup_aggregate }),
    ...(column.rollup_field === undefined ? {} : { rollup_field: column.rollup_field }),
  };
}

function cellValueForField(value, fieldType) {
  if (value === null) return { Null: null };
  switch (fieldType) {
    case "integer":
      return { Integer: Number(value) };
    case "decimal":
      return { Decimal: Number(value) };
    case "boolean":
      return { Boolean: Boolean(value) };
    case "date":
      return { Date: String(value) };
    case "text":
    case "long_text":
      return { Text: String(value) };
    case "relation": {
      if (!Array.isArray(value)) {
        throw new Error("relation cells must be a JSON array of record ids");
      }
      return { Relation: { record_ids: value.map((item) => String(item)) } };
    }
    default: {
      const _exhaustive = fieldType;
      throw new Error(`unsupported demo field type ${String(_exhaustive)}`);
    }
  }
}

function buildDemoResources(template) {
  const resources = [];
  for (const directory of template.directories) {
    resources.push({ path: directory.path, kind: "folder" });
  }
  for (const file of template.files) {
    const kind = resourceKindForPath(file.path);
    if (kind === "file") {
      resources.push({ path: file.path, kind, formatId: formatIdForPath(file.path) });
    } else {
      resources.push({ path: file.path, kind });
    }
  }
  for (const packageDef of template.dataPackages) {
    resources.push({ path: packageDef.path, kind: "data-app" });
  }
  resources.sort((left, right) => {
    if (left.path === "Home.md") return -1;
    if (right.path === "Home.md") return 1;
    return left.path.localeCompare(right.path);
  });
  return resources;
}

function buildDemoPages(template) {
  const pages = {};
  for (const file of template.files) {
    if (resourceKindForPath(file.path) !== "page") continue;
    pages[file.path] = readFileSync(file.source, "utf8");
  }
  return pages;
}

function buildDemoTextFiles(template) {
  const textFiles = {};
  for (const file of template.files) {
    if (resourceKindForPath(file.path) !== "file") continue;
    if (!isUtf8TextSeed(file.path)) continue;
    textFiles[file.path] = readFileSync(file.source, "utf8");
  }
  return textFiles;
}

function buildDemoNotebooks(template) {
  const notebooks = {};
  for (const file of template.files) {
    if (resourceKindForPath(file.path) !== "notebook") continue;
    notebooks[file.path] = readFileSync(file.source, "utf8");
  }
  return notebooks;
}

function buildDemoCanvas(template) {
  const canvas = template.files.find((file) => resourceKindForPath(file.path) === "canvas");
  if (!canvas) {
    throw new Error(`${DEMO_TEMPLATE_ID}: expected at least one .canvas seed for browser demo`);
  }
  return JSON.parse(readFileSync(canvas.source, "utf8"));
}

function demoViewLayoutFromSeed(view) {
  const layout = {
    name: view.name,
    layout_type: view.layout,
  };
  if (view.group_by !== undefined) layout.group_by = view.group_by;
  if (view.cover_field !== undefined) layout.cover_field = view.cover_field;
  if (view.date_field !== undefined) layout.date_field = view.date_field;
  return layout;
}

function buildDemoFormCatalog(packageDef) {
  return packageDef.forms.map((form) => ({
    name: form.name,
    table: form.table,
    fields: form.fields,
    ...(form.title === undefined ? {} : { title: form.title }),
    ...(form.description === undefined ? {} : { description: form.description }),
  }));
}

function buildDemoActionCatalog(packageDef) {
  return (packageDef.actions ?? []).map((action) => ({
    name: action.name,
    label: action.label,
    table: action.table,
    scope: action.scope,
    action: action.action,
  }));
}

function buildDemoInterfaceCatalog(packageDef) {
  return (packageDef.interfaces ?? []).map((iface) => ({
    name: iface.name,
    views: iface.views,
    forms: iface.forms,
    ...(iface.title === undefined ? {} : { title: iface.title }),
    ...(iface.description === undefined ? {} : { description: iface.description }),
  }));
}

function buildDemoViewCatalog(packageDef) {
  const views = [
    { name: "All", layout_type: "grid" },
    ...packageDef.views.map(demoViewLayoutFromSeed),
  ];
  views.sort((left, right) => left.name.localeCompare(right.name));
  return views;
}

function demoSnapshotLayoutFields(view) {
  const fields = { layout_type: view.layout_type };
  if (view.group_by !== undefined) fields.group_by = view.group_by;
  if (view.cover_field !== undefined) fields.cover_field = view.cover_field;
  if (view.date_field !== undefined) fields.date_field = view.date_field;
  return fields;
}

function buildDemoDataAppFromPackage(packageDef) {
  const columns = [
    { name: "id", field_type: "text", sqlite_type: "TEXT" },
    ...packageDef.columns.map(demoColumnMetadata),
  ];
  const rowIdsByTable = new Map();
  const tableSnapshots = new Map();
  for (const extraTable of packageDef.extraTables ?? []) {
    tableSnapshots.set(
      extraTable.table,
      buildDemoTableSnapshot(extraTable.table, extraTable.columns, extraTable.rows, rowIdsByTable),
    );
  }
  const demoRows = buildDemoTableSnapshot(
    packageDef.table,
    packageDef.columns,
    packageDef.rows,
    rowIdsByTable,
    true,
  );
  tableSnapshots.set(packageDef.table, demoRows);
  syncDemoBacklinkRelations(packageDef, tableSnapshots);
  for (const extraTable of packageDef.extraTables ?? []) {
    resolveDemoComputedColumns(
      extraTable.table,
      extraTable.columns,
      tableSnapshots.get(extraTable.table) ?? [],
      tableSnapshots,
    );
  }
  resolveDemoComputedColumns(
    packageDef.table,
    packageDef.columns,
    tableSnapshots.get(packageDef.table) ?? [],
    tableSnapshots,
  );
  const rowIds = new Set();
  for (const rows of tableSnapshots.values()) {
    for (const row of rows) {
      if (rowIds.has(row.id)) {
        throw new Error(
          `${DEMO_TEMPLATE_ID}: duplicate demo data-app row id ${JSON.stringify(row.id)}`,
        );
      }
      rowIds.add(row.id);
    }
  }
  const relation_targets = {};
  for (const [tableName, rows] of tableSnapshots) {
    relation_targets[tableName] = rows;
  }
  const saved_views = buildDemoViewCatalog(packageDef);
  const available_views = saved_views.map((view) => view.name);
  const active_view = "All";
  const activeView = saved_views.find((view) => view.name === active_view);
  if (!activeView) {
    throw new Error(
      `${DEMO_TEMPLATE_ID}: missing default All view for ${packageDef.path}`,
    );
  }
  return {
    title: packageDef.title,
    default_table: packageDef.table,
    package_revision: "demo:0",
    columns,
    rows: demoRows,
    row_offset: 0,
    row_limit: demoRows.length,
    row_total: demoRows.length,
    has_more: false,
    available_views,
    active_view,
    filters: [],
    saved_views,
    ...(Object.keys(relation_targets).length > 0 ? { relation_targets } : {}),
    ...demoSnapshotLayoutFields(activeView),
  };
}

function buildDemoDataApp(template) {
  const packageDef =
    template.dataPackages.find((entry) => entry.path === "CRM.data") ?? template.dataPackages[0];
  if (!packageDef) {
    throw new Error(`${DEMO_TEMPLATE_ID}: expected a dataPackages entry for browser demo CRM`);
  }
  return buildDemoDataAppFromPackage(packageDef);
}

function buildDemoDataApps(template) {
  const apps = {};
  for (const packageDef of template.dataPackages) {
    apps[packageDef.path] = buildDemoDataAppFromPackage(packageDef);
  }
  return apps;
}

function buildDemoPackageFormsByPath(template) {
  const forms = {};
  for (const packageDef of template.dataPackages) {
    forms[packageDef.path] = buildDemoFormCatalog(packageDef);
  }
  return forms;
}

function buildDemoPackageActionsByPath(template) {
  const actions = {};
  for (const packageDef of template.dataPackages) {
    actions[packageDef.path] = buildDemoActionCatalog(packageDef);
  }
  return actions;
}

function buildDemoPackageInterfacesByPath(template) {
  const interfaces = {};
  for (const packageDef of template.dataPackages) {
    interfaces[packageDef.path] = buildDemoInterfaceCatalog(packageDef);
  }
  return interfaces;
}

export function emitDemoWorkspace(templates) {
  const template = templates.find((entry) => entry.id === DEMO_TEMPLATE_ID);
  if (!template) {
    throw new Error(`missing ${DEMO_TEMPLATE_ID} workspace template`);
  }
  const defaults = { quickNoteDirectory: template.workspaceDefaults.quickNoteDirectory };
  for (const key of ["dailyNoteDirectory", "attachmentsDirectory", "templateDirectory", "archiveDirectory"]) {
    if (template.workspaceDefaults[key] !== undefined) {
      defaults[key] = template.workspaceDefaults[key];
    }
  }
  // Mirrors the manifest's editable `directories:` section for browser review.
  const directoryPurposes = {};
  for (const directory of template.directories) {
    if (directory.purpose) directoryPurposes[directory.path] = directory.purpose;
  }
  const snapshot = {
    root: "/Users/you/Lattice/Workspaces/First Look",
    title: "First Look",
    id: DEMO_WORKSPACE_ID,
    capabilities: template.capabilities,
    defaults,
    sourceTemplate: DEMO_TEMPLATE_ID,
    ...(Object.keys(directoryPurposes).length > 0 ? { directoryPurposes } : {}),
    manifestRevision: "demo:0",
    resources: buildDemoResources(template),
  };
  const crmPackage =
    template.dataPackages.find((entry) => entry.path === "CRM.data") ?? template.dataPackages[0];
  if (!crmPackage) {
    throw new Error(`${DEMO_TEMPLATE_ID}: expected a dataPackages entry for browser demo CRM`);
  }
  const demoDataApps = buildDemoDataApps(template);
  const module = {
    demoSnapshot: snapshot,
    demoCanvas: buildDemoCanvas(template),
    demoDataApp: demoDataApps[crmPackage.path],
    demoDataApps,
    demoPackageForms: buildDemoFormCatalog(crmPackage),
    demoPackageFormsByPath: buildDemoPackageFormsByPath(template),
    demoPackageActions: buildDemoActionCatalog(crmPackage),
    demoPackageActionsByPath: buildDemoPackageActionsByPath(template),
    demoPackageInterfaces: buildDemoInterfaceCatalog(crmPackage),
    demoPackageInterfacesByPath: buildDemoPackageInterfacesByPath(template),
    demoPages: buildDemoPages(template),
    demoTextFiles: buildDemoTextFiles(template),
    demoNotebooks: buildDemoNotebooks(template),
  };
  return `// GENERATED by scripts/compile-templates.mjs — do not edit.
import type { WorkspaceSnapshot } from "./types";
import type { DataAppSnapshot } from "./data/types";
import type { FormSummary } from "./data/forms";
import type { ActionSummary } from "./data/actions";
import type { InterfaceSummary } from "./data/interfaces";

export const demoSnapshot: WorkspaceSnapshot = ${JSON.stringify(module.demoSnapshot, null, 2)};

export const demoCanvas = ${JSON.stringify(module.demoCanvas, null, 2)};

export const demoDataApp: DataAppSnapshot = ${JSON.stringify(module.demoDataApp, null, 2)};

export const demoDataApps: Record<string, DataAppSnapshot> = ${JSON.stringify(module.demoDataApps, null, 2)};

export const demoPackageForms: FormSummary[] = ${JSON.stringify(module.demoPackageForms, null, 2)};

export const demoPackageFormsByPath: Record<string, FormSummary[]> = ${JSON.stringify(module.demoPackageFormsByPath, null, 2)};

export const demoPackageActions: ActionSummary[] = ${JSON.stringify(module.demoPackageActions, null, 2)};

export const demoPackageActionsByPath: Record<string, ActionSummary[]> = ${JSON.stringify(module.demoPackageActionsByPath, null, 2)};

export const demoPackageInterfaces: InterfaceSummary[] = ${JSON.stringify(module.demoPackageInterfaces, null, 2)};

export const demoPackageInterfacesByPath: Record<string, InterfaceSummary[]> = ${JSON.stringify(module.demoPackageInterfacesByPath, null, 2)};

export const demoPages: Record<string, string> = ${JSON.stringify(module.demoPages, null, 2)};

export const demoTextFiles: Record<string, string> = ${JSON.stringify(module.demoTextFiles, null, 2)};

export const demoNotebooks: Record<string, string> = ${JSON.stringify(module.demoNotebooks, null, 2)};
`;
}

function main() {
  const templates = compileTemplates();
  writeFileSync(RUST_OUT, emitRust(templates));
  writeFileSync(TS_OUT, emitTypeScript(templates));
  writeFileSync(DEMO_OUT, emitDemoWorkspace(templates));
  console.log(
    `compiled ${templates.length} workspace templates to ${relative(ROOT, RUST_OUT).split(sep).join("/")}, ${relative(ROOT, TS_OUT).split(sep).join("/")}, and ${relative(ROOT, DEMO_OUT).split(sep).join("/")}`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
