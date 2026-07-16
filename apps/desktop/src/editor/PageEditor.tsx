import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { editorExtensions } from "./extensions";
import {
  joinFrontmatter,
  parseMarkdownToJSON,
  serializeJSONToMarkdown,
  splitFrontmatter,
} from "./markdown";
import { StaleRevisionError, type PageIO } from "./pageIO";

const AUTOSAVE_DELAY_MS = 800;
/** How long the "Saved" indicator lingers before fading back to idle. */
const SAVED_INDICATOR_MS = 1500;

export type SaveState =
  | { status: "idle" }
  | { status: "dirty" }
  | { status: "saving" }
  | { status: "saved" }
  | { status: "conflict"; message: string }
  | { status: "error"; message: string };

/** Whether `state` represents an edit not yet durably saved. */
export function isUnsaved(state: SaveState): boolean {
  return state.status !== "idle" && state.status !== "saved";
}

/** Short label for the save-state indicator in the breadcrumb. */
export function saveIndicatorText(state: SaveState): string {
  switch (state.status) {
    case "idle":
      return "";
    case "dirty":
      return "Edited";
    case "saving":
      return "Saving…";
    case "saved":
      return "Saved";
    case "conflict":
      return "Save conflict";
    case "error":
      return "Save failed";
    default:
      // Exhaustiveness guard: a new SaveState variant must be handled above.
      return state satisfies never;
  }
}

interface PageEditorProps {
  raw: string;
  revision: string | null;
  io: PageIO;
  onSaveStateChange?: (state: SaveState) => void;
}

/**
 * Tiptap page editor. Owns markdown parse/serialize (via `./markdown`),
 * debounced autosave, Cmd/Ctrl+S, and a conflict banner for stale saves.
 *
 * Frontmatter is split off on load and reattached verbatim on save — v0
 * shows it collapsed and never parses or edits it (docs/07).
 */
export function PageEditor({ raw, revision, io, onSaveStateChange }: PageEditorProps) {
  const [{ frontmatter }] = useState(() => splitFrontmatter(raw));
  const initialDoc = useMemo(() => parseMarkdownToJSON(splitFrontmatter(raw).body), [raw]);

  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });

  // Notify the parent as an effect (not from inside the state updater above)
  // so this never fires while React is still rendering `PageEditor` itself.
  useEffect(() => {
    onSaveStateChange?.(saveState);
  }, [saveState, onSaveStateChange]);

  const revisionRef = useRef(revision);
  const savingRef = useRef(false);
  const autosaveTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const savedIndicatorTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);

  const editor = useEditor({
    extensions: editorExtensions,
    content: initialDoc,
    onUpdate: () => {
      setSaveState((prev) => (prev.status === "conflict" ? prev : { status: "dirty" }));
      scheduleAutosave();
    },
  });

  const clearAutosaveTimer = useCallback(() => {
    if (autosaveTimer.current !== null) {
      window.clearTimeout(autosaveTimer.current);
      autosaveTimer.current = null;
    }
  }, []);

  const performSave = useCallback(async () => {
    if (savingRef.current) return;
    // A conflict must be resolved (Reload / Keep editing) before further
    // saves are attempted — otherwise we'd just refetch the same conflict.
    if (saveState.status === "conflict") return;

    clearAutosaveTimer();
    savingRef.current = true;
    setSaveState({ status: "saving" });

    const body = serializeJSONToMarkdown(editor.getJSON());
    const fullRaw = joinFrontmatter(frontmatter, body);

    try {
      const nextRevision = await io.save(fullRaw, revisionRef.current);
      revisionRef.current = nextRevision;
      setSaveState({ status: "saved" });
      if (savedIndicatorTimer.current !== null) window.clearTimeout(savedIndicatorTimer.current);
      savedIndicatorTimer.current = window.setTimeout(() => {
        setSaveState((prev) => (prev.status === "saved" ? { status: "idle" } : prev));
      }, SAVED_INDICATOR_MS);
    } catch (err) {
      if (err instanceof StaleRevisionError) {
        setSaveState({ status: "conflict", message: err.message });
      } else {
        setSaveState({ status: "error", message: err instanceof Error ? err.message : String(err) });
      }
    } finally {
      savingRef.current = false;
    }
  }, [editor, frontmatter, io, saveState.status, clearAutosaveTimer, setSaveState]);

  const performSaveRef = useRef(performSave);
  performSaveRef.current = performSave;

  function scheduleAutosave() {
    clearAutosaveTimer();
    autosaveTimer.current = window.setTimeout(() => {
      void performSaveRef.current();
    }, AUTOSAVE_DELAY_MS);
  }

  // Cmd/Ctrl+S: save immediately, bypassing the debounce.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void performSaveRef.current();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    return () => {
      clearAutosaveTimer();
      if (savedIndicatorTimer.current !== null) window.clearTimeout(savedIndicatorTimer.current);
    };
  }, [clearAutosaveTimer]);

  async function handleReload() {
    clearAutosaveTimer();
    const snapshot = await io.load();
    revisionRef.current = snapshot.revision;
    editor.commands.setContent(parseMarkdownToJSON(splitFrontmatter(snapshot.raw).body));
    setSaveState({ status: "idle" });
  }

  function handleKeepEditing() {
    setSaveState({ status: "dirty" });
    scheduleAutosave();
  }

  return (
    <div className="page-editor">
      {saveState.status === "conflict" && (
        <div className="conflict-banner">
          <span className="conflict-banner-copy">
            This page changed on disk since it was opened — saving now would overwrite that
            change.
          </span>
          <div className="conflict-banner-actions">
            <button className="secondary-button" onClick={handleKeepEditing}>
              Keep editing
            </button>
            <button className="primary-button" onClick={() => void handleReload()}>
              Reload
            </button>
          </div>
        </div>
      )}

      {frontmatter && (
        <details className="frontmatter-block">
          <summary>Frontmatter</summary>
          <pre className="frontmatter-content">{frontmatter.trim()}</pre>
        </details>
      )}

      {saveState.status === "error" && <p className="error-text">{saveState.message}</p>}

      <EditorContent editor={editor} className="markdown-body page-editor-content" />
    </div>
  );
}
