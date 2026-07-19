import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { compileTemplates, emitDemoWorkspace } from "./compile-templates.mjs";

const CATEGORIES = [
  "Everyday",
  "Work",
  "Knowledge & Research",
  "Data & Advanced",
  "Sample",
];

test("workspace template packages validate", () => {
  const templates = compileTemplates();
  assert.deepEqual(
    templates.filter((template) => template.visibility === "gallery").map((template) => template.id),
    ["personal", "project", "writing", "ops", "research", "second-brain", "data-lab", "dev-notebook", "blank"],
  );
  assert.equal(templates.filter((template) => template.recommended).length, 1);
  assert.ok(templates.every((template) => template.version === 2));
  assert.ok(templates.every((template) => CATEGORIES.includes(template.category)));
  assert.equal(templates.find((template) => template.id === "personal")?.category, "Everyday");
  assert.equal(templates.find((template) => template.id === "project")?.category, "Work");
  assert.equal(
    templates.find((template) => template.id === "research")?.category,
    "Knowledge & Research",
  );
  assert.equal(
    templates.find((template) => template.id === "second-brain")?.category,
    "Knowledge & Research",
  );
  assert.equal(
    templates.find((template) => template.id === "data-lab")?.category,
    "Data & Advanced",
  );
  assert.equal(
    templates.find((template) => template.id === "dev-notebook")?.category,
    "Work",
  );
  assert.equal(templates.find((template) => template.id === "blank")?.category, "Data & Advanced");
  assert.equal(templates.find((template) => template.id === "demo")?.category, "Sample");
  assert.equal(templates.find((template) => template.id === "team")?.category, "Work");
});

function fixture(overrides = {}, content = "# Home\n") {
  const root = mkdtempSync(join(tmpdir(), "lattice-template-test-"));
  const template = join(root, "fixture");
  mkdirSync(join(template, "files"), { recursive: true });
  const manifest = {
    format: "lattice-workspace-template",
    version: 2,
    id: "fixture",
    order: 1,
    name: "Fixture",
    category: "Everyday",
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

test("template compiler accepts directory objects and openOnCreate", () => {
  const templates = compileTemplates(
    fixture({
      directories: [
        { path: "Inbox", purpose: "Raw captures", defaultKind: "page", icon: "inbox" },
        "Archive",
      ],
      workspaceDefaults: {
        quickNoteDirectory: "Inbox",
        dailyNoteDirectory: "Journal",
        archiveDirectory: "Archive",
      },
      openOnCreate: "Home.md",
    }),
  );
  assert.equal(templates.length, 1);
  assert.deepEqual(templates[0].directories, [
    { path: "Inbox", purpose: "Raw captures", defaultKind: "page", icon: "inbox" },
    { path: "Archive" },
  ]);
  assert.equal(templates[0].openOnCreate, "Home.md");
  assert.equal(templates[0].workspaceDefaults.dailyNoteDirectory, "Journal");
});

test("template compiler rejects invalid categories and sample mismatches", () => {
  assert.throws(() => compileTemplates(fixture({ category: "Focused work" })), /invalid category/);
  assert.throws(
    () => compileTemplates(fixture({ category: "Sample", visibility: "gallery" })),
    /reserved for visibility: sample/,
  );
  assert.throws(
    () => compileTemplates(fixture({ category: "Everyday", visibility: "sample" })),
    /requires category Sample/,
  );
});

test("template compiler accepts version 1 fixtures during migration", () => {
  const templates = compileTemplates(fixture({ version: 1 }));
  assert.equal(templates[0].version, 1);
});

test("template compiler accepts declarative dataPackages", () => {
  const templates = compileTemplates(
    fixture({
      directories: ["Data"],
      dataPackages: [
        {
          path: "Data/Contacts.data",
          title: "Contacts",
          table: "contacts",
          columns: [
            { name: "name", type: "text" },
            { name: "email", type: "text" },
          ],
          rows: [{ name: "Ada", email: "ada@example.com" }],
        },
      ],
    }),
  );
  assert.equal(templates[0].dataPackages.length, 1);
  assert.equal(templates[0].dataPackages[0].path, "Data/Contacts.data");
  assert.equal(templates[0].dataPackages[0].rows.length, 1);
});

test("template compiler accepts declarative dataPackage views", () => {
  const templates = compileTemplates(
    fixture({
      directories: ["Data"],
      dataPackages: [
        {
          path: "Data/Tasks.data",
          title: "Tasks",
          table: "tasks",
          columns: [
            { name: "name", type: "text" },
            { name: "status", type: "text" },
            { name: "due_date", type: "date" },
          ],
          rows: [{ name: "Ship views", status: "Active", due_date: "2026-08-01" }],
          views: [
            {
              name: "Board",
              layout: "board",
              group_by: "status",
              columns: ["name", "status"],
            },
            {
              name: "Calendar",
              layout: "calendar",
              date_field: "due_date",
            },
          ],
        },
      ],
    }),
  );
  const views = templates[0].dataPackages[0].views;
  assert.equal(views.length, 2);
  assert.equal(views[0].layout, "board");
  assert.equal(views[0].group_by, "status");
  assert.equal(views[1].layout, "calendar");
  assert.equal(views[1].date_field, "due_date");
});

test("template compiler accepts declarative dataPackage forms", () => {
  const templates = compileTemplates(
    fixture({
      directories: ["Data"],
      dataPackages: [
        {
          path: "Data/Contacts.data",
          title: "Contacts",
          table: "contacts",
          columns: [
            { name: "name", type: "text" },
            { name: "email", type: "text" },
            { name: "status", type: "text" },
          ],
          rows: [{ name: "Ada", email: "ada@example.com", status: "Active" }],
          forms: [
            {
              name: "ContactIntake",
              fields: ["name", "email", "status"],
              title: "Contact intake",
              description: "Create a contact.",
            },
          ],
        },
      ],
    }),
  );
  const forms = templates[0].dataPackages[0].forms;
  assert.equal(forms.length, 1);
  assert.equal(forms[0].name, "ContactIntake");
  assert.equal(forms[0].table, "contacts");
  assert.deepEqual(forms[0].fields, ["name", "email", "status"]);
  assert.equal(forms[0].title, "Contact intake");
});

test("demo template emits kitchen-sink browser fixture", () => {
  const templates = compileTemplates();
  const demo = templates.find((template) => template.id === "demo");
  assert.ok(demo);
  assert.equal(demo.openOnCreate, "Home.md");
  assert.ok(demo.dataPackages.some((entry) => entry.path === "CRM.data"));
  assert.ok(demo.dataPackages.some((entry) => entry.path === "Projects/Delivery.data"));
  assert.ok(demo.dataPackages.some((entry) => entry.path === "Data/Metrics.data"));
  assert.ok(demo.dataPackages.some((entry) => entry.path === "OKRs.data"));
  assert.ok(demo.files.some((file) => file.path === "Research/Architecture.md"));
  assert.ok(demo.files.some((file) => file.path === "Data/sample.csv"));
  assert.ok(demo.files.some((file) => file.path === "Resources/mark.svg"));
  assert.ok(demo.directories.some((entry) => entry.path === "Inbox" && entry.purpose));

  const source = emitDemoWorkspace(templates);
  assert.match(source, /export const demoSnapshot/);
  assert.match(source, /export const demoDataApps/);
  assert.match(source, /Projects\/Delivery\.data/);
  assert.match(source, /Data\/Metrics\.data/);
  assert.match(source, /OKRs\.data/);
  assert.match(source, /"title": "First Look"/);
  assert.match(source, /"sourceTemplate": "demo"/);
  assert.match(source, /"id": "0198-demo"/);
  assert.match(source, /"id": "0198-demo-ada-lovelace"/);
  assert.match(source, /"id": "0198-demo-grace-hopper"/);
  assert.match(source, /Research\/Architecture\.md/);
  assert.match(source, /Resources\/config\.json/);
  assert.match(source, /Resources\/mark\.svg/);
  assert.match(source, /"formatId": "file:text"/);
  assert.match(source, /CRM\.data/);
  assert.match(source, /"relation_targets"/);
  assert.match(source, /mermaid/);
  assert.match(source, /export const demoNotebooks/);
  assert.match(source, /Notebooks\/CRM exploration\.ipynb/);
  assert.match(source, /# CRM exploration/);
  assert.match(source, /export const demoPackageForms/);
  assert.match(source, /"name": "ContactIntake"/);
});

test("template compiler rejects invalid dataPackages", () => {
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          dataPackages: [
            {
              path: "Contacts.sqlite",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "name", type: "text" }],
              rows: [],
            },
          ],
        }),
      ),
    /must end with \.data/,
  );
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          dataPackages: [
            {
              path: "Contacts.data",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "name", type: "text" }],
              rows: [{ name: "Ada", unknown: true }],
            },
          ],
        }),
      ),
    /unknown column/,
  );
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          dataPackages: [
            {
              path: "Contacts.data",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "id", type: "text" }],
              rows: [],
            },
          ],
        }),
      ),
    /cannot be id/,
  );
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          files: ["Home.md", "Contacts.data"],
          dataPackages: [
            {
              path: "Contacts.data",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "name", type: "text" }],
              rows: [],
            },
          ],
        }),
      ),
    /duplicate destination/,
  );
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          dataPackages: [
            {
              path: "Contacts.data",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "name", type: "text" }],
              rows: [],
              views: [{ name: "Board", layout: "board", group_by: "missing" }],
            },
          ],
        }),
      ),
    /unknown column/,
  );
  assert.throws(
    () =>
      compileTemplates(
        fixture({
          dataPackages: [
            {
              path: "Contacts.data",
              title: "Contacts",
              table: "contacts",
              columns: [{ name: "name", type: "text" }],
              rows: [],
              views: [{ name: "Board", layout: "grid", group_by: "name" }],
            },
          ],
        }),
      ),
    /only supported for board views/,
  );
});
