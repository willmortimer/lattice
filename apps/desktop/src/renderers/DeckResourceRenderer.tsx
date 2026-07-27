import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Button, SurfaceHeader } from "@lattice/ui";

import { assembleDeckStaticDocument } from "../deck/latticeView";
import { createDeckPresentationSession, nearbySlideIndexes, resolveDeckSlideIndex } from "../presentation/presentationSession";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import "./deckResource.css";

const DEFAULT_TRANSITION_MS = 280;

function systemThemeVars(): Record<string, string> {
  const root = document.documentElement;
  const result: Record<string, string> = {};
  for (let index = 0; index < root.style.length; index += 1) {
    const name = root.style.item(index);
    if (name.startsWith("--lt-")) result[name] = root.style.getPropertyValue(name).trim();
  }
  return result;
}

function markdownText(notes?: string | null): string {
  // Notes are intentionally text-only here. No authored Markdown HTML crosses
  // into the presentation host before the notes renderer earns that authority.
  return (notes ?? "").replace(/^#{1,6}\s+/gm, "").replace(/[`*_>#]/g, "").trim();
}

function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
  useEffect(() => {
    const query = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!query) return;
    const change = () => setReduced(query.matches);
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  return reduced;
}

function elapsedLabel(milliseconds: number): string {
  const seconds = Math.floor(milliseconds / 1000);
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

interface DeckSlideFrameProps {
  slide: { id: string; html: string };
  deck: { title: string; themeCss: string };
  themeVars: Record<string, string>;
  workspaceRoot: string | null;
}

function DeckSlideFrame({ slide, deck, themeVars, workspaceRoot }: DeckSlideFrameProps) {
  const [srcDoc, setSrcDoc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void assembleDeckStaticDocument({
      html: slide.html,
      title: `${deck.title} — ${slide.id}`,
      styles: [deck.themeCss],
      themeVars,
      includeVocabulary: true,
      root: workspaceRoot ?? "",
    }).then((document) => {
      if (!cancelled) setSrcDoc(document);
    });
    return () => {
      cancelled = true;
    };
  }, [deck.themeCss, deck.title, slide.html, slide.id, themeVars, workspaceRoot]);

  return (
    <iframe
      title={`Slide: ${slide.id}`}
      sandbox=""
      srcDoc={srcDoc ?? undefined}
    />
  );
}

export function DeckResourceRenderer({ context, session }: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "deck") return null;
  const deck = session.deck;
  const presentation = useMemo(() => createDeckPresentationSession(deck, session.initialSlideId), [deck, session.initialSlideId]);
  const [index, setIndex] = useState(() => resolveDeckSlideIndex(presentation.orderedIds, presentation.initialId));
  const [overview, setOverview] = useState(false);
  const [notesOpen, setNotesOpen] = useState(false);
  const [audience, setAudience] = useState(false);
  const [running, setRunning] = useState(false);
  const [elapsedBase, setElapsedBase] = useState(0);
  const startedAt = useRef<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const reducedMotion = useReducedMotion();
  const stageRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setIndex(resolveDeckSlideIndex(presentation.orderedIds, presentation.initialId));
  }, [presentation]);
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [running]);
  useEffect(() => {
    const onFullscreen = () => setAudience(document.fullscreenElement === stageRef.current);
    document.addEventListener("fullscreenchange", onFullscreen);
    return () => document.removeEventListener("fullscreenchange", onFullscreen);
  }, []);
  const elapsed = elapsedBase + (running && startedAt.current ? now - startedAt.current : 0);
  const target = deck.slides[index];
  const go = (next: number) => setIndex(() => {
    if (deck.loop) return (next + deck.slides.length) % deck.slides.length;
    return Math.max(0, Math.min(deck.slides.length - 1, next));
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (["ArrowRight", "ArrowDown", "PageDown", " ", "Enter"].includes(event.key)) { event.preventDefault(); go(index + 1); }
      else if (["ArrowLeft", "ArrowUp", "PageUp", "Backspace"].includes(event.key)) { event.preventDefault(); go(index - 1); }
      else if (event.key === "Home") { event.preventDefault(); go(0); }
      else if (event.key === "End") { event.preventDefault(); go(deck.slides.length - 1); }
      else if (event.key === "Escape" && document.fullscreenElement) void document.exitFullscreen();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [deck.loop, deck.slides.length, index]);
  const themeVars = systemThemeVars();
  const transition = reducedMotion ? { type: "cut" as const } : (target?.transition ?? { type: "cut" as const });
  const transitionName = transition.type === "push" ? `push-${transition.direction ?? "left"}` : transition.type;
  const duration = transition.durationMs ?? transition.duration_ms ?? (transition.type === "cut" ? 0 : DEFAULT_TRANSITION_MS);
  const mounted = new Set(nearbySlideIndexes(index, deck.slides.length));
  const remaining = deck.durationMinutes ? Math.max(0, deck.durationMinutes * 60_000 - elapsed) : null;

  const toggleTimer = () => {
    if (running) { setElapsedBase(elapsed); startedAt.current = null; setRunning(false); }
    else { startedAt.current = Date.now(); setNow(Date.now()); setRunning(true); }
  };
  const toggleFullscreen = () => {
    if (document.fullscreenElement) { void document.exitFullscreen(); }
    else { void stageRef.current?.requestFullscreen?.(); }
  };
  return <section className="deck-resource" aria-label={`${deck.title} presentation`}>
    {!audience && <SurfaceHeader title={deck.title} />}
    {!audience && <div className="deck-toolbar" role="toolbar" aria-label="Presentation controls">
      <Button onClick={() => go(index - 1)} disabled={!deck.loop && index === 0}>Previous</Button>
      <label>Slide <select aria-label="Select slide" value={target?.id ?? ""} onChange={(event) => go(deck.slides.findIndex((slide) => slide.id === event.target.value))}>{deck.slides.map((slide, slideIndex) => <option value={slide.id} key={slide.id}>{slideIndex + 1}. {slide.id}</option>)}</select></label>
      <Button onClick={() => go(index + 1)} disabled={!deck.loop && index === deck.slides.length - 1}>Next</Button>
      <Button onClick={() => setOverview((value) => !value)}>{overview ? "Stage" : "Overview"}</Button>
      <Button onClick={() => setNotesOpen((value) => !value)}>{notesOpen ? "Hide notes" : "Notes"}</Button>
      <Button onClick={toggleTimer}>{running ? "Pause" : "Start"} {elapsedLabel(elapsed)}</Button>
      <Button onClick={() => { setRunning(false); startedAt.current = null; setElapsedBase(0); setNow(Date.now()); }}>Reset</Button>
      {remaining !== null && <output aria-label="Remaining time">{elapsedLabel(remaining)} remaining</output>}
      <Button onClick={toggleFullscreen}>{audience ? "Exit fullscreen" : "Fullscreen"}</Button>
    </div>}
    {overview ? <ol className="deck-overview" aria-label="Slide overview">{deck.slides.map((slide, slideIndex) => <li key={slide.id}><button type="button" onClick={() => { go(slideIndex); setOverview(false); }}><span>{slideIndex + 1}</span><strong>{slide.id}</strong><small>{slide.source}</small></button></li>)}</ol> : <div ref={stageRef} className="deck-stage" data-audience={audience || undefined} data-transition={transitionName} style={{ "--deck-transition-ms": `${duration}ms`, aspectRatio: deck.aspectRatio.replace(":", " / ") } as CSSProperties}>
      {deck.slides.map((slide, slideIndex) => mounted.has(slideIndex) ? <article key={slide.id} id={slide.id} className="deck-frame" data-current={slideIndex === index || undefined} aria-hidden={slideIndex !== index}>
        <DeckSlideFrame slide={slide} deck={deck} themeVars={themeVars} workspaceRoot={context.workspaceRoot} />
      </article> : null)}
      {!target && <div className="deck-degraded">The requested slide is missing. Select another slide from Overview.</div>}
    </div>}
    {notesOpen && !overview && <aside className="deck-notes" aria-label="Speaker notes"><h2>Notes — {target?.id ?? "missing"}</h2><pre>{markdownText(target?.notes) || "No speaker notes for this slide."}</pre></aside>}
  </section>;
}
