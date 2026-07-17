export type ReconciliationDisposition = "ignore" | "conflict" | "reload";

export function dispositionForModifiedResource(args: {
  eventPath: string;
  currentPath: string | null;
  eventRevision: string | undefined;
  currentRevision: string | null;
  unsaved: boolean;
}): ReconciliationDisposition {
  if (!args.currentPath || args.eventPath !== args.currentPath) return "ignore";
  if (args.eventRevision && args.eventRevision === args.currentRevision) return "ignore";
  return args.unsaved ? "conflict" : "reload";
}

export function pathIsRemoved(path: string, removedPath: string): boolean {
  return path === removedPath || path.startsWith(`${removedPath}/`);
}

export function shouldClearRenamedPath(path: string, from: string): boolean {
  return path === from;
}

export function conflictSiblingPath(path: string, date = new Date().toISOString().slice(0, 10)): string {
  const slash = path.lastIndexOf("/");
  const dir = slash >= 0 ? path.slice(0, slash + 1) : "";
  const base = slash >= 0 ? path.slice(slash + 1) : path;
  const dot = base.lastIndexOf(".");
  const stem = dot > 0 ? base.slice(0, dot) : base;
  const ext = dot > 0 ? base.slice(dot) : "";
  return `${dir}${stem} (conflict ${date})${ext}`;
}
