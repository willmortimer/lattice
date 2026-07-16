import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { compileTemplates } from "./compile-templates.mjs";

test("workspace template packages validate", () => {
  const templates = compileTemplates();
  assert.deepEqual(
    templates.filter((template) => template.visibility === "gallery").map((template) => template.id),
    ["personal", "project", "research", "data-lab", "blank"],
  );
  assert.equal(templates.filter((template) => template.recommended).length, 1);
});

function fixture(overrides = {}, content = "# Home\n") {
  const root = mkdtempSync(join(tmpdir(), "lattice-template-test-"));
  const template = join(root, "fixture");
  mkdirSync(join(template, "files"), { recursive: true });
  const manifest = {
    format: "lattice-workspace-template",
    version: 1,
    id: "fixture",
    order: 1,
    name: "Fixture",
    category: "Test",
    description: "Fixture",
    visibility: "gallery",
    recommendedTitle: "Fixture",
    directories: [],
    preview: ["Home.md"],
    files: ["Home.md"],
    capabilities: ["pages"],
    workspaceDefaults: { quickNoteDirectory: "Inbox" },
    ...overrides,
  };
  writeFileSync(join(template, "template.json"), JSON.stringify(manifest));
  writeFileSync(join(template, "files", "Home.md"), content);
  return root;
}

test("template compiler rejects unsafe destinations", () => {
  assert.throws(
    () => compileTemplates(fixture({ directories: ["../escape"] })),
    /unsafe path/,
  );
});

test("template compiler rejects unresolved seeded links", () => {
  assert.throws(
    () => compileTemplates(fixture({}, "# Home\n\n[[Missing]]\n")),
    /unresolved link/,
  );
});
