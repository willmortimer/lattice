import { invoke } from "@tauri-apps/api/core";

import { inBrowser } from "../demo";
import { GENERATED_TEMPLATE_CATALOG } from "../templateCatalog.generated";
import type { WorkspaceSnapshot } from "../types";

export type TemplateVisibility = "gallery" | "legacy" | "sample";

export interface TemplateDescriptor {
  id: string;
  order: number;
  name: string;
  category: string;
  description: string;
  visibility: TemplateVisibility;
  recommended: boolean;
  recommendedTitle: string;
  directories: string[];
  preview: string[];
  capabilities: string[];
  quickNoteDirectory: string;
}

export interface WorkspaceProvisionOutcome {
  workspace: WorkspaceSnapshot;
  defaultWorkspaceStatus: "not-requested" | "updated" | "failed";
  diagnostics: Array<{ code: string; message: string; retryable: boolean }>;
}

function generatedDescriptors(): TemplateDescriptor[] {
  return GENERATED_TEMPLATE_CATALOG.map((template) => ({
    ...template,
    directories: [...template.directories],
    preview: [...template.preview],
    capabilities: [...template.capabilities],
    quickNoteDirectory: template.workspaceDefaults.quickNoteDirectory,
  }));
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
