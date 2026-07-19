import { useEffect, useState } from "react";

import {
  extractShikiCodeInnerHtml,
  highlightCodeInWorker,
} from "./syntaxHighlight/shikiHighlight";
import { mapFenceLanguage } from "./syntaxHighlight/mapFenceLanguage";

const HIGHLIGHT_DEBOUNCE_MS = 120;

export interface CodeBlockHighlightProps {
  /** Raw fence body text. */
  text: string;
  /** Fence language attribute (may be an alias). */
  language: string | null;
  /** When false, pending work is cancelled and the overlay clears (ADR 0036). */
  isVisible: boolean;
}

/**
 * Deferred, worker-backed Shiki highlight for a code fence.
 * Returns inner HTML suitable for an absolute overlay over editable text.
 */
export function useCodeBlockHighlight({
  text,
  language,
  isVisible,
}: CodeBlockHighlightProps): string | null {
  const [html, setHtml] = useState<string | null>(null);

  useEffect(() => {
    if (!isVisible || text.length === 0) {
      setHtml(null);
      return;
    }

    const lang = mapFenceLanguage(language);
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      void highlightCodeInWorker(text, lang, controller.signal)
        .then((shikiHtml) => {
          if (!controller.signal.aborted) {
            setHtml(extractShikiCodeInnerHtml(shikiHtml));
          }
        })
        .catch((err: unknown) => {
          if (controller.signal.aborted) return;
          if (err instanceof DOMException && err.name === "AbortError") return;
          setHtml(null);
        });
    }, HIGHLIGHT_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [isVisible, text, language]);

  return html;
}
