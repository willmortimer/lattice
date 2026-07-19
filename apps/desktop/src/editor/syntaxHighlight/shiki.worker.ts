import type { ShikiHighlightRequest, ShikiHighlightResponse } from "./shikiProtocol";
import { highlightWithLatticeShiki } from "./latticeHighlighter";

self.onmessage = (event: MessageEvent<ShikiHighlightRequest>) => {
  const { id, code, lang } = event.data;
  void highlightWithLatticeShiki(code, lang)
    .then((html) => {
      const response: ShikiHighlightResponse = { id, ok: true, html };
      self.postMessage(response);
    })
    .catch((err: unknown) => {
      const response: ShikiHighlightResponse = {
        id,
        ok: false,
        error: err instanceof Error ? err.message : String(err),
      };
      self.postMessage(response);
    });
};
