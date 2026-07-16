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
  if (manifest.format !== "lattice-workspace-template" || manifest.version !== 1) {
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
  if (
    !Array.isArray(manifest.files) ||
    !Array.isArray(manifest.directories) ||
    manifest.files.length + manifest.directories.length > MAX_FILES
  ) {
    throw new Error(`${directoryName}: too many files`);
  }

  const destinations = new Set(["lattice.yaml"]);
  for (const path of [...manifest.directories, ...manifest.files]) {
    assertSafePath(path, directoryName);
    const normalized = posix.normalize(path);
    if (destinations.has(normalized)) {
      throw new Error(`${directoryName}: duplicate destination ${normalized}`);
    }
    destinations.add(normalized);
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
  validateLinks(directoryName, manifest.directories, files);

  return {
    ...manifest,
    recommended: Boolean(manifest.recommended),
    files,
  };
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

function rustArray(values) {
  return `&[${values.map(rustString).join(", ")}]`;
}

function emitRust(templates) {
  const entries = templates.map((template) => {
    const files = template.files
      .map(
        (file) =>
          `SeedFile { path: ${rustString(file.path)}, bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates/workspaces/${template.id}/files/${file.path}")) }`,
      )
      .join(",\n            ");
    return `GeneratedTemplate {
        id: ${rustString(template.id)},
        order: ${template.order},
        name: ${rustString(template.name)},
        category: ${rustString(template.category)},
        description: ${rustString(template.description)},
        visibility: ${rustString(template.visibility)},
        recommended: ${template.recommended},
        recommended_title: ${rustString(template.recommendedTitle)},
        directories: ${rustArray(template.directories)},
        preview: ${rustArray(template.preview)},
        capabilities: ${rustArray(template.capabilities)},
        quick_note_directory: ${rustString(template.workspaceDefaults.quickNoteDirectory ?? "Inbox")},
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
