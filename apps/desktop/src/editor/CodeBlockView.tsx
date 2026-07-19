import { useEffect, useState } from "react";
import { NodeViewContent, NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";

import { useCodeBlockHighlight } from "./CodeBlockHighlight";
import { useDeferredUntilVisible } from "./visibilityDeferred";

let mermaidInitialized = false;

/** `mermaid.initialize` is process-wide and idempotent-unsafe to call
 * twice with conflicting config, so every code block view shares one
 * init rather than each calling it on its own first render. */
async function ensureMermaidInitialized(): Promise<typeof import("mermaid").default> {
  const { default: mermaid } = await import("mermaid");
  if (!mermaidInitialized) {
    mermaid.initialize({ startOnLoad: false, theme: "dark" });
    mermaidInitialized = true;
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
