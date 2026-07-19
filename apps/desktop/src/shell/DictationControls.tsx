import { CircleNotch, Microphone } from "@phosphor-icons/react";
import type { RefObject } from "react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import type { PageEditorHandle } from "../editor/PageEditor";
import { inBrowser } from "../demo";
import {
  cancelVoiceSession,
  DictationCapture,
  getVoiceStatus,
  listenVoiceEvents,
  prepareVoiceModel,
  startVoiceSession,
  type VoiceStatus,
} from "../lib/voice";

type DictationPhase = "idle" | "preparing" | "listening" | "finalizing" | "unavailable";

interface DictationControlsProps {
  enabled: boolean;
  pageEditorRef: RefObject<PageEditorHandle | null>;
  onError: (message: string) => void;
}

/**
 * Hold-to-talk dictation control for the page header (M2).
 * Provisional text is decoration-only; finals insert through the editor handle.
 */
export function DictationControls({ enabled, pageEditorRef, onError }: DictationControlsProps) {
  const labelId = useId();
  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const captureRef = useRef(new DictationCapture());
  const sessionIdRef = useRef<string | null>(null);
  const anchorRef = useRef(0);
  const highestRevisionRef = useRef(0);
  const holdingRef = useRef(false);
  const startGenerationRef = useRef(0);

  useEffect(() => {
    if (inBrowser || !enabled) {
      setPhase("unavailable");
      return;
    }
    let cancelled = false;
    void getVoiceStatus()
      .then((next) => {
        if (cancelled) return;
        setStatus(next);
        setPhase(next.available ? (next.preparing ? "preparing" : "idle") : "unavailable");
        if (next.message) setHint(next.message);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setPhase("unavailable");
        setHint(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [enabled]);

  useEffect(() => {
    if (inBrowser || !enabled) return;
    let unlisten: (() => void) | undefined;
    void listenVoiceEvents((event) => {
      if (event.type === "partial") {
        if (event.revision < highestRevisionRef.current) return;
        highestRevisionRef.current = event.revision;
        pageEditorRef.current?.setDictationProvisional(event.text, anchorRef.current);
        return;
      }
      if (event.type === "final") {
        pageEditorRef.current?.commitDictationFinal(event.text, anchorRef.current);
        highestRevisionRef.current = 0;
        setPhase("idle");
        setHint(null);
        return;
      }
      if (event.type === "status") {
        setHint(event.message);
        if (event.state === "listening") setPhase("listening");
        if (event.state === "finalizing") setPhase("finalizing");
        if (event.state === "preparing") setPhase("preparing");
        if (event.state === "ready") {
          setStatus((prev) =>
            prev
              ? { ...prev, prepared: true, preparing: false, message: event.message }
              : prev,
          );
          if (!holdingRef.current) setPhase("idle");
        }
        if (event.state === "idle" && !holdingRef.current) setPhase("idle");
        return;
      }
      if (event.type === "failed") {
        pageEditorRef.current?.clearDictationProvisional();
        holdingRef.current = false;
        sessionIdRef.current = null;
        setPhase("idle");
        onError(event.message);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [enabled, onError, pageEditorRef]);

  const beginHold = useCallback(async () => {
    if (!enabled || inBrowser || phase === "unavailable" || phase === "preparing") return;
    if (holdingRef.current) return;
    holdingRef.current = true;
    highestRevisionRef.current = 0;
    const generation = ++startGenerationRef.current;

    try {
      if (!status?.prepared) {
        setPhase("preparing");
        const prepared = await prepareVoiceModel();
        if (!holdingRef.current || generation !== startGenerationRef.current) return;
        setStatus(prepared);
      }
      if (!holdingRef.current || generation !== startGenerationRef.current) return;

      setPhase("listening");
      const anchor = pageEditorRef.current?.beginDictation() ?? 0;
      anchorRef.current = anchor;
      const { sessionId } = await startVoiceSession();
      if (!holdingRef.current || generation !== startGenerationRef.current) {
        await cancelVoiceSession(sessionId).catch(() => undefined);
        setPhase("idle");
        return;
      }
      sessionIdRef.current = sessionId;
      await captureRef.current.start(sessionId);
      if (!holdingRef.current || generation !== startGenerationRef.current) {
        await captureRef.current.cancel().catch(() => undefined);
        sessionIdRef.current = null;
        setPhase("idle");
      }
    } catch (err) {
      if (generation !== startGenerationRef.current) return;
      holdingRef.current = false;
      sessionIdRef.current = null;
      pageEditorRef.current?.clearDictationProvisional();
      setPhase("idle");
      onError(err instanceof Error ? err.message : String(err));
    }
  }, [enabled, onError, pageEditorRef, phase, status?.prepared]);

  const endHold = useCallback(async () => {
    if (!holdingRef.current) return;
    holdingRef.current = false;
    startGenerationRef.current += 1;
    const sessionId = sessionIdRef.current;
    sessionIdRef.current = null;
    if (!sessionId) {
      // Released during prepare / session start — abandon without finishing.
      pageEditorRef.current?.clearDictationProvisional();
      setPhase("idle");
      return;
    }
    setPhase("finalizing");
    try {
      await captureRef.current.stopAndFinish();
    } catch (err) {
      pageEditorRef.current?.clearDictationProvisional();
      setPhase("idle");
      onError(err instanceof Error ? err.message : String(err));
    }
  }, [onError, pageEditorRef]);

  const cancelHold = useCallback(async () => {
    holdingRef.current = false;
    startGenerationRef.current += 1;
    sessionIdRef.current = null;
    pageEditorRef.current?.clearDictationProvisional();
    try {
      await captureRef.current.cancel();
    } catch {
      // Best-effort cancel.
    }
    setPhase("idle");
    setHint(null);
  }, [pageEditorRef]);

  if (!enabled || phase === "unavailable") {
    return null;
  }

  const listening = phase === "listening";
  const busy = phase === "preparing" || phase === "finalizing";
  const label =
    phase === "listening"
      ? "Listening — release to insert"
      : phase === "preparing"
        ? "Preparing model…"
        : phase === "finalizing"
          ? "Inserting…"
          : status?.prepared
            ? "Hold to dictate"
            : "Hold to prepare & dictate";

  return (
    <div
      className={[
        "dictation-controls",
        listening ? "is-listening" : "",
        busy ? "is-busy" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <button
        type="button"
        className="dictation-ptt"
        aria-labelledby={labelId}
        aria-pressed={listening}
        disabled={busy}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.preventDefault();
          event.currentTarget.setPointerCapture(event.pointerId);
          void beginHold();
        }}
        onPointerUp={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId);
          }
          void endHold();
        }}
        onPointerCancel={() => {
          void cancelHold();
        }}
        onLostPointerCapture={() => {
          if (holdingRef.current) void endHold();
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape" && holdingRef.current) {
            event.preventDefault();
            void cancelHold();
          }
          if (event.key === " " || event.key === "Enter") {
            if (event.repeat) return;
            event.preventDefault();
            void beginHold();
          }
        }}
        onKeyUp={(event) => {
          if (event.key === " " || event.key === "Enter") {
            event.preventDefault();
            void endHold();
          }
        }}
      >
        <span className="dictation-ptt-glyph" aria-hidden>
          {busy ? <CircleNotch size={15} className="dictation-spinner" /> : <Microphone size={15} weight={listening ? "fill" : "regular"} />}
        </span>
        {listening && <span className="dictation-pulse" aria-hidden />}
      </button>
      <div className="dictation-meta">
        <span id={labelId} className="dictation-label">
          {label}
        </span>
        {hint && phase !== "idle" && (
          <span className="dictation-hint" role="status">
            {hint}
          </span>
        )}
      </div>
    </div>
  );
}
