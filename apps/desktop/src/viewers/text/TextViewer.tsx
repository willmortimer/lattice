import { useEffect, useMemo, useRef, useState } from "react";
import { applyResourceUpdate } from "../../lib/resourceRuntime";
import { loadTextResource } from "../../controllers/resourceLoad";
import type { OpenResourceSession } from "../../resourceSession";
import type { Resource } from "../../types";
import type { SaveState } from "../../editor/saveState";
import { StructuredTree } from "./StructuredTree";
import { parseStructuredInWorker, type StructuredParseResult } from "./structuredParser";
import { TextCodeMirror, syntaxForPath } from "./TextCodeMirror";
import "./textViewer.css";

const UTF8_BOM = new Uint8Array([0xef, 0xbb, 0xbf]);

type TextSession = Extract<OpenResourceSession, { kind: "text" }>;

export interface TextViewerProps {
  session: TextSession;
  root: string | null;
  onSaveStateChange?: (state: SaveState) => void;
  onRevisionChange?: (revision: string | null) => void;
  onOpenExternally?: (resource: Resource) => void;
}

function encodeText(value: string, encoding: TextSession["encoding"]): Uint8Array {
  if (encoding === "utf8" || encoding === "utf8-bom") {
    const encoded = new TextEncoder().encode(value);
    if (encoding === "utf8") return encoded;
    const output = new Uint8Array(UTF8_BOM.length + encoded.length);
    output.set(UTF8_BOM);
    output.set(encoded, UTF8_BOM.length);
    return output;
  }
  const bytes = new Uint8Array(value.length * 2 + (encoding === "utf16-le" ? 0 : 2));
  let offset = 0;
  if (encoding === "utf16-be") {
    bytes[0] = 0xfe;
    bytes[1] = 0xff;
    offset = 2;
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (encoding === "utf16-le") {
      bytes[index * 2] = code & 0xff;
      bytes[index * 2 + 1] = code >> 8;
    } else {
      bytes[offset + index * 2] = code >> 8;
      bytes[offset + index * 2 + 1] = code & 0xff;
    }
  }
  return bytes;
}

function report(state: ((state: SaveState) => void) | undefined, next: SaveState) {
  state?.(next);
}

export function TextViewer({ session, root, onSaveStateChange, onRevisionChange, onOpenExternally }: TextViewerProps) {
  const [activeSession, setActiveSession] = useState(session);
  const [content, setContent] = useState(session.content);
  const [revision, setRevision] = useState(session.revision);
  const [dirty, setDirty] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [parseResult, setParseResult] = useState<StructuredParseResult | null>(null);
  const [showTree, setShowTree] = useState(true);
  const saveController = useRef<AbortController | null>(null);
  const syntaxInfo = useMemo(() => syntaxForPath(session.resource.path, session.resource.formatId), [session.resource.formatId, session.resource.path]);
  const structuredSyntax = syntaxInfo.syntax === "json" || syntaxInfo.syntax === "yaml" ? syntaxInfo.syntax : null;
  const isStructured = structuredSyntax !== null && !activeSession.truncated;

  useEffect(() => {
    setActiveSession(session);
    setContent(session.content);
    setRevision(session.revision);
    setDirty(false);
    setSaveError(null);
  }, [session]);

  useEffect(() => {
    if (!isStructured) {
      setParseResult(null);
      return;
    }
    const controller = new AbortController();
    setParseResult(null);
    void parseStructuredInWorker(content, structuredSyntax, controller.signal).then(setParseResult).catch(() => {
      if (!controller.signal.aborted) setParseResult(null);
    });
    return () => controller.abort();
  }, [content, isStructured, structuredSyntax]);

  useEffect(() => () => saveController.current?.abort(), []);

  const updateContent = (next: string) => {
    setContent(next);
    setDirty(true);
    setSaveError(null);
    report(onSaveStateChange, { status: "dirty" });
  };

  const save = async () => {
    if (!activeSession.editable || !dirty || !root) return;
    saveController.current?.abort();
    const controller = new AbortController();
    saveController.current = controller;
    report(onSaveStateChange, { status: "saving" });
    setSaveError(null);
    try {
      const nextRevision = await applyResourceUpdate({
        root,
        path: activeSession.resource.path,
        content: encodeText(content, activeSession.encoding),
        baseRevision: revision,
      }, controller.signal);
      setRevision(nextRevision);
      setDirty(false);
      onRevisionChange?.(nextRevision);
      report(onSaveStateChange, { status: "saved" });
    } catch (error) {
      if (controller.signal.aborted) return;
      const message = String(error);
      setSaveError(message);
      report(onSaveStateChange, { status: message.includes("STALE") ? "conflict" : "error", message });
    }
  };

  const moveWindow = async (direction: -1 | 1) => {
    if (!root || activeSession.editable) return;
    const controller = new AbortController();
    const nextOffset = Math.max(0, Math.min(activeSession.totalSize - 1, activeSession.offset + direction * activeSession.content.length));
    try {
      const loaded = await loadTextResource(root, activeSession.resource.path, controller.signal, { offset: nextOffset });
      setActiveSession({ ...activeSession, inspection: loaded.inspection, content: loaded.window.content, revision: loaded.inspection.revision, offset: loaded.window.offset, totalSize: loaded.window.totalSize, truncated: loaded.window.truncated, encoding: loaded.window.encoding, editable: loaded.editable });
      setContent(loaded.window.content);
      setRevision(loaded.inspection.revision);
      setDirty(false);
    } catch (error) {
      setSaveError(String(error));
    }
  };

  const treeAvailable = parseResult?.ok === true;
  return (
    <section className="lattice-text-viewer" aria-label="Text resource viewer">
      <header className="lattice-text-toolbar">
        <div className="lattice-text-toolbar-group">
          <span className="lattice-text-kind">{syntaxInfo.syntax === "plain-text" ? "Text" : syntaxInfo.syntax.toUpperCase()}</span>
          <span className="lattice-text-size">{activeSession.totalSize.toLocaleString()} bytes</span>
          {!activeSession.editable && <span className="lattice-text-badge">Read-only window</span>}
        </div>
        <div className="lattice-text-toolbar-group">
          {treeAvailable && <button type="button" className="lattice-text-button" aria-pressed={showTree} onClick={() => setShowTree((value) => !value)}>{showTree ? "Hide tree" : "Show tree"}</button>}
          {activeSession.editable && <button type="button" className="lattice-text-button lattice-text-button-primary" disabled={!dirty} onClick={() => void save()}>Save</button>}
          {!activeSession.editable && onOpenExternally && (
            <button type="button" className="lattice-text-button" onClick={() => onOpenExternally(activeSession.resource)}>Open externally</button>
          )}
          <span className="lattice-text-status" role="status" aria-live="polite">{saveError ?? (dirty ? "Edited" : activeSession.editable ? "Saved" : "Viewing window")}</span>
        </div>
      </header>
      {!activeSession.editable && (
        <div className="lattice-text-window-controls" aria-label="Text window controls">
          <button type="button" className="lattice-text-button" disabled={activeSession.offset <= 0} onClick={() => void moveWindow(-1)}>Previous window</button>
          <span>Bytes {activeSession.offset.toLocaleString()}–{Math.min(activeSession.totalSize, activeSession.offset + activeSession.content.length).toLocaleString()} of {activeSession.totalSize.toLocaleString()}</span>
          <button type="button" className="lattice-text-button" disabled={activeSession.offset + activeSession.content.length >= activeSession.totalSize} onClick={() => void moveWindow(1)}>Next window</button>
        </div>
      )}
      <div className={`lattice-text-layout${showTree && treeAvailable ? " lattice-text-layout-with-tree" : ""}`}>
        <TextCodeMirror
          initialValue={content}
          syntax={syntaxInfo.syntax}
          language={syntaxInfo.language}
          readOnly={!activeSession.editable}
          resetKey={`${activeSession.resource.path}:${activeSession.offset}:${activeSession.revision}`}
          onChange={updateContent}
        />
        {showTree && treeAvailable && parseResult.ok && <StructuredTree root={parseResult.root} />}
      </div>
      {parseResult && !parseResult.ok && isStructured && (
        <p className="lattice-text-diagnostic" role="note">Structured view unavailable: {parseResult.diagnostics[0]?.message ?? "source is malformed"}. The source remains editable.</p>
      )}
    </section>
  );
}
