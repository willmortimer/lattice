import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Editor, Extensions } from "@tiptap/core";
import { EditorContent, ReactNodeViewRenderer, useEditor } from "@tiptap/react";

import { CodeBlockView } from "./CodeBlockView";
import { ConflictEnvelope } from "./ConflictEnvelope";
import { editorExtensions } from "./extensions";
import { ImageView } from "./ImageView";
import {
  joinFrontmatter,
  parseMarkdownToJSON,
  serializeJSONToMarkdown,
  splitFrontmatter,
} from "./markdown";
import { StaleRevisionError, type PageIO } from "./pageIO";
import { type SaveState } from "./saveState";
export { isUnsaved, saveIndicatorText, type SaveState } from "./saveState";

/** How long the "Saved" indicator lingers before fading back to idle. */
const SAVED_INDICATOR_MS = 1500;

interface SlashMenuState {
  from: number;
  to: number;
  query: string;
  left: number;
  top: number;
}

const SLASH_COMMANDS = [
  { id: "text", label: "Text", description: "Plain paragraph" },
  { id: "heading-1", label: "Heading 1", description: "Large section heading" },
  { id: "heading-2", label: "Heading 2", description: "Medium section heading" },
  { id: "heading-3", label: "Heading 3", description: "Small section heading" },
  { id: "bullet-list", label: "Bulleted list", description: "Unordered list" },
  { id: "ordered-list", label: "Numbered list", description: "Ordered list" },
  { id: "quote", label: "Quote", description: "Quoted block" },
  { id: "code", label: "Code block", description: "Fenced code" },
  { id: "divider", label: "Divider", description: "Horizontal rule" },
  { id: "table", label: "Table", description: "3 × 3 table" },
] as const;

/**
 * The live editor's extension list: `editorExtensions` (the schema
 * `markdown.ts` also builds from — see its doc comment) with read-view
 * node views layered on for `image` and `codeBlock`. `.extend()` only
 * adds `addNodeView`, so the schema itself — what a document can contain —
 * stays identical between live editing and the standalone codec.
 */
const liveExtensions: Extensions = editorExtensions.map((extension) => {
  if (extension.name === "image") {
    return extension.extend({ addNodeView: () => ReactNodeViewRenderer(ImageView) });
  }
  if (extension.name === "codeBlock") {
    return extension.extend({ addNodeView: () => ReactNodeViewRenderer(CodeBlockView) });
  }
  return extension;
});

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
  /** Navigate when the user clicks a `wiki:…` link (`[[Target]]`). */
  onOpenWiki?: (target: string) => void;
  autosaveDelayMs?: number;
  spellcheck?: boolean;
  slashCommands?: boolean;
  showFrontmatter?: boolean;
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
  {
    raw,
    revision,
    io,
    onSaveStateChange,
    onRevisionChange,
    onOpenWiki,
    autosaveDelayMs = 800,
    spellcheck = true,
    slashCommands = true,
    showFrontmatter = true,
  },
  ref,
) {
  const [{ frontmatter }] = useState(() => splitFrontmatter(raw));
  const initialDoc = useMemo(() => parseMarkdownToJSON(splitFrontmatter(raw).body), [raw]);

  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [slashMenu, setSlashMenu] = useState<SlashMenuState | null>(null);
  const [slashIndex, setSlashIndex] = useState(0);
  const editorContainerRef = useRef<HTMLDivElement>(null);

  const updateSlashMenu = useCallback(
    (currentEditor: Editor) => {
      if (!slashCommands || !currentEditor.isEditable) {
        setSlashMenu(null);
        return;
      }
      const { $from } = currentEditor.state.selection;
      if (!$from.parent.isTextblock) {
        setSlashMenu(null);
        return;
      }
      const before = $from.parent.textBetween(0, $from.parentOffset, undefined, "\ufffc");
      const match = before.match(/(?:^|\s)\/([a-z0-9-]*)$/i);
      if (!match) {
        setSlashMenu(null);
        return;
      }
      const query = match[1] ?? "";
      const from = $from.pos - query.length - 1;
      const coordinates = currentEditor.view.coordsAtPos($from.pos);
      setSlashIndex(0);
      setSlashMenu({
        from,
        to: $from.pos,
        query,
        left: coordinates.left,
        top: coordinates.bottom + 6,
      });
    },
    [slashCommands],
  );

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
    extensions: liveExtensions,
    content: initialDoc,
    editorProps: {
      attributes: {
        spellcheck: spellcheck ? "true" : "false",
      },
    },
    onUpdate: ({ editor: currentEditor }) => {
      setSaveState((prev) => (prev.status === "conflict" ? prev : { status: "dirty" }));
      scheduleAutosave();
      updateSlashMenu(currentEditor);
    },
    onSelectionUpdate: ({ editor: currentEditor }) => updateSlashMenu(currentEditor),
  });

  useEffect(() => {
    editorContainerRef.current
      ?.querySelector<HTMLElement>("[contenteditable='true']")
      ?.setAttribute("spellcheck", spellcheck ? "true" : "false");
  }, [spellcheck]);

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
    }, autosaveDelayMs);
  }

  const filteredSlashCommands = useMemo(() => {
    const query = slashMenu?.query.toLowerCase() ?? "";
    return SLASH_COMMANDS.filter(
      (command) =>
        command.label.toLowerCase().includes(query) ||
        command.description.toLowerCase().includes(query),
    );
  }, [slashMenu?.query]);

  const runSlashCommand = useCallback(
    (id: (typeof SLASH_COMMANDS)[number]["id"]) => {
      if (!slashMenu) return;
      const chain = editor.chain().focus().deleteRange({ from: slashMenu.from, to: slashMenu.to });
      switch (id) {
        case "heading-1":
          chain.setHeading({ level: 1 }).run();
          break;
        case "heading-2":
          chain.setHeading({ level: 2 }).run();
          break;
        case "heading-3":
          chain.setHeading({ level: 3 }).run();
          break;
        case "bullet-list":
          chain.toggleBulletList().run();
          break;
        case "ordered-list":
          chain.toggleOrderedList().run();
          break;
        case "quote":
          chain.toggleBlockquote().run();
          break;
        case "code":
          chain.toggleCodeBlock().run();
          break;
        case "divider":
          chain.setHorizontalRule().run();
          break;
        case "table":
          chain.insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
          break;
        case "text":
          chain.setParagraph().run();
          break;
      }
      setSlashMenu(null);
    },
    [editor, slashMenu],
  );

  useEffect(() => {
    if (!slashMenu) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setSlashMenu(null);
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        setSlashIndex((index) => (index + 1) % Math.max(filteredSlashCommands.length, 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSlashIndex(
          (index) =>
            (index - 1 + Math.max(filteredSlashCommands.length, 1)) %
            Math.max(filteredSlashCommands.length, 1),
        );
      } else if (event.key === "Enter" && filteredSlashCommands.length > 0) {
        event.preventDefault();
        runSlashCommand(filteredSlashCommands[slashIndex]?.id ?? filteredSlashCommands[0].id);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [filteredSlashCommands, runSlashCommand, slashIndex, slashMenu]);

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

      {frontmatter && showFrontmatter && (
        <details className="frontmatter-block">
          <summary>Frontmatter</summary>
          <pre className="frontmatter-content">{frontmatter.trim()}</pre>
        </details>
      )}

      {saveState.status === "error" && <p className="error-text">{saveState.message}</p>}

      <div
        ref={editorContainerRef}
        onClick={(event) => {
          if (!onOpenWiki) return;
          const anchor = (event.target as HTMLElement | null)?.closest?.("a");
          if (!anchor) return;
          const href = anchor.getAttribute("href");
          if (!href?.startsWith("wiki:")) return;
          event.preventDefault();
          event.stopPropagation();
          onOpenWiki(decodeURIComponent(href.slice("wiki:".length)));
        }}
      >
        <EditorContent editor={editor} className="markdown-body page-editor-content" />
      </div>
      <p className="page-editor-hint">Type / for blocks and formatting · ⌘S saves immediately</p>
      {slashMenu && (
        <div
          className="slash-menu"
          role="listbox"
          aria-label="Block commands"
          style={{ left: slashMenu.left, top: slashMenu.top }}
        >
          {filteredSlashCommands.length === 0 ? (
            <p>No matching commands</p>
          ) : (
            filteredSlashCommands.map((command, index) => (
              <button
                type="button"
                role="option"
                aria-selected={slashIndex === index}
                key={command.id}
                className={slashIndex === index ? "slash-command-active" : ""}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => runSlashCommand(command.id)}
              >
                <strong>{command.label}</strong>
                <span>{command.description}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
});
