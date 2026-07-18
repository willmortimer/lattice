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
]);
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
    if (!column || typeof column !== "object" || Array.isArray(column)) {
      throw new Error(`${template}: ${columnLabel} must be an object`);
    }
    if (typeof column.name !== "string" || !isSqlIdentifier(column.name)) {
      throw new Error(`${template}: ${columnLabel}.name must be a valid SQL identifier`);
    }
    if (column.name === "id") {
      throw new Error(`${template}: ${columnLabel}.name cannot be id (reserved)`);
    }
    if (columnNames.has(column.name)) {
      throw new Error(`${template}: ${columnLabel} duplicate column ${column.name}`);
    }
    if (typeof column.type !== "string" || !FIELD_TYPES.has(column.type)) {
      throw new Error(
        `${template}: ${columnLabel}.type must be one of ${[...FIELD_TYPES].join(", ")}`,
      );
    }
    let relationTable;
    if (column.type === "relation") {
      if (typeof column.relation_table !== "string" || !isSqlIdentifier(column.relation_table)) {
        throw new Error(
          `${template}: ${columnLabel}.relation_table must be a valid SQL identifier for relation columns`,
        );
      }
      relationTable = column.relation_table;
    } else if (column.relation_table !== undefined) {
      throw new Error(
        `${template}: ${columnLabel}.relation_table is only supported for relation columns`,
      );
    }
    columnNames.add(column.name);
    columns.push({
      name: column.name,
      type: column.type,
      ...(relationTable === undefined ? {} : { relation_table: relationTable }),
    });
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

  return {
    path: entry.path,
    title: entry.title,
    table: entry.table,
    columns,
    rows,
    views,
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
  return `SeedDataColumn { name: ${rustString(column.name)}, field_type: ${rustString(column.type)}, relation_table: ${rustOptionString(column.relation_table)} }`;
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

function rustDataPackage(packageDef) {
  const columns = packageDef.columns.map(rustDataColumn).join(",\n                ");
  const rows = packageDef.rows.map((row) => rustString(JSON.stringify(row))).join(",\n                ");
  const views = packageDef.views.map(rustDataView).join(",\n                ");
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
            views: &[${views ? `\n                ${views}\n            ` : ""}],
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
  return "file";
}

function formatIdForPath(path) {
  const extension = fileExtension(path);
  if (extension === "svg") return "file:text";
  if (IMAGE_EXTENSIONS.has(extension)) return "file:image";
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

function demoRowId(row, index) {
  if (typeof row.name === "string") {
    const slug = row.name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    if (slug) return `${DEMO_WORKSPACE_ID}-${slug}`;
  }
  return `${DEMO_WORKSPACE_ID}-row-${index}`;
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

function resolveDemoRelationRefs(references, targetTable, rowIdsByName, knownIds) {
  return references.map((reference) => {
    if (knownIds.has(reference)) return reference;
    const resolved = rowIdsByName.get(reference);
    if (!resolved) {
      throw new Error(
        `${DEMO_TEMPLATE_ID}: relation reference ${JSON.stringify(reference)} not found in table ${targetTable}`,
      );
    }
    return resolved;
  });
}

function buildDemoDataApp(template) {
  const packageDef =
    template.dataPackages.find((entry) => entry.path === "CRM.data") ?? template.dataPackages[0];
  if (!packageDef) {
    throw new Error(`${DEMO_TEMPLATE_ID}: expected a dataPackages entry for browser demo CRM`);
  }
  const columns = [
    { name: "id", field_type: "text", sqlite_type: "TEXT" },
    ...packageDef.columns.map((column) => ({
      name: column.name,
      field_type: column.type,
      sqlite_type: SQLITE_TYPES[column.type],
      ...(column.relation_table === undefined
        ? {}
        : { relation_table: column.relation_table }),
    })),
  ];
  const rows = packageDef.rows.map((row, index) => {
    const id = demoRowId(row, index);
    const values = { id: { Text: id } };
    for (const column of packageDef.columns) {
      if (column.type === "relation") continue;
      const raw = Object.prototype.hasOwnProperty.call(row, column.name) ? row[column.name] : null;
      values[column.name] = cellValueForField(raw, column.type);
    }
    return { id, values, row };
  });
  const rowIdsByName = new Map(
    rows.map((entry) => {
      if (typeof entry.row.name !== "string") {
        throw new Error(`${DEMO_TEMPLATE_ID}: demo CRM rows require a name column for relation seeds`);
      }
      return [entry.row.name, entry.id];
    }),
  );
  const knownIds = new Set(rows.map((entry) => entry.id));
  for (const entry of rows) {
    for (const column of packageDef.columns) {
      if (column.type !== "relation") continue;
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
      const recordIds = resolveDemoRelationRefs(
        raw,
        column.relation_table,
        rowIdsByName,
        knownIds,
      );
      entry.values[column.name] = { Relation: { record_ids: recordIds } };
    }
  }
  const demoRows = rows.map(({ id, values }) => ({ id, values }));
  const rowIds = new Set();
  for (const row of demoRows) {
    if (rowIds.has(row.id)) {
      throw new Error(
        `${DEMO_TEMPLATE_ID}: duplicate demo data-app row id ${JSON.stringify(row.id)}`,
      );
    }
    rowIds.add(row.id);
  }
  const relation_targets = {};
  for (const column of packageDef.columns) {
    if (column.type !== "relation" || !column.relation_table) continue;
    if (column.relation_table === packageDef.table) {
      relation_targets[column.relation_table] = demoRows;
    }
  }
  const saved_views = buildDemoViewCatalog(packageDef);
  const available_views = saved_views.map((view) => view.name);
  const active_view = "All";
  const activeView = saved_views.find((view) => view.name === active_view);
  if (!activeView) {
    throw new Error(`${DEMO_TEMPLATE_ID}: missing default All view in browser demo CRM`);
  }
  return {
    title: packageDef.title,
    default_table: packageDef.table,
    package_revision: "demo:0",
    columns,
    rows: demoRows,
    available_views,
    active_view,
    filters: [],
    saved_views,
    ...(Object.keys(relation_targets).length > 0 ? { relation_targets } : {}),
    ...demoSnapshotLayoutFields(activeView),
  };
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
  const module = {
    demoSnapshot: snapshot,
    demoCanvas: buildDemoCanvas(template),
    demoDataApp: buildDemoDataApp(template),
    demoPages: buildDemoPages(template),
    demoTextFiles: buildDemoTextFiles(template),
  };
  return `// GENERATED by scripts/compile-templates.mjs — do not edit.
import type { WorkspaceSnapshot } from "./types";
import type { DataAppSnapshot } from "./data/types";

export const demoSnapshot: WorkspaceSnapshot = ${JSON.stringify(module.demoSnapshot, null, 2)};

export const demoCanvas = ${JSON.stringify(module.demoCanvas, null, 2)};

export const demoDataApp: DataAppSnapshot = ${JSON.stringify(module.demoDataApp, null, 2)};

export const demoPages: Record<string, string> = ${JSON.stringify(module.demoPages, null, 2)};

export const demoTextFiles: Record<string, string> = ${JSON.stringify(module.demoTextFiles, null, 2)};
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
