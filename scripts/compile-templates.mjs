#!/usr/bin/env node

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TEMPLATE_ROOT = join(ROOT, "templates", "workspaces");
const RUST_OUT = join(ROOT, "crates", "lattice-core", "src", "template_catalog.generated.rs");
const TS_OUT = join(ROOT, "apps", "desktop", "src", "templateCatalog.generated.ts");
const MAX_FILES = 128;
const MAX_BYTES = 2 * 1024 * 1024;
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
const FIELD_TYPES = new Set(["text", "long_text", "integer", "decimal", "boolean", "date"]);
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
    columnNames.add(column.name);
    columns.push({ name: column.name, type: column.type });
  }

  const rows = entry.rows.map((row, rowIndex) => {
    const rowLabel = `${label}.rows[${rowIndex}]`;
    if (!row || typeof row !== "object" || Array.isArray(row)) {
      throw new Error(`${template}: ${rowLabel} must be an object`);
    }
    for (const key of Object.keys(row)) {
      if (!columnNames.has(key)) {
        throw new Error(`${template}: ${rowLabel} unknown column ${key}`);
      }
      assertJsonCell(row[key], template, `${rowLabel}.${key}`);
    }
    return row;
  });

  return {
    path: entry.path,
    title: entry.title,
    table: entry.table,
    columns,
    rows,
  };
}

function assertJsonCell(value, template, label) {
  const kind = value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
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
  return `SeedDataColumn { name: ${rustString(column.name)}, field_type: ${rustString(column.type)} }`;
}

function rustDataPackage(packageDef) {
  const columns = packageDef.columns.map(rustDataColumn).join(",\n                ");
  const rows = packageDef.rows.map((row) => rustString(JSON.stringify(row))).join(",\n                ");
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

function main() {
  const templates = compileTemplates();
  writeFileSync(RUST_OUT, emitRust(templates));
  writeFileSync(TS_OUT, emitTypeScript(templates));
  console.log(
    `compiled ${templates.length} workspace templates to ${relative(ROOT, RUST_OUT).split(sep).join("/")} and ${relative(ROOT, TS_OUT).split(sep).join("/")}`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
