import { useEffect, useState } from "react";
import { NodeViewContent, NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";

import { useCodeBlockHighlight } from "./CodeBlockHighlight";
import { useDeferredUntilVisible } from "./visibilityDeferred";

type MermaidApi = typeof import("mermaid").default;

let mermaidModule: MermaidApi | null = null;
let mermaidThemeKey: string | null = null;

function readCssToken(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function shellAppearance(): "dark" | "light" {
  if (typeof document === "undefined") return "dark";
  const scheme = getComputedStyle(document.documentElement).colorScheme;
  if (scheme.includes("light")) return "light";
  return "dark";
}

function mermaidConfig() {
  const appearance = shellAppearance();
  const text = readCssToken("--lt-text", appearance === "light" ? "#1a1a1a" : "#f2ede3");
  const soft = readCssToken("--lt-text-soft", appearance === "light" ? "#5c5c5c" : "#c9c2b7");
  const panel = readCssToken("--lt-panel", appearance === "light" ? "#ffffff" : "#1a1f2a");
  const raise = readCssToken("--lt-bg-raise", appearance === "light" ? "#f4f1ea" : "#12161f");
  const line = readCssToken("--lt-line", appearance === "light" ? "#d0cbc0" : "#2a3140");
  const accent = readCssToken("--lt-accent", appearance === "light" ? "#0b57d0" : "#f5a623");
  return {
    startOnLoad: false,
    // Match shell appearance so node fills/text stay readable on --lt-bg-raise.
    theme: appearance === "light" ? ("default" as const) : ("dark" as const),
    themeVariables: {
      darkMode: appearance === "dark",
      background: raise,
      primaryColor: panel,
      primaryTextColor: text,
      primaryBorderColor: line,
      secondaryColor: raise,
      secondaryTextColor: soft,
      secondaryBorderColor: line,
      tertiaryColor: panel,
      tertiaryTextColor: text,
      tertiaryBorderColor: line,
      lineColor: soft,
      textColor: text,
      mainBkg: panel,
      nodeBorder: line,
      clusterBkg: raise,
      clusterBorder: line,
      titleColor: text,
      edgeLabelBackground: raise,
      actorBkg: panel,
      actorBorder: line,
      actorTextColor: text,
      signalColor: soft,
      labelBoxBkgColor: panel,
      labelTextColor: text,
      loopTextColor: soft,
      noteBkgColor: raise,
      noteTextColor: text,
      noteBorderColor: line,
      activationBkgColor: accent,
      activationBorderColor: line,
    },
  };
}

/** `mermaid.initialize` is process-wide; re-init when shell appearance changes. */
async function ensureMermaidInitialized(): Promise<MermaidApi> {
  const { default: mermaid } = await import("mermaid");
  const key = `${shellAppearance()}:${readCssToken("--lt-text", "")}:${readCssToken("--lt-panel", "")}`;
  if (!mermaidModule || mermaidThemeKey !== key) {
    mermaid.initialize(mermaidConfig());
    mermaidModule = mermaid;
    mermaidThemeKey = key;
  }
  return mermaid;
}

/**
 * Code fence node view: editable source stays in `NodeViewContent` while
 * Shiki highlighting paints an overlay (ADR 0036 / docs/07). Mermaid fences
 * skip Shiki and render a deferred SVG diagram underneath instead.
 */
export function CodeBlockView({ node }: NodeViewProps) {
  const language = node.attrs.language as string | null;
  const isMermaid = language === "mermaid";
  const text = node.textContent;
  const { ref, isVisible } = useDeferredUntilVisible();
  const [svg, setSvg] = useState<string | null>(null);
  const [renderError, setRenderError] = useState<string | null>(null);

  const highlightHtml = useCodeBlockHighlight({
    text,
    language,
    isVisible: isVisible && !isMermaid,
  });
  const showHighlight = !isMermaid && highlightHtml != null;

  useEffect(() => {
    if (!isVisible || !isMermaid || text.trim().length === 0) {
      setSvg(null);
      setRenderError(null);
      return;
    }

    let cancelled = false;
    const diagramId = `mermaid-${Math.random().toString(36).slice(2)}`;

    ensureMermaidInitialized()
      .then((mermaid) => mermaid.render(diagramId, text))
      .then(({ svg: rendered }) => {
        if (!cancelled) {
          setSvg(rendered);
          setRenderError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setSvg(null);
          setRenderError(err instanceof Error ? err.message : String(err));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isVisible, isMermaid, text]);

  const preClassName = showHighlight ? "code-block-pre code-block-pre--highlighted" : "code-block-pre";

  return (
    <NodeViewWrapper className="code-block-view">
      <div ref={ref}>
        <pre className={preClassName}>
          <span className="code-block-stack">
            {showHighlight && (
              // eslint-disable-next-line react/no-danger -- Shiki token HTML from our worker, not raw user HTML
              <span
                className="code-block-highlight-layer"
                aria-hidden="true"
                dangerouslySetInnerHTML={{ __html: highlightHtml }}
              />
            )}
            <NodeViewContent<"code">
              as="code"
              className={showHighlight ? "code-block-edit-layer" : undefined}
            />
          </span>
        </pre>
        {isMermaid && !isVisible && (
          <p className="page-embed-deferred-hint" role="status">
            Diagram preview loads when visible.
          </p>
        )}
        {isMermaid && isVisible && svg && (
          // eslint-disable-next-line react/no-danger -- mermaid.render output, not user HTML
          <div className="mermaid-preview" dangerouslySetInnerHTML={{ __html: svg }} />
        )}
        {isMermaid && isVisible && renderError && (
          <p className="error-text mermaid-error">{renderError}</p>
        )}
      </div>
    </NodeViewWrapper>
  );
}
