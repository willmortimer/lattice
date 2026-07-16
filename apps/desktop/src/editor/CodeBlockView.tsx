import { useEffect, useState } from "react";
import { NodeViewContent, NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";

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
 */
export function CodeBlockView({ node }: NodeViewProps) {
  const language = node.attrs.language as string | null;
  const isMermaid = language === "mermaid";
  const text = node.textContent;
  const [svg, setSvg] = useState<string | null>(null);
  const [renderError, setRenderError] = useState<string | null>(null);

  useEffect(() => {
    if (!isMermaid || text.trim().length === 0) {
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
  }, [isMermaid, text]);

  return (
    <NodeViewWrapper className="code-block-view">
      <pre>
        <NodeViewContent<"code"> as="code" />
      </pre>
      {isMermaid && svg && (
        // eslint-disable-next-line react/no-danger -- mermaid.render output, not user HTML
        <div className="mermaid-preview" dangerouslySetInnerHTML={{ __html: svg }} />
      )}
      {isMermaid && renderError && <p className="error-text mermaid-error">{renderError}</p>}
    </NodeViewWrapper>
  );
}
