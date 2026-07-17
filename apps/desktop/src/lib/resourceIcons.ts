import type { Icon } from "@phosphor-icons/react";
import {
  BracketsCurly,
  File,
  FileCode,
  FileImage,
  FilePdf,
  FileText,
  Folder,
  FolderOpen,
  Question,
} from "@phosphor-icons/react";

import { deriveResourceFormatId } from "../resourceRendererRegistry";
import type { Resource, ResourceKind } from "../types";

export type ResourceTreeIconDecision =
  | { type: "kind-mark"; kind: ResourceKind }
  | { type: "phosphor"; Icon: Icon };

const FORMAT_ICONS: Record<string, Icon> = {
  "file:image": FileImage,
  "file:pdf": FilePdf,
  "file:code": FileCode,
  "file:json": BracketsCurly,
  "file:yaml": FileCode,
  "file:text": FileText,
  "file:unknown": File,
};

/** Stable Phosphor icon for ordinary files; Lattice kinds keep KindMark. */
export function formatIdTreeIcon(formatId: string): Icon {
  return FORMAT_ICONS[formatId] ?? Question;
}

export function resourceTreeIcon(resource: Resource): ResourceTreeIconDecision {
  if (resource.kind === "file") {
    const formatId = deriveResourceFormatId(resource);
    return {
      type: "phosphor",
      Icon: formatIdTreeIcon(formatId),
    };
  }
  return { type: "kind-mark", kind: resource.kind };
}

export function folderTreeIcon(collapsed: boolean): Icon {
  return collapsed ? Folder : FolderOpen;
}
