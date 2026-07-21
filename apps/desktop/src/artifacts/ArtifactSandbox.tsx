import { useEffect, useRef, useState, type CSSProperties } from "react";

import {
  isArtifactFrameMessage,
  type ArtifactHostToFrameMessage,
} from "../lib/artifactBridge";
import {
  readArtifactEntrypoint,
  resolveArtifactBinding,
  type ArtifactManifestDto,
} from "../lib/artifactRun";
import {
  detectSystemAppearance,
  readThemeMirror,
  selectThemeMirrorEntry,
} from "../theme/apply";
import "./artifactResource.css";

export interface ArtifactSandboxProps {
  root: string | null;
  packagePath: string;
  manifest: ArtifactManifestDto;
  /** Optional fixed height (interactive embeds). */
  height?: string | number | null;
  onOpenResource?: (path: string) => void;
  onTitleChange?: (title: string) => void;
  className?: string;
}

function parseHeight(height: string | number | null | undefined): CSSProperties | undefined {
  if (height == null || height === "") return undefined;
  if (typeof height === "number") return { height: `${height}px` };
  const trimmed = height.trim();
  if (!trimmed) return undefined;
  if (/^\d+$/.test(trimmed)) return { height: `${trimmed}px` };
  return { height: trimmed };
}

function collectThemeMessage(): ArtifactHostToFrameMessage {
  const mirror = readThemeMirror();
  if (mirror) {
    const entry = selectThemeMirrorEntry(mirror, detectSystemAppearance());
    return {
      type: "lattice.artifact.theme",
      vars: entry.vars,
      background: entry.background,
      appearance: entry.appearance,
    };
  }
  const root = document.documentElement;
  const vars: Record<string, string> = {};
  for (let i = 0; i < root.style.length; i++) {
    const name = root.style.item(i);
    if (name.startsWith("--lt-")) {
      vars[name] = root.style.getPropertyValue(name).trim();
    }
  }
  return {
    type: "lattice.artifact.theme",
    vars,
    background: root.style.background || undefined,
    appearance: root.style.colorScheme || undefined,
  };
}

/**
 * Sandboxed artifact iframe: no ambient Tauri, theme mirrored via postMessage,
 * named BindingSpec resolution only, suspends when off-screen.
 */
export function ArtifactSandbox({
  root,
  packagePath,
  manifest,
  height,
  onOpenResource,
  onTitleChange,
  className,
}: ArtifactSandboxProps) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [srcDoc, setSrcDoc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [visible, setVisible] = useState(true);
  const [frameHeight, setFrameHeight] = useState<number | null>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const onOpenRef = useRef(onOpenResource);
  const onTitleRef = useRef(onTitleChange);
  onOpenRef.current = onOpenResource;
  onTitleRef.current = onTitleChange;

  useEffect(() => {
    const element = wrapRef.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        setVisible(entries.some((entry) => entry.isIntersecting));
      },
      { rootMargin: "80px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!root) {
      setSrcDoc(null);
      setError("Open a native workspace to run sandboxed artifacts.");
      return;
    }
    if (!visible) return;

    const controller = new AbortController();
    setBusy(true);
    setError(null);
    void readArtifactEntrypoint(root, packagePath)
      .then((entrypoint) => {
        if (controller.signal.aborted) return;
        setSrcDoc(entrypoint.html);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setSrcDoc(null);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });
    return () => controller.abort();
  }, [packagePath, root, visible, manifest.entrypoint, manifest.packagePath]);

  useEffect(() => {
    if (!srcDoc || !root) return;

    const onMessage = (event: MessageEvent) => {
      if (frameRef.current && event.source !== frameRef.current.contentWindow) return;
      if (!isArtifactFrameMessage(event.data)) return;
      const message = event.data;
      switch (message.type) {
        case "lattice.artifact.requestBinding": {
          void resolveArtifactBinding(root, packagePath, message.name)
            .then((data) => {
              postToFrame({
                type: "lattice.artifact.bindingResult",
                id: message.id,
                ok: true,
                data,
              });
            })
            .catch((err: unknown) => {
              postToFrame({
                type: "lattice.artifact.bindingResult",
                id: message.id,
                ok: false,
                error: err instanceof Error ? err.message : String(err),
              });
            });
          break;
        }
        case "lattice.artifact.openResource":
          onOpenRef.current?.(message.path);
          break;
        case "lattice.artifact.notify":
          if (message.title) onTitleRef.current?.(message.title);
          if (typeof message.height === "number" && Number.isFinite(message.height)) {
            setFrameHeight(message.height);
          }
          break;
        default: {
          const _exhaustive: never = message;
          return _exhaustive;
        }
      }
    };

    function postToFrame(message: ArtifactHostToFrameMessage) {
      frameRef.current?.contentWindow?.postMessage(message, "*");
    }

    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [packagePath, root, srcDoc]);

  const handleLoad = () => {
    const win = frameRef.current?.contentWindow;
    if (!win) return;
    const bindingNames = Object.keys(manifest.bindings).sort();
    win.postMessage(
      {
        type: "lattice.artifact.init",
        title: manifest.title ?? null,
        bindings: bindingNames,
      } satisfies ArtifactHostToFrameMessage,
      "*",
    );
    win.postMessage(collectThemeMessage(), "*");
  };

  const heightStyle = parseHeight(height) ?? (frameHeight ? { height: `${frameHeight}px` } : undefined);

  return (
    <div
      ref={wrapRef}
      className={`artifact-sandbox${className ? ` ${className}` : ""}`}
      style={heightStyle}
    >
      {busy && !srcDoc ? (
        <p className="artifact-sandbox-status" role="status">
          Loading artifact…
        </p>
      ) : null}
      {error ? (
        <div className="artifact-sandbox-fallback" role="alert">
          <p className="artifact-sandbox-status">{error}</p>
          {manifest.fallback.file ? (
            <p className="artifact-sandbox-fallback-file">
              Fallback: <code>{manifest.fallback.file}</code>
            </p>
          ) : null}
          {manifest.fallback.text ? <p>{manifest.fallback.text}</p> : null}
        </div>
      ) : null}
      {visible && srcDoc && !error ? (
        <iframe
          ref={frameRef}
          className="artifact-sandbox-frame"
          title={manifest.title ?? "Artifact"}
          sandbox="allow-scripts"
          srcDoc={srcDoc}
          onLoad={handleLoad}
        />
      ) : null}
      {!visible && !error ? (
        <p className="artifact-sandbox-status" role="status">
          Artifact suspended (off-screen)
        </p>
      ) : null}
    </div>
  );
}
