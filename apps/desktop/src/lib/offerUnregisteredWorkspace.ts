export type UnregisteredKind = "file" | "folder";

export type UnregisteredOpenPayload = {
  path: string;
  kind?: UnregisteredKind;
};

export type UnregisteredOffer =
  | { action: "toast"; path: string }
  | { action: "add-folder"; path: string; title: string };

export type NewWorkspaceDialogMode = "new-child" | "existing";
export type NewWorkspaceDialogStep = "gallery" | "details";

export type NewWorkspaceDialogInitialState = {
  step: NewWorkspaceDialogStep;
  mode: NewWorkspaceDialogMode;
  parentPath: string | null;
  title: string;
  titleTouched: boolean;
  makeDefault: boolean;
};

/** Last path segment, used as the default workspace title. */
export function folderTitleFromPath(path: string): string {
  const name = path.split(/[/\\]/).filter(Boolean).pop()?.trim();
  return name && name.length > 0 ? name : "Workspace";
}

/**
 * Folders can be initialized as a workspace. Stray files stay toast-only —
 * never wrap the parent directory unless the payload is a folder.
 */
export function offerForUnregisteredOpen(payload: UnregisteredOpenPayload): UnregisteredOffer {
  const path = payload.path.trim();
  switch (payload.kind) {
    case "folder":
      if (!path) return { action: "toast", path: payload.path };
      return {
        action: "add-folder",
        path,
        title: folderTitleFromPath(path),
      };
    case "file":
    case undefined:
      return { action: "toast", path: payload.path };
    default: {
      const _exhaustive: never = payload.kind;
      return { action: "toast", path: payload.path };
    }
  }
}

export function initialNewWorkspaceDialogState(args: {
  hasValidDefault: boolean;
  existingFolderPath: string | null;
}): NewWorkspaceDialogInitialState {
  if (args.existingFolderPath) {
    return {
      step: "details",
      mode: "existing",
      parentPath: args.existingFolderPath,
      title: folderTitleFromPath(args.existingFolderPath),
      titleTouched: true,
      makeDefault: !args.hasValidDefault,
    };
  }
  return {
    step: "gallery",
    mode: "new-child",
    parentPath: null,
    title: "Personal",
    titleTouched: false,
    makeDefault: !args.hasValidDefault,
  };
}
