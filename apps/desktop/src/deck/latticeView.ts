/**
 * Host-side materialization for script-free `<lattice-view>` slide elements.
 *
 * A viewbox is a snapshot/card, never a nested resource renderer. This keeps
 * Deck HTML portable and prevents a static iframe from acquiring workspace,
 * Tauri, network, or live-component authority.
 */
import { invoke } from "@tauri-apps/api/core";

import { assembleStaticDocument, type StaticDocumentInput } from "../artifacts/staticDocument";

export const MAX_DECK_VIEWBOX_IMAGE_BYTES = 8 * 1024 * 1024;
export const MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES = 32 * 1024 * 1024;

export type DeckViewMode = "static" | "live";

export interface DeckViewboxDto {
  resource: string;
  kind: string;
  title: string;
  state: "static" | "degraded" | "live-fallback";
  excerpt?: string | null;
  imageDataUrl?: string | null;
  message?: string | null;
  byteLength: number;
}

export interface LatticeViewRequest {
  resource: string;
  mode: DeckViewMode;
}

export type DeckViewboxInvoker = (root: string, request: LatticeViewRequest) => Promise<DeckViewboxDto>;

export const invokeDeckViewbox: DeckViewboxInvoker = (root, request) =>
  invoke<DeckViewboxDto>("deck_materialize_viewbox", { request: { root, ...request } });

/** Parse only the deliberately small source contract accepted by static Decks. */
export function parseLatticeViewRequest(attributes: Pick<Element, "getAttribute">): LatticeViewRequest | null {
  const resource = attributes.getAttribute("resource")?.trim();
  if (!resource) return null;
  const rawMode = attributes.getAttribute("mode")?.trim().toLowerCase();
  return { resource, mode: rawMode === "live" ? "live" : "static" };
}

function cardElement(doc: Document, view: DeckViewboxDto, request?: LatticeViewRequest): HTMLElement {
  const card = doc.createElement("section");
  card.className = `lt-card lattice-viewbox lattice-viewbox--${view.state}`;
  card.setAttribute("data-lattice-view-resource", request?.resource ?? view.resource);
  card.setAttribute("data-lattice-view-mode", request?.mode ?? "static");
  card.setAttribute("data-lattice-view-kind", view.kind);

  const header = doc.createElement("div");
  header.className = "lt-cluster lattice-viewbox__header";
  const title = doc.createElement("strong");
  title.textContent = view.title;
  const kind = doc.createElement("span");
  kind.className = "lt-badge";
  kind.textContent = view.kind;
  header.append(title, kind);
  card.append(header);

  if (view.imageDataUrl) {
    const image = doc.createElement("img");
    image.src = view.imageDataUrl;
    image.alt = view.title;
    image.loading = "lazy";
    image.className = "lattice-viewbox__image";
    card.append(image);
  }
  if (view.excerpt) {
    const excerpt = doc.createElement("p");
    excerpt.className = "lattice-viewbox__excerpt";
    excerpt.textContent = view.excerpt;
    card.append(excerpt);
  }
  if (view.message) {
    const notice = doc.createElement("p");
    notice.className = view.state === "degraded" ? "lt-degraded" : "lt-callout";
    notice.textContent = view.message;
    card.append(notice);
  }
  return card;
}

function degradedCard(doc: Document, message: string, request?: LatticeViewRequest): HTMLElement {
  return cardElement(doc, {
    resource: request?.resource ?? "",
    kind: "resource",
    title: request?.resource ?? "Unavailable viewbox",
    state: "degraded",
    message,
    byteLength: 0,
  }, request);
}

/**
 * Replace every view element with an inert card. The original slide source is
 * never written back; `mode="live"` is preserved on the card for future
 * migration while rendering an explicit static fallback today.
 */
export async function materializeLatticeViews(input: {
  html: string;
  root: string;
  materialize?: DeckViewboxInvoker;
}): Promise<string> {
  const parser = new DOMParser();
  const doc = parser.parseFromString(input.html, "text/html");
  const materialize = input.materialize ?? invokeDeckViewbox;
  let imageBytes = 0;

  for (const element of [...doc.querySelectorAll("lattice-view")]) {
    const request = parseLatticeViewRequest(element);
    if (!request) {
      element.replaceWith(degradedCard(doc, "lattice-view requires a workspace-relative resource attribute."));
      continue;
    }
    try {
      const result = await materialize(input.root, request);
      if (result.imageDataUrl) {
        if (result.byteLength > MAX_DECK_VIEWBOX_IMAGE_BYTES || imageBytes + result.byteLength > MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES) {
          element.replaceWith(degradedCard(doc, "Raster viewboxes exceed the Deck's bounded inline image budget.", request));
          continue;
        }
        imageBytes += result.byteLength;
      }
      element.replaceWith(cardElement(doc, result, request));
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      element.replaceWith(degradedCard(doc, `Unable to materialize this viewbox: ${detail}`, request));
    }
  }
  return doc.body.innerHTML;
}

/** Async wrapper around the shared static-document assembly boundary. */
export async function assembleDeckStaticDocument(
  input: StaticDocumentInput & { root: string; materialize?: DeckViewboxInvoker },
): Promise<string> {
  const html = await materializeLatticeViews(input);
  return assembleStaticDocument({
    html,
    title: input.title,
    styles: input.styles,
    assets: input.assets,
    themeVars: input.themeVars,
    includeVocabulary: input.includeVocabulary,
  });
}
