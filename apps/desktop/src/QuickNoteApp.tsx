import { Button, IconButton, TooltipProvider } from "@lattice/ui";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, ExternalLink, FileText, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { createNativePageIO } from "./editor/pageIO";
import { createPage, resolveQuickNoteTemplatePath } from "./lib/pages";
import { loadProfile } from "./lib/profile";
import { quickNotePath } from "./lib/timestamp";
import {
  applyResolvedTheme,
  detectSystemAppearance,
  type ThemeCatalogPayload,
} from "./theme/apply";
import type { Resource } from "./types";

interface QuickNotePage {
  root: string;
  workspaceTitle: string;
  path: string;
}

interface OpenPayload {
  root: string | null;
}

export function QuickNoteApp() {
  const [page, setPage] = useState<QuickNotePage | null>(null);
  const [draft, setDraft] = useState("");
  const [saveState, setSaveState] = useState<"idle" | "dirty" | "saving" | "saved" | "error">(
    "idle",
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const creatingRef = useRef(false);
  const revisionRef = useRef<string | null>(null);
  const autosaveDelayRef = useRef(800);
  const saveTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);

  const prepare = useCallback(async (requestedRoot?: string | null) => {
    if (creatingRef.current) return;
    if (page) return;
    const profile = await loadProfile();
    autosaveDelayRef.current = profile.settings.desktop.editor.autosaveDelayMs;
    const root =
      requestedRoot ??
      profile.recents[0]?.root ??
      profile.effectiveDefaultWorkspace ??
      null;
    if (!root) {
      setError("Open a workspace in Lattice before using Quick Note.");
      return;
    }

    creatingRef.current = true;
    setLoading(true);
    setError(null);
    try {
      const workspace = await invoke<{
        title: string;
        defaults: { quickNoteDirectory: string; templateDirectory?: string | null };
        resources: Resource[];
      }>("open_workspace", { path: root });
      const path = quickNotePath(new Date(), workspace.defaults.quickNoteDirectory);
      // Quick Note default: `<templateDirectory>/Daily.md` when configured,
      // else `Templates/Daily.md` when that resource exists.
      const templatePath = resolveQuickNoteTemplatePath(
        workspace.defaults.templateDirectory,
        workspace.resources.map((resource) => resource.path),
      );
      const title = new Date().toISOString().slice(0, 10);
      await createPage({
        root,
        relPath: path,
        content: "",
        templatePath,
        title,
      });
      const io = createNativePageIO(root, path);
      const loaded = await io.load();
      const catalog = await invoke<ThemeCatalogPayload>("list_themes", {
        system: detectSystemAppearance(),
        workspaceRoot: root,
      });
      applyResolvedTheme(catalog.resolved);
      setPage({
        root,
        workspaceTitle: workspace.title,
        path,
      });
      setDraft(loaded.raw);
      revisionRef.current = loaded.revision;
      setSaveState("idle");
    } catch (err) {
      setError(String(err));
    } finally {
      creatingRef.current = false;
      setLoading(false);
    }
  }, [page]);

  const saveDraft = useCallback(
    async (raw: string) => {
      if (!page) return;
      setSaveState("saving");
      try {
        const revision = await createNativePageIO(page.root, page.path).save(
          raw,
          revisionRef.current,
        );
        revisionRef.current = revision;
        setSaveState("saved");
        window.setTimeout(() => setSaveState((state) => (state === "saved" ? "idle" : state)), 1200);
      } catch (err) {
        setSaveState("error");
        setError(String(err));
      }
    },
    [page],
  );

  function updateDraft(value: string) {
    setDraft(value);
    setSaveState("dirty");
    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    saveTimerRef.current = window.setTimeout(
      () => void saveDraft(value),
      autosaveDelayRef.current,
    );
  }

  useEffect(
    () => () => {
      if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    },
    [],
  );

  async function flushDraft() {
    if (!page || (saveState !== "dirty" && saveState !== "error")) return;
    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    await saveDraft(draft);
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<OpenPayload>("quick-note-open", (event) => {
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
        void getCurrentWindow().hide();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  async function openInMain() {
    if (!page) return;
    await flushDraft();
    await emitTo("main", "open-resource", { root: page.root, path: page.path });
    await getCurrentWindow().hide();
  }

  async function openExternally() {
    if (!page) return;
    await flushDraft();
    await openPath(`${page.root}/${page.path}`);
  }

  async function openInCode() {
    if (!page) return;
    await flushDraft();
    const absolute = `${page.root}/${page.path}`;
    await openUrl(`vscode://file/${encodeURI(absolute)}`);
  }

  async function closeWindow() {
    await flushDraft();
    await getCurrentWindow().hide();
    setPage(null);
    setDraft("");
    setSaveState("idle");
    setError(null);
  }

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
            {page && (
              <>
                <Button variant="ghost" size="sm" onClick={() => void openInCode()}>
                  <ExternalLink size={13} />
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
            <textarea
              className="quick-note-editor"
              value={draft}
              autoFocus
              spellCheck
              aria-label="Quick Note Markdown"
              placeholder="Capture a thought… Markdown is saved directly into Inbox."
              onChange={(event) => updateDraft(event.target.value)}
              onKeyDown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
                  event.preventDefault();
                  if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
                  void saveDraft(draft);
                }
              }}
            />
          )}
        </main>
      </div>
    </TooltipProvider>
  );
}
