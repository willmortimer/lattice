import { invoke } from "@tauri-apps/api/core";

import { inBrowser } from "../demo";
import { GENERATED_TEMPLATE_CATALOG } from "../templateCatalog.generated";
import type { WorkspaceSnapshot } from "../types";

export type TemplateVisibility = "gallery" | "legacy" | "sample";

export type TemplateCategory =
  | "Everyday"
  | "Work"
  | "Knowledge & Research"
  | "Data & Advanced"
  | "Sample";

export interface TemplateDirectory {
  path: string;
  purpose?: string;
  defaultKind?: string;
  icon?: string;
}

export interface TemplateWorkspaceDefaults {
  quickNoteDirectory: string;
  dailyNoteDirectory?: string;
  attachmentsDirectory?: string;
  templateDirectory?: string;
  archiveDirectory?: string;
}

export interface TemplateDescriptor {
  id: string;
  order: number;
  name: string;
  category: TemplateCategory | string;
  description: string;
  visibility: TemplateVisibility;
  recommended: boolean;
  recommendedTitle: string;
  directories: TemplateDirectory[];
  preview: string[];
  capabilities: string[];
  quickNoteDirectory: string;
  dailyNoteDirectory?: string;
  attachmentsDirectory?: string;
  templateDirectory?: string;
  archiveDirectory?: string;
  openOnCreate?: string;
}

export interface WorkspaceProvisionOutcome {
  workspace: WorkspaceSnapshot;
  defaultWorkspaceStatus: "not-requested" | "updated" | "failed";
  diagnostics: Array<{ code: string; message: string; retryable: boolean }>;
}

function normalizeDirectory(
  directory: string | { path: string; purpose?: string; defaultKind?: string; icon?: string },
): TemplateDirectory {
  if (typeof directory === "string") return { path: directory };
  return {
    path: directory.path,
    ...(directory.purpose !== undefined ? { purpose: directory.purpose } : {}),
    ...(directory.defaultKind !== undefined ? { defaultKind: directory.defaultKind } : {}),
    ...(directory.icon !== undefined ? { icon: directory.icon } : {}),
  };
}

function generatedDescriptors(): TemplateDescriptor[] {
  return GENERATED_TEMPLATE_CATALOG.map((template) => {
    const defaults = template.workspaceDefaults as TemplateWorkspaceDefaults;
    return {
      id: template.id,
      order: template.order,
      name: template.name,
      category: template.category,
      description: template.description,
      visibility: template.visibility,
      recommended: template.recommended,
      recommendedTitle: template.recommendedTitle,
      directories: template.directories.map((directory) => normalizeDirectory(directory)),
      preview: [...template.preview],
      capabilities: [...template.capabilities],
      quickNoteDirectory: defaults.quickNoteDirectory,
      ...(defaults.dailyNoteDirectory !== undefined
        ? { dailyNoteDirectory: defaults.dailyNoteDirectory }
        : {}),
      ...(defaults.attachmentsDirectory !== undefined
        ? { attachmentsDirectory: defaults.attachmentsDirectory }
        : {}),
      ...(defaults.templateDirectory !== undefined
        ? { templateDirectory: defaults.templateDirectory }
        : {}),
      ...(defaults.archiveDirectory !== undefined
        ? { archiveDirectory: defaults.archiveDirectory }
        : {}),
      ...("openOnCreate" in template && template.openOnCreate !== undefined
        ? { openOnCreate: template.openOnCreate }
        : {}),
    };
  });
}

/** Build a path → purpose map from the embedded template catalog. */
export function directoryPurposesFromCatalog(
  templateId?: string | null,
): Readonly<Record<string, string>> {
  const purposes: Record<string, string> = {};
  const templates =
    templateId == null
      ? GENERATED_TEMPLATE_CATALOG
      : GENERATED_TEMPLATE_CATALOG.filter((template) => template.id === templateId);
  for (const template of templates) {
    for (const directory of template.directories) {
      const normalized = normalizeDirectory(directory);
      if (normalized.purpose) purposes[normalized.path] = normalized.purpose;
    }
  }
  return purposes;
}

export async function listTemplates(): Promise<TemplateDescriptor[]> {
  if (inBrowser) return generatedDescriptors();
  return invoke("list_templates");
}

export async function provisionWorkspace(input: {
  path: string;
  title: string;
  template: string;
  setDefault: boolean;
  initializeExisting: boolean;
}): Promise<WorkspaceProvisionOutcome> {
  if (inBrowser) {
    return {
      workspace: {
        root: input.path,
        title: input.title,
        id: "browser-template",
        resources: [
          { path: "Home.md", kind: "page" },
          { path: "Inbox", kind: "folder" },
        ],
        capabilities: ["pages", "canvas", "sqlite"],
        defaults: { quickNoteDirectory: "Inbox" },
        manifestRevision: "demo:0",
      },
      defaultWorkspaceStatus: input.setDefault ? "updated" : "not-requested",
      diagnostics: [],
    };
  }
  return invoke("create_workspace", input);
}
