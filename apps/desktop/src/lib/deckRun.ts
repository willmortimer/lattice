import { invoke } from "@tauri-apps/api/core";

export type DeckAspectRatio = "16:9" | "4:3";
export type DeckTransition = {
  type: "cut" | "fade" | "push";
  direction?: "left" | "right" | "up" | "down";
  durationMs?: number;
  /** Rust's canonical manifest uses snake_case; transport retains it. */
  duration_ms?: number;
};
export interface DeckSlideDto { id: string; source: string; html: string; notes?: string | null; transition?: DeckTransition | null; }
export interface DeckSessionDto {
  format: "lattice-deck";
  version: number;
  id: string;
  title: string;
  aspectRatio: DeckAspectRatio;
  themeCss: string;
  slides: DeckSlideDto[];
  start?: string | null;
  loop: boolean;
  durationMinutes?: number | null;
  packagePath: string;
}

/** Native package read. Rendering remains host-owned and script-free. */
export function loadDeckSession(root: string, relPath: string): Promise<DeckSessionDto> {
  return invoke<DeckSessionDto>("deck_load_session", { request: { root, relPath } });
}
