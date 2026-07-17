import { useEffect, useState } from "react";
import { NodeViewContent, NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";

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
 * Read-view Mermaid embed: the fenced code stays fully editable through
 * `NodeViewContent` (ProseMirror owns that DOM directly); a block whose
 * language is `mermaid` additionally renders its diagram underneath,
 * loaded lazily so pages without one never pay for the dependency.
 *
 * Diagram layout is further deferred until the block is near the viewport
 * (ADR 0036).
 */
export function CodeBlockView({ node }: NodeViewProps) {
  const language = node.attrs.language as string | null;
  const isMermaid = language === "mermaid";
  const text = node.textContent;
  const { ref, isVisible } = useDeferredUntilVisible();
  const [svg, setSvg] = useState<string | null>(null);
  const [renderError, setRenderError] = useState<string | null>(null);

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

  return (
    <NodeViewWrapper className="code-block-view">
      <div ref={ref}>
        <pre>
          <NodeViewContent<"code"> as="code" />
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
