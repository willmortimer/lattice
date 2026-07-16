import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { ConflictEnvelope } from "./ConflictEnvelope";
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
  /**
   * Fires whenever the revision this editor considers its clean base
   * changes (initial load, successful save, or an explicit reload) — not on
   * every keystroke. Lets a parent detect "this incoming external-edit
   * event is just an echo of a save I already know about" without forcing
   * a remount on every autosave.
   */
  onRevisionChange?: (revision: string | null) => void;
}

/** Imperative escape hatch for a parent that needs the live buffer outside
 * the normal load/save cycle — e.g. an external-edit conflict envelope's
 * "keep local" and "keep both" actions, which must save (or copy aside)
 * whatever is currently in the editor, not the `raw` it was mounted with. */
export interface PageEditorHandle {
  /** The editor's current content, serialized back to full page text
   * (frontmatter included, verbatim). */
  getRaw(): string;
}

/**
 * Tiptap page editor. Owns markdown parse/serialize (via `./markdown`),
 * debounced autosave, Cmd/Ctrl+S, and a conflict banner for stale saves.
 *
 * Frontmatter is split off on load and reattached verbatim on save — v0
 * shows it collapsed and never parses or edits it (docs/07).
 */
export const PageEditor = forwardRef<PageEditorHandle, PageEditorProps>(function PageEditor(
  { raw, revision, io, onSaveStateChange, onRevisionChange },
  ref,
) {
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

  const onRevisionChangeRef = useRef(onRevisionChange);
  onRevisionChangeRef.current = onRevisionChange;
  /** Every place that updates `revisionRef` funnels through here so the
   * parent's view of "this editor's clean base revision" never drifts from
   * the editor's own. */
  const setRevision = useCallback((next: string | null) => {
    revisionRef.current = next;
    onRevisionChangeRef.current?.(next);
  }, []);

  // The initial revision (on mount) is a "revision change" too — the parent
  // must learn it even though nothing was saved or reloaded yet.
  useEffect(() => {
    setRevision(revision);
    // Only on mount: `revision` here is this instance's initial prop value,
    // and `PageEditor` is remounted (fresh `key`) rather than re-fed a new
    // `revision` prop in place.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const editor = useEditor({
    extensions: editorExtensions,
    content: initialDoc,
    onUpdate: () => {
      setSaveState((prev) => (prev.status === "conflict" ? prev : { status: "dirty" }));
      scheduleAutosave();
    },
  });

  useImperativeHandle(
    ref,
    () => ({
      getRaw: () => joinFrontmatter(frontmatter, serializeJSONToMarkdown(editor.getJSON())),
    }),
    [editor, frontmatter],
  );

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
      setRevision(nextRevision);
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
  }, [editor, frontmatter, io, saveState.status, clearAutosaveTimer, setRevision]);

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
    setRevision(snapshot.revision);
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
        <ConflictEnvelope
          message="This page changed on disk since it was opened — saving now would overwrite that change."
          actions={[
            { label: "Keep editing", onClick: handleKeepEditing },
            { label: "Reload", onClick: () => void handleReload(), variant: "primary" },
          ]}
        />
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
});
