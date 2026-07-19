import type { HighlighterCore } from "shiki/core";
import { createCssVariablesTheme, createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";

import bash from "@shikijs/langs/bash";
import javascript from "@shikijs/langs/javascript";
import json from "@shikijs/langs/json";
import python from "@shikijs/langs/python";
import rust from "@shikijs/langs/rust";
import sql from "@shikijs/langs/sql";
import typescript from "@shikijs/langs/typescript";
import yaml from "@shikijs/langs/yaml";

export const LATTICE_SHIKI_THEME_NAME = "lattice-css-variables";

const latticeCssVariablesTheme = createCssVariablesTheme({
  name: LATTICE_SHIKI_THEME_NAME,
  variablePrefix: "--shiki-",
  fontStyle: true,
});

/** Explicit grammar set — keep in sync with `mapFenceLanguage` aliases. */
const HIGHLIGHT_LANGS = [
  typescript,
  javascript,
  rust,
  python,
  json,
  yaml,
  bash,
  sql,
] as const;

let highlighterPromise: Promise<HighlighterCore> | null = null;

/**
 * Singleton fine-grained highlighter (worker or main-thread fallback).
 * Avoids the full `shiki` language catalog in the desktop bundle.
 */
export function getLatticeHighlighter(): Promise<HighlighterCore> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      langs: [...HIGHLIGHT_LANGS],
      themes: [latticeCssVariablesTheme],
      engine: createOnigurumaEngine(import("shiki/wasm")),
    });
  }
  return highlighterPromise;
}

export async function highlightWithLatticeShiki(code: string, lang: string): Promise<string> {
  const highlighter = await getLatticeHighlighter();
  const resolvedLang = highlighter.getLoadedLanguages().includes(lang) ? lang : "text";
  return highlighter.codeToHtml(code, {
    lang: resolvedLang,
    theme: LATTICE_SHIKI_THEME_NAME,
  });
}
