import { Button, IconButton, TooltipProvider } from "@lattice/ui";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut, ArrowUpRight, FileText, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { createNativePageIO, StaleRevisionError } from "./editor/pageIO";
import { createSerializedSaveController } from "./editor/serializedSave";
import { prepareQuickNote } from "./lib/pages";
import { mergeDictationPlainText } from "./lib/mergeDictationPlainText";
import { loadProfile } from "./lib/profile";
import { deleteResource } from "./lib/resourceMutations";
import { voiceHintsFromPage } from "./lib/voice";
import {
  QuickNoteDictation,
  type QuickNoteDictationHandle,
} from "./shell/QuickNoteDictation";
import {
  applyResolvedTheme,
  detectSystemAppearance,
  type ThemeCatalogPayload,
} from "./theme/apply";

interface QuickNotePage {
  root: string;
  workspaceTitle: string;
  path: string;
}

interface OpenPayload {
  root: string | null;
}

type SaveState = "idle" | "dirty" | "saving" | "saved" | "error";

export function QuickNoteApp() {
  const [page, setPage] = useState<QuickNotePage | null>(null);
  const [draft, setDraft] = useState("");
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [provisionalText, setProvisionalText] = useState<string | null>(null);
  const [dictationAnchor, setDictationAnchor] = useState(0);
  const creatingRef = useRef(false);
  const autosaveDelayRef = useRef(800);
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const userEditedRef = useRef(false);
  const initialDraftRef = useRef("");
  const dictationRef = useRef<QuickNoteDictationHandle | null>(null);
  const pendingRootRef = useRef<string | null | undefined>(undefined);
  const saveControllerRef = useRef<ReturnType<
    typeof createSerializedSaveController<string | null>
  > | null>(null);

  const disposeSaveController = useCallback(() => {
    saveControllerRef.current?.dispose();
    saveControllerRef.current = null;
  }, []);

  const prepare = useCallback(async (requestedRoot?: string | null): Promise<boolean> => {
    if (creatingRef.current) return false;
    if (page) return true;
    const profile = await loadProfile();
    autosaveDelayRef.current = profile.settings.desktop.editor.autosaveDelayMs;
    document.documentElement.dataset.pageWidth = profile.settings.desktop.editor.pageWidth;
    const root =
      requestedRoot ??
      profile.recents[0]?.root ??
      profile.effectiveDefaultWorkspace ??
      null;
    if (!root) {
      setError("Open a workspace in Lattice before using Quick Note.");
      return false;
    }

    creatingRef.current = true;
    setLoading(true);
    setError(null);
    try {
      const prepared = await prepareQuickNote(root);
      const catalog = await invoke<ThemeCatalogPayload>("list_themes", {
        system: detectSystemAppearance(),
        workspaceRoot: root,
      });
      applyResolvedTheme(catalog.resolved);

      disposeSaveController();
      const io = createNativePageIO(prepared.root, prepared.path);
      saveControllerRef.current = createSerializedSaveController<string | null>({
        initialRevision: prepared.revision,
        save: async (baseRevision) => io.save(draftRef.current, baseRevision),
        onStatus: (status) => {
          switch (status) {
            case "idle":
              setSaveState("idle");
              break;
            case "dirty":
              setSaveState("dirty");
              break;
            case "saving":
              setSaveState("saving");
              break;
            case "saved":
              setSaveState("saved");
              break;
            case "conflict":
            case "error":
              setSaveState("error");
              break;
            default:
              status satisfies never;
          }
        },
        isConflict: (err) => err instanceof StaleRevisionError,
        savedIndicatorMs: 1200,
      });

      setPage({
        root: prepared.root,
        workspaceTitle: prepared.workspaceTitle,
        path: prepared.path,
      });
      setDraft(prepared.content);
      initialDraftRef.current = prepared.content;
      userEditedRef.current = false;
      setSaveState("idle");
      return true;
    } catch (err) {
      setError(String(err));
      return false;
    } finally {
      creatingRef.current = false;
      setLoading(false);
    }
  }, [disposeSaveController, page]);

  function updateDraft(value: string) {
    userEditedRef.current = true;
    draftRef.current = value;
    setDraft(value);
    saveControllerRef.current?.markDirty(autosaveDelayRef.current);
  }

  useEffect(
    () => () => {
      disposeSaveController();
    },
    [disposeSaveController],
  );

  async function flushDraft() {
    await saveControllerRef.current?.flush();
  }

  const resetNoteState = useCallback(() => {
    disposeSaveController();
    setPage(null);
    setDraft("");
    initialDraftRef.current = "";
    userEditedRef.current = false;
    setProvisionalText(null);
    setDictationAnchor(0);
    setSaveState("idle");
    setError(null);
  }, [disposeSaveController]);

  const discardEmptyDictationNote = useCallback(async () => {
    if (!page) {
      resetNoteState();
      return;
    }
    const { root, path } = page;
    resetNoteState();
    try {
      await deleteResource(root, path);
    } catch (err) {
      setError(String(err));
    }
  }, [page, resetNoteState]);

  const commitDictationFinal = useCallback(
    async (text: string, anchor: number) => {
      if (!page) return;
      saveControllerRef.current?.clearTimer();

      const trimmedFinal = text.trim();
      if (!trimmedFinal && !userEditedRef.current && draft.trim() === initialDraftRef.current.trim()) {
        // Silence-only capture on a fresh note: remove the empty inbox page.
        await discardEmptyDictationNote();
        return;
      }

      const merged = mergeDictationPlainText(draft, text, anchor);
      draftRef.current = merged;
      setDraft(merged);
      setProvisionalText(null);
      saveControllerRef.current?.markDirty(0);
      await flushDraft();
    },
    [discardEmptyDictationNote, draft, page],
  );

  const ensurePageReady = useCallback(async () => {
    if (page) return true;
    const root = pendingRootRef.current;
    return prepare(root);
  }, [page, prepare]);

  const voiceContext = useMemo(() => {
    if (!page) return null;
    return voiceHintsFromPage({
      documentPath: page.path,
      pageTitle: page.path.split("/").pop()?.replace(/\.md$/i, "") ?? "Quick Note",
      workspaceName: page.workspaceTitle,
      rawContent: draft,
    });
  }, [draft, page]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<OpenPayload>("quick-note-open", (event) => {
      pendingRootRef.current = event.payload.root;
      void prepare(event.payload.root);
    }).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
  }, [prepare]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        dictationRef.current?.cancel();
        void getCurrentWindow().hide();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  async function openInMain() {
    if (!page) return;
    dictationRef.current?.cancel();
    await flushDraft();
    await emitTo("main", "open-resource", { root: page.root, path: page.path });
    await getCurrentWindow().hide();
  }

  async function openExternally() {
    if (!page) return;
    dictationRef.current?.cancel();
    await flushDraft();
    await openPath(`${page.root}/${page.path}`);
  }

  async function openInCode() {
    if (!page) return;
    dictationRef.current?.cancel();
    await flushDraft();
    const absolute = `${page.root}/${page.path}`;
    await openUrl(`vscode://file/${encodeURI(absolute)}`);
  }

  async function closeWindow() {
    dictationRef.current?.cancel();
    await flushDraft();
    await getCurrentWindow().hide();
    resetNoteState();
  }

  const mirrorBefore = draft.slice(0, dictationAnchor);
  const mirrorAfter = draft.slice(dictationAnchor);

  return (
    <TooltipProvider>
      <div className="quick-note-shell">
        <div className="quick-note-native-titlebar" data-tauri-drag-region />
        <header className="quick-note-head" data-tauri-drag-region>
          <div className="quick-note-heading">
            <FileText size={15} aria-hidden="true" />
            <span>Quick Note</span>
            {page && <span className="quick-note-workspace">{page.workspaceTitle}</span>}
          </div>
          <div className="quick-note-actions">
            <QuickNoteDictation
              ref={dictationRef}
              enabled
              ensurePageReady={ensurePageReady}
              getInsertPosition={() =>
                textareaRef.current?.selectionStart ?? draft.length
              }
              voiceContext={voiceContext}
              onProvisionalChange={(text, anchor) => {
                setProvisionalText(text);
                setDictationAnchor(anchor);
              }}
              onFinal={(text, anchor) => {
                void commitDictationFinal(text, anchor);
              }}
              onError={(message) => setError(message)}
            />
            {page && (
              <>
                <Button variant="ghost" size="sm" onClick={() => void openInCode()}>
                  <ArrowSquareOut size={13} />
                  VS Code
                </Button>
                <Button variant="ghost" size="sm" onClick={() => void openExternally()}>
                  <ArrowUpRight size={13} />
                  External
                </Button>
                <Button variant="secondary" size="sm" onClick={() => void openInMain()}>
                  Open in Lattice
                </Button>
              </>
            )}
            <IconButton label="Close Quick Note" onClick={() => void closeWindow()}>
              <X size={15} />
            </IconButton>
          </div>
        </header>

        <div className="quick-note-meta">
          <span>{page?.path ?? "Inbox/"}</span>
          <span className={`save-state save-state-${saveState}`}>
            {saveState === "dirty"
              ? "Edited"
              : saveState === "saving"
                ? "Saving…"
                : saveState === "saved"
                  ? "Saved"
                  : saveState === "error"
                    ? "Save failed"
                    : page
                      ? "Autosaves"
                      : ""}
          </span>
        </div>

        <main className="quick-note-body">
          {loading && <div className="quick-note-empty">Creating a note in your Inbox…</div>}
          {!loading && error && (
            <div className="quick-note-empty">
              <p>{error}</p>
              <Button variant="secondary" onClick={() => void prepare(null)}>
                Try recent workspace
              </Button>
            </div>
          )}
          {!loading && !error && !page && (
            <div className="quick-note-empty">
              <p>Quick Note is ready.</p>
              <Button variant="primary" onClick={() => void prepare(null)}>
                Start a note
              </Button>
            </div>
          )}
          {page && (
            <div className="quick-note-editor-stack">
              <div className="quick-note-editor-mirror" aria-hidden="true">
                <span>{mirrorBefore}</span>
                {provisionalText && (
                  <span className="quick-note-dictation-provisional">{provisionalText}</span>
                )}
                <span>{mirrorAfter}</span>
              </div>
              <textarea
                ref={textareaRef}
                className="quick-note-editor"
                value={draft}
                autoFocus
                spellCheck
                aria-label="Quick Note Markdown"
                placeholder="Capture a thought… Markdown is saved directly into Inbox."
                onChange={(event) => updateDraft(event.target.value)}
                onSelect={(event) => {
                  if (provisionalText) return;
                  setDictationAnchor(event.currentTarget.selectionStart);
                }}
                onKeyDown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
                    event.preventDefault();
                    saveControllerRef.current?.clearTimer();
                    void flushDraft();
                  }
                }}
              />
            </div>
          )}
        </main>
      </div>
    </TooltipProvider>
  );
}
