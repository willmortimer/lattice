import type { ResourceKind } from "../types";

/**
 * Mirrors `ResourceKind::classify` in crates/lattice-core/src/resource.rs,
 * minus the filesystem `is_dir` check (canvas file nodes only carry a
 * path string, not a stat). Package kinds are recognized purely by their
 * dotted suffix, matching the workspace naming convention.
 */
export function classifyPath(path: string): ResourceKind {
  // Strip a trailing slash so "Foo.data/" classifies the same as "Foo.data".
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  const name = trimmed.split("/").pop() ?? trimmed;

  if (name.endsWith(".workflow.yaml") || name.endsWith(".workflow.yml")) {
    return "workflow";
  }
  if (name.endsWith(".derived.yaml") || name.endsWith(".derived.yml")) {
    return "derived";
  }

  const dotIndex = name.lastIndexOf(".");
  const ext = dotIndex >= 0 ? name.slice(dotIndex + 1) : "";

  switch (ext) {
    case "data":
      return "data-app";
    case "dataset":
      return "dataset";
    case "ink":
      return "ink";
    case "artifact":
      return "artifact";
    case "app":
      return "app";
    case "task":
      return "task";
    case "md":
    case "markdown":
      return "page";
    case "canvas":
      return "canvas";
    case "ipynb":
      return "notebook";
    default:
      return "file";
  }
}
