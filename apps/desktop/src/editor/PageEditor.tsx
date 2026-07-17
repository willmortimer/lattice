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
import { DOMParser as ProseMirrorDOMParser, Slice } from "@tiptap/pm/model";
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import {
  ArrowDown,
  ArrowUp,
  Bold,
  Code,
  CopyPlus,
  Heading1,
  Heading2,
  Italic,
  Link as LinkIcon,
  List,
  Pilcrow,
  Quote,
  Strikethrough,
  Unlink,
} from "lucide-react";

import { CodeBlockView } from "./CodeBlockView";
import { ConflictEnvelope } from "./ConflictEnvelope";
import { BlockDragHandle } from "./BlockDragHandle";
import { editorExtensions } from "./extensions";
import { ImageView } from "./ImageView";
import { LatticeEmbedView } from "./LatticeEmbedView";
import {
  joinFrontmatter,
  parseMarkdownToJSON,
  splitFrontmatter,
} from "./markdown";
import { classifyClipboard, sanitizePastedHtml } from "./pasteSanitize";
import {
  latticeEmbedMarkdown,
  pageDropIntent,
  readResourceDragPayload,
  wikiLinkMarkdown,
} from "../lib/resourceDrag";
import { PageModeChrome } from "./PageModeChrome";
import { PagePreview } from "./PagePreview";
import { PageSourceEditor } from "./PageSourceEditor";
import { applyModeSwitch, bodyForPersistence, type PageMode } from "./pageDraft";
import { StaleRevisionError, type PageIO } from "./pageIO";
import { type SaveState } from "./saveState";
import { KindMark } from "../KindMark";
import type { ResourceLinkTarget } from "../lib/resourceLinks";
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

interface FloatingToolbarState {
  left: number;
  top: number;
}

interface WikiMenuState extends SlashMenuState {}

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
  { id: "table", label: "Table", description: "Create a typed SQLite-backed table" },
  { id: "markdown-table", label: "Markdown table", description: "Static 3 × 3 page table" },
] as const;

/**
 * The live editor's extension list: `editorExtensions` (the schema
 * `markdown.ts` also builds from — see its doc comment) with read-view
 * node views layered on for `image` and `codeBlock`. `.extend()` only
 * adds `addNodeView`, so the schema itself — what a document can contain —
 * stays identical between live editing and the standalone codec.
 */
const liveExtensions: Extensions = [
  ...editorExtensions.map((extension) => {
    if (extension.name === "image") {
      return extension.extend({ addNodeView: () => ReactNodeViewRenderer(ImageView) });
    }
    if (extension.name === "codeBlock") {
      return extension.extend({ addNodeView: () => ReactNodeViewRenderer(CodeBlockView) });
    }
    if (extension.name === "latticeEmbed") {
      return extension.extend({ addNodeView: () => ReactNodeViewRenderer(LatticeEmbedView) });
    }
    return extension;
  }),
  BlockDragHandle,
];

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
  /** Create and open a canonical `.data` table package. */
  onCreateTable?: () => Promise<void> | void;
  /** Typed workspace resource targets offered after typing `[[`. */
  wikiTargets?: ResourceLinkTarget[];
  /** Native bounded catalog query used instead of hydrating all targets. */
  onSearchWiki?: (query: string) => Promise<ResourceLinkTarget[]>;
  /** Import a pasted/dropped file through the semantic command boundary. */
  onImportAsset?: (file: File) => Promise<string>;
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
    onCreateTable,
    wikiTargets = [],
    onSearchWiki,
    onImportAsset,
    autosaveDelayMs = 800,
    spellcheck = true,
    slashCommands = true,
    showFrontmatter = true,
  },
  ref,
) {
  const [{ frontmatter, body: initialBody }] = useState(() => splitFrontmatter(raw));
  const initialDoc = useMemo(() => parseMarkdownToJSON(initialBody), [initialBody]);

  const [mode, setMode] = useState<PageMode>("edit");
  const [draftBody, setDraftBody] = useState(initialBody);
  const [sourceParseError, setSourceParseError] = useState<string | null>(null);
  const [sourceResetKey, setSourceResetKey] = useState(0);

  const [saveState, setSaveState] = useState<SaveState>({ status: "idle" });
  const [searchedWikiTargets, setSearchedWikiTargets] = useState<ResourceLinkTarget[]>([]);
  const [slashMenu, setSlashMenu] = useState<SlashMenuState | null>(null);
  const [slashIndex, setSlashIndex] = useState(0);
  const [wikiMenu, setWikiMenu] = useState<WikiMenuState | null>(null);
  const [wikiIndex, setWikiIndex] = useState(0);
  const [selectionToolbar, setSelectionToolbar] = useState<FloatingToolbarState | null>(null);
  const [blockToolbar, setBlockToolbar] = useState<FloatingToolbarState | null>(null);
  const editorContainerRef = useRef<HTMLDivElement>(null);

  const onCreateTableRef = useRef(onCreateTable);
  onCreateTableRef.current = onCreateTable;
  const onImportAssetRef = useRef(onImportAsset);
  onImportAssetRef.current = onImportAsset;

  const updateEditorMenus = useCallback(
    (currentEditor: Editor) => {
      if (!currentEditor.isEditable || mode !== "edit") {
        setSlashMenu(null);
        setWikiMenu(null);
        setSelectionToolbar(null);
        setBlockToolbar(null);
        return;
      }

      const { from: selectionFrom, to: selectionTo, $from } = currentEditor.state.selection;
      if (selectionFrom !== selectionTo) {
        const start = currentEditor.view.coordsAtPos(selectionFrom);
        const end = currentEditor.view.coordsAtPos(selectionTo);
        setSelectionToolbar({
          left: (start.left + end.right) / 2,
          top: Math.min(start.top, end.top) - 8,
        });
        setBlockToolbar(null);
        setSlashMenu(null);
        setWikiMenu(null);
        return;
      }

      setSelectionToolbar(null);
      const blockCoordinates = currentEditor.view.coordsAtPos($from.start());
      setBlockToolbar({
        left: blockCoordinates.left - 8,
        top: blockCoordinates.top,
      });

      if (!$from.parent.isTextblock) {
        setSlashMenu(null);
        setWikiMenu(null);
        return;
      }
      const before = $from.parent.textBetween(0, $from.parentOffset, undefined, "\ufffc");

      const wikiMatch = before.match(/\[\[([^\]|\n]*)$/);
      if (wikiMatch) {
        const query = wikiMatch[1] ?? "";
        const from = $from.pos - query.length - 2;
        const coordinates = currentEditor.view.coordsAtPos($from.pos);
        setWikiIndex(0);
        setWikiMenu({
          from,
          to: $from.pos,
          query,
          left: coordinates.left,
          top: coordinates.bottom + 6,
        });
        setSlashMenu(null);
        return;
      }
      setWikiMenu(null);

      if (!slashCommands) {
        setSlashMenu(null);
        return;
      }
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
    [mode, slashCommands],
  );

  const importFilesIntoView = useCallback(
    async (view: EditorView, files: File[], position?: number) => {
      const importAsset = onImportAssetRef.current;
      if (!importAsset) return;

      if (position != null) {
        view.dispatch(view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(position))));
      }

      for (const file of files) {
        const src = await importAsset(file);
        if (view.isDestroyed) return;
        const { schema } = view.state;
        const transaction = view.state.tr;
        if (file.type.startsWith("image/") && schema.nodes.image) {
          transaction.replaceSelectionWith(
            schema.nodes.image.create({ src, alt: file.name, title: null }),
          );
        } else if (schema.marks.link) {
          transaction.replaceSelectionWith(
            schema.text(file.name, [schema.marks.link.create({ href: src })]),
          );
        }
        view.dispatch(transaction.scrollIntoView());
      }
    },
    [],
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

  const modeRef = useRef(mode);
  modeRef.current = mode;
  const draftBodyRef = useRef(draftBody);
  draftBodyRef.current = draftBody;

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
      handlePaste: (view, event) => {
        const kind = classifyClipboard(event.clipboardData);
        if (kind === "files" && onImportAssetRef.current) {
          const files = Array.from(event.clipboardData?.files ?? []);
          event.preventDefault();
          void importFilesIntoView(view, files);
          return true;
        }

        if (kind === "markdown" && event.clipboardData) {
          const markdown = event.clipboardData.getData("text/markdown");
          try {
            const docJson = parseMarkdownToJSON(markdown);
            const node = view.state.schema.nodeFromJSON(docJson);
            event.preventDefault();
            const tr = view.state.tr.replaceSelection(new Slice(node.content, 0, 0));
            view.dispatch(tr.scrollIntoView());
            return true;
          } catch {
            // Fall through to HTML/plain handling when markdown is malformed.
          }
        }

        if (kind === "html" && event.clipboardData) {
          const clean = sanitizePastedHtml(event.clipboardData.getData("text/html"));
          if (clean.trim()) {
            event.preventDefault();
            const element = document.createElement("div");
            element.innerHTML = clean;
            const slice = ProseMirrorDOMParser.fromSchema(view.state.schema).parseSlice(element, {
              preserveWhitespace: true,
            });
            view.dispatch(view.state.tr.replaceSelection(slice).scrollIntoView());
            return true;
          }
        }

        const pastedText = event.clipboardData?.getData("text/plain").trim() ?? "";
        const { from, to } = view.state.selection;
        if (
          from !== to &&
          /^(https?:\/\/|mailto:)/i.test(pastedText) &&
          view.state.schema.marks.link
        ) {
          event.preventDefault();
          view.dispatch(
            view.state.tr.addMark(
              from,
              to,
              view.state.schema.marks.link.create({ href: pastedText }),
            ),
          );
          return true;
        }
        return false;
      },
      handleDrop: (view, event) => {
        const resourcePayload = readResourceDragPayload(event.dataTransfer);
        if (resourcePayload) {
          event.preventDefault();
          const markdown =
            pageDropIntent(event) === "embed"
              ? latticeEmbedMarkdown(resourcePayload)
              : wikiLinkMarkdown(resourcePayload);
          try {
            const docJson = parseMarkdownToJSON(markdown);
            const node = view.state.schema.nodeFromJSON(docJson);
            const position = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos;
            let tr = view.state.tr;
            if (typeof position === "number") {
              tr = tr.setSelection(TextSelection.near(tr.doc.resolve(position)));
            }
            tr = tr.replaceSelection(new Slice(node.content, 0, 0));
            view.dispatch(tr.scrollIntoView());
          } catch (error) {
            console.warn("Failed to drop resource into page:", error);
          }
          return true;
        }

        const files = Array.from(event.dataTransfer?.files ?? []);
        if (files.length === 0 || !onImportAssetRef.current) return false;
        event.preventDefault();
        const position = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos;
        void importFilesIntoView(view, files, position);
        return true;
      },
    },
    onUpdate: ({ editor: currentEditor }) => {
      setSaveState((prev) => (prev.status === "conflict" ? prev : { status: "dirty" }));
      scheduleAutosave();
      updateEditorMenus(currentEditor);
    },
    onSelectionUpdate: ({ editor: currentEditor }) => updateEditorMenus(currentEditor),
  });

  useEffect(() => {
    editorContainerRef.current
      ?.querySelector<HTMLElement>("[contenteditable='true']")
      ?.setAttribute("spellcheck", spellcheck ? "true" : "false");
  }, [spellcheck]);

  useImperativeHandle(
    ref,
    () => ({
      getRaw: () =>
        joinFrontmatter(
          frontmatter,
          bodyForPersistence(modeRef.current, draftBodyRef.current, editor?.getJSON() ?? null),
        ),
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

    const body = bodyForPersistence(
      modeRef.current,
      draftBodyRef.current,
      editor?.getJSON() ?? null,
    );
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

  const requestModeChange = useCallback(
    (targetMode: PageMode) => {
      const result = applyModeSwitch({
        from: modeRef.current,
        to: targetMode,
        draftBody: draftBodyRef.current,
        editJson: modeRef.current === "edit" ? editor?.getJSON() ?? null : null,
      });
      if (result.blocked) {
        setSourceParseError(result.sourceParseError);
        setMode("source");
        setSourceResetKey((key) => key + 1);
        return;
      }
      setSourceParseError(result.sourceParseError);
      setDraftBody(result.draftBody);
      draftBodyRef.current = result.draftBody;
      if (result.editContent && editor) {
        editor.commands.setContent(result.editContent);
      }
      if (result.mode === "source") {
        setSourceResetKey((key) => key + 1);
      }
      setMode(result.mode);
      modeRef.current = result.mode;
    },
    [editor],
  );

  const handleSourceChange = useCallback((nextBody: string) => {
    draftBodyRef.current = nextBody;
    setDraftBody(nextBody);
    setSourceParseError(null);
    setSaveState((prev) => (prev.status === "conflict" ? prev : { status: "dirty" }));
    scheduleAutosave();
  }, []);

  const filteredSlashCommands = useMemo(() => {
    const query = slashMenu?.query.toLowerCase() ?? "";
    return SLASH_COMMANDS.filter(
      (command) =>
        command.label.toLowerCase().includes(query) ||
        command.description.toLowerCase().includes(query),
    );
  }, [slashMenu?.query]);

  useEffect(() => {
    if (!wikiMenu || !onSearchWiki) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void onSearchWiki(wikiMenu.query).then((targets) => {
        if (!cancelled) setSearchedWikiTargets(targets);
      });
    }, 80);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [onSearchWiki, wikiMenu]);

  const filteredWikiTargets = useMemo(() => {
    const query = wikiMenu?.query.trim().toLowerCase() ?? "";
    return (onSearchWiki ? searchedWikiTargets : wikiTargets)
      .filter(
        (target) =>
          !query ||
          target.display.toLowerCase().includes(query) ||
          target.canonical.toLowerCase().includes(query),
      )
      .slice(0, 12);
  }, [onSearchWiki, searchedWikiTargets, wikiMenu?.query, wikiTargets]);

  const runSlashCommand = useCallback(
    async (id: (typeof SLASH_COMMANDS)[number]["id"]) => {
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
          chain.run();
          setSlashMenu(null);
          await performSaveRef.current();
          await onCreateTableRef.current?.();
          return;
        case "markdown-table":
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

  const insertWikiTarget = useCallback(
    (target: ResourceLinkTarget) => {
      if (!wikiMenu) return;
      editor
        .chain()
        .focus()
        .deleteRange({ from: wikiMenu.from, to: wikiMenu.to })
        .insertContent({
          type: "text",
          text: target.canonical,
          marks: [
            { type: "link", attrs: { href: `wiki:${encodeURIComponent(target.canonical)}` } },
          ],
        })
        .run();
      setWikiMenu(null);
    },
    [editor, wikiMenu],
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
        void runSlashCommand(
          filteredSlashCommands[slashIndex]?.id ?? filteredSlashCommands[0].id,
        );
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [filteredSlashCommands, runSlashCommand, slashIndex, slashMenu]);

  useEffect(() => {
    if (!wikiMenu) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setWikiMenu(null);
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        setWikiIndex((index) => (index + 1) % Math.max(filteredWikiTargets.length, 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setWikiIndex(
          (index) =>
            (index - 1 + Math.max(filteredWikiTargets.length, 1)) %
            Math.max(filteredWikiTargets.length, 1),
        );
      } else if (event.key === "Enter" && filteredWikiTargets.length > 0) {
        event.preventDefault();
        insertWikiTarget(filteredWikiTargets[wikiIndex] ?? filteredWikiTargets[0]);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [filteredWikiTargets, insertWikiTarget, wikiIndex, wikiMenu]);

  const setSelectionLink = useCallback(() => {
    if (editor.isActive("link")) {
      editor.chain().focus().unsetLink().run();
      return;
    }
    const input = window.prompt("Link URL or [[page]]")?.trim();
    if (!input) return;
    const wiki = input.match(/^\[\[([^\]]+)\]\]$/);
    const href = wiki ? `wiki:${encodeURIComponent(wiki[1].trim())}` : input;
    editor.chain().focus().setLink({ href }).run();
  }, [editor]);

  const transformBlock = useCallback(
    (kind: "paragraph" | "heading-1" | "heading-2" | "bullet-list" | "quote") => {
      const chain = editor.chain().focus();
      switch (kind) {
        case "paragraph":
          chain.setParagraph().run();
          break;
        case "heading-1":
          chain.setHeading({ level: 1 }).run();
          break;
        case "heading-2":
          chain.setHeading({ level: 2 }).run();
          break;
        case "bullet-list":
          chain.toggleBulletList().run();
          break;
        case "quote":
          chain.toggleBlockquote().run();
          break;
      }
    },
    [editor],
  );

  const moveOrDuplicateBlock = useCallback(
    (action: "up" | "down" | "duplicate") => {
      const { state, view } = editor;
      const index = state.selection.$from.index(0);
      const node = state.doc.child(index);
      let start = 0;
      for (let childIndex = 0; childIndex < index; childIndex += 1) {
        start += state.doc.child(childIndex).nodeSize;
      }
      const end = start + node.nodeSize;
      const transaction = state.tr;

      if (action === "duplicate") {
        const insertAt = end;
        transaction.insert(insertAt, node.copy(node.content));
        transaction.setSelection(TextSelection.near(transaction.doc.resolve(insertAt + 1)));
      } else if (action === "up" && index > 0) {
        const previous = state.doc.child(index - 1);
        const insertAt = start - previous.nodeSize;
        transaction.delete(start, end).insert(insertAt, node);
        transaction.setSelection(TextSelection.near(transaction.doc.resolve(insertAt + 1)));
      } else if (action === "down" && index < state.doc.childCount - 1) {
        const next = state.doc.child(index + 1);
        transaction.delete(start, end).insert(start + next.nodeSize, node);
        transaction.setSelection(
          TextSelection.near(transaction.doc.resolve(start + next.nodeSize + 1)),
        );
      } else {
        return;
      }

      view.dispatch(transaction.scrollIntoView());
      view.focus();
    },
    [editor],
  );

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
    const reloaded = splitFrontmatter(snapshot.raw);
    const body = reloaded.body;
    setDraftBody(body);
    draftBodyRef.current = body;
    setSourceParseError(null);
    setMode("edit");
    modeRef.current = "edit";
    editor.commands.setContent(parseMarkdownToJSON(body));
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

      <PageModeChrome
        mode={mode}
        sourceParseError={sourceParseError}
        onModeChange={requestModeChange}
      />

      {mode === "source" && (
        <PageSourceEditor
          value={draftBody}
          resetKey={`${sourceResetKey}`}
          onChange={handleSourceChange}
        />
      )}

      {mode === "preview" && (
        <PagePreview draftBody={draftBody} parseError={sourceParseError} />
      )}

      {mode === "edit" && (
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
      )}
      {mode === "edit" && (
      <p className="page-editor-hint">
        Type / for blocks · [[ to link a page · paste or drop files · ⌘S saves immediately
      </p>
      )}
      {mode === "edit" && selectionToolbar && (
        <div
          className="editor-floating-toolbar editor-selection-toolbar"
          role="toolbar"
          aria-label="Text formatting"
          style={{ left: selectionToolbar.left, top: selectionToolbar.top }}
        >
          <button type="button" aria-label="Bold" title="Bold" onClick={() => editor.chain().focus().toggleBold().run()}>
            <Bold size={14} />
          </button>
          <button type="button" aria-label="Italic" title="Italic" onClick={() => editor.chain().focus().toggleItalic().run()}>
            <Italic size={14} />
          </button>
          <button type="button" aria-label="Strikethrough" title="Strikethrough" onClick={() => editor.chain().focus().toggleStrike().run()}>
            <Strikethrough size={14} />
          </button>
          <button type="button" aria-label="Inline code" title="Inline code" onClick={() => editor.chain().focus().toggleCode().run()}>
            <Code size={14} />
          </button>
          <button type="button" aria-label={editor.isActive("link") ? "Remove link" : "Add link"} title={editor.isActive("link") ? "Remove link" : "Add link"} onClick={setSelectionLink}>
            {editor.isActive("link") ? <Unlink size={14} /> : <LinkIcon size={14} />}
          </button>
        </div>
      )}
      {mode === "edit" && blockToolbar && !slashMenu && !wikiMenu && (
        <div
          className="editor-floating-toolbar editor-block-toolbar"
          role="toolbar"
          aria-label="Block actions"
          style={{ left: blockToolbar.left, top: blockToolbar.top }}
        >
          <button type="button" aria-label="Paragraph" title="Paragraph" onClick={() => transformBlock("paragraph")}><Pilcrow size={13} /></button>
          <button type="button" aria-label="Heading 1" title="Heading 1" onClick={() => transformBlock("heading-1")}><Heading1 size={13} /></button>
          <button type="button" aria-label="Heading 2" title="Heading 2" onClick={() => transformBlock("heading-2")}><Heading2 size={13} /></button>
          <button type="button" aria-label="Bulleted list" title="Bulleted list" onClick={() => transformBlock("bullet-list")}><List size={13} /></button>
          <button type="button" aria-label="Quote" title="Quote" onClick={() => transformBlock("quote")}><Quote size={13} /></button>
          <span className="editor-toolbar-separator" />
          <button type="button" aria-label="Move block up" title="Move block up" onClick={() => moveOrDuplicateBlock("up")}><ArrowUp size={13} /></button>
          <button type="button" aria-label="Move block down" title="Move block down" onClick={() => moveOrDuplicateBlock("down")}><ArrowDown size={13} /></button>
          <button type="button" aria-label="Duplicate block" title="Duplicate block" onClick={() => moveOrDuplicateBlock("duplicate")}><CopyPlus size={13} /></button>
        </div>
      )}
      {mode === "edit" && slashMenu && (
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
                onClick={() => void runSlashCommand(command.id)}
              >
                <strong>{command.label}</strong>
                <span>{command.description}</span>
              </button>
            ))
          )}
        </div>
      )}
      {mode === "edit" && wikiMenu && (
        <div
          className="slash-menu wiki-menu"
          role="listbox"
          aria-label="Link to page"
          style={{ left: wikiMenu.left, top: wikiMenu.top }}
        >
          {filteredWikiTargets.length === 0 ? (
            <p>No matching pages</p>
          ) : (
            filteredWikiTargets.map((target, index) => (
              <button
                type="button"
                role="option"
                aria-selected={wikiIndex === index}
                key={target.path}
                className={wikiIndex === index ? "slash-command-active" : ""}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => insertWikiTarget(target)}
              >
                <strong>
                  <KindMark kind={target.kind} size={13} />
                  {target.display}
                </strong>
                <span>{target.canonical}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
});
