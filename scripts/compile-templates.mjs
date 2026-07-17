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
  if (
    !Array.isArray(manifest.files) ||
    !Array.isArray(manifest.directories) ||
    manifest.files.length + manifest.directories.length > MAX_FILES
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
  for (const path of [...directories.map((entry) => entry.path), ...manifest.files]) {
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
  };
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

function emitRust(templates) {
  const entries = templates.map((template) => {
    const files = template.files
      .map(
        (file) =>
          `SeedFile { path: ${rustString(file.path)}, bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates/workspaces/${template.id}/files/${file.path}")) }`,
      )
      .join(",\n            ");
    const directories = template.directories.map(rustDirectory).join(",\n            ");
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
