import { parseStructuredSource, type StructuredParseLimits, type StructuredSyntax } from "./structuredParserCore";

interface ParseRequest {
  id: number;
  source: string;
  syntax: StructuredSyntax;
  limits?: Partial<StructuredParseLimits>;
}

self.onmessage = (event: MessageEvent<ParseRequest>) => {
  const request = event.data;
  const result = parseStructuredSource(request.source, request.syntax, request.limits);
  self.postMessage({ id: request.id, result });
};
