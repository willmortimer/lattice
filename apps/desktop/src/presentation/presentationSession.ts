import type { DeckSessionDto } from "../lib/deckRun";

/** Future Page and Canvas sequencers share this small host-owned boundary. */
export type PresentationKind = "page" | "deck" | "canvas";
export interface PresentationSession {
  kind: PresentationKind;
  id: string;
  title: string;
  orderedIds: readonly string[];
  initialId: string;
}

/** Deck is the only registered presentation source in this delivery. */
export function createDeckPresentationSession(deck: DeckSessionDto, anchor?: string | null): PresentationSession {
  const orderedIds = deck.slides.map((slide) => slide.id);
  const initialId = anchor && orderedIds.includes(anchor)
    ? anchor
    : deck.start && orderedIds.includes(deck.start)
      ? deck.start
      : orderedIds[0] ?? "";
  return { kind: "deck", id: deck.id, title: deck.title, orderedIds, initialId };
}

export function nearbySlideIndexes(current: number, count: number): number[] {
  return [current - 1, current, current + 1].filter((index) => index >= 0 && index < count);
}

export function resolveDeckSlideIndex(ids: readonly string[], anchor?: string | null): number {
  const index = anchor ? ids.indexOf(anchor) : -1;
  return index >= 0 ? index : 0;
}
