import { CircleNotch, Microphone } from "@phosphor-icons/react";
import { forwardRef, useCallback, useEffect, useId, useImperativeHandle, useRef, useState } from "react";

import { inBrowser } from "../demo";
import {
  cancelActiveVoiceSession,
  cancelVoiceSession,
  DictationCapture,
  getVoiceStatus,
  listenVoiceEvents,
  prepareVoiceModel,
  startVoiceSession,
  type VoiceSessionContextHints,
  type VoiceStatus,
} from "../lib/voice";

type DictationPhase = "idle" | "preparing" | "listening" | "finalizing" | "unavailable";

export type QuickNoteDictationHandle = {
  cancel: () => void;
};

export type QuickNoteDictationProps = {
  enabled: boolean;
  ensurePageReady: () => Promise<boolean>;
  getInsertPosition: () => number;
  voiceContext: VoiceSessionContextHints | null;
  onProvisionalChange: (text: string | null, anchor: number) => void;
  onFinal: (text: string, anchor: number) => void;
  onError: (message: string) => void;
};

/**
 * Hold-to-talk dictation for the Quick Note window. Provisional text is
 * decoration-only; finals are committed by the parent in one save path.
 */
export const QuickNoteDictation = forwardRef<QuickNoteDictationHandle, QuickNoteDictationProps>(
  function QuickNoteDictation(
    {
      enabled,
      ensurePageReady,
      getInsertPosition,
      voiceContext,
      onProvisionalChange,
      onFinal,
      onError,
    },
    ref,
  ) {
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
    const acceptPartialsRef = useRef(false);
    const opChainRef = useRef(Promise.resolve());

    const enqueue = useCallback((op: () => Promise<void>) => {
      const next = opChainRef.current.then(op, op);
      opChainRef.current = next.then(
        () => undefined,
        () => undefined,
      );
      return next;
    }, []);

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

    const voiceContextRef = useRef(voiceContext);
    useEffect(() => {
      voiceContextRef.current = voiceContext;
    }, [voiceContext]);

    const clearProvisional = useCallback(() => {
      onProvisionalChange(null, 0);
    }, [onProvisionalChange]);

    useEffect(() => {
      if (inBrowser || !enabled) return;
      let unlisten: (() => void) | undefined;
      void listenVoiceEvents((event) => {
        if (event.type === "partial") {
          if (!acceptPartialsRef.current) return;
          if (sessionIdRef.current && event.sessionId !== sessionIdRef.current) return;
          if (event.revision < highestRevisionRef.current) return;
          highestRevisionRef.current = event.revision;
          onProvisionalChange(event.text, anchorRef.current);
          return;
        }
        if (event.type === "final") {
          acceptPartialsRef.current = false;
          clearProvisional();
          onFinal(event.text, anchorRef.current);
          highestRevisionRef.current = 0;
          setPhase("idle");
          setHint(null);
          return;
        }
        if (event.type === "status") {
          setHint(event.message);
          if (event.state === "listening") {
            acceptPartialsRef.current = true;
            setPhase("listening");
          }
          if (event.state === "finalizing") {
            acceptPartialsRef.current = false;
            clearProvisional();
            setPhase("finalizing");
          }
          if (event.state === "preparing") setPhase("preparing");
          if (event.state === "ready") {
            setStatus((prev) =>
              prev ? { ...prev, prepared: true, preparing: false, message: event.message } : prev,
            );
            if (!holdingRef.current) setPhase("idle");
          }
          if (event.state === "idle" && !holdingRef.current) {
            acceptPartialsRef.current = false;
            setPhase("idle");
          }
          return;
        }
        if (event.type === "failed") {
          acceptPartialsRef.current = false;
          clearProvisional();
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
    }, [clearProvisional, enabled, onError, onFinal, onProvisionalChange]);

    const beginHold = useCallback(() => {
      if (!enabled || inBrowser) return;
      if (holdingRef.current) return;
      holdingRef.current = true;
      highestRevisionRef.current = 0;
      acceptPartialsRef.current = false;
      const generation = ++startGenerationRef.current;

      void enqueue(async () => {
        if (!holdingRef.current || generation !== startGenerationRef.current) return;

        try {
          const ready = await ensurePageReady();
          if (!ready || !holdingRef.current || generation !== startGenerationRef.current) {
            holdingRef.current = false;
            await cancelActiveVoiceSession().catch(() => undefined);
            setPhase("idle");
            return;
          }

          if (!status?.prepared) {
            setPhase("preparing");
            const prepared = await prepareVoiceModel();
            if (!holdingRef.current || generation !== startGenerationRef.current) {
              await cancelActiveVoiceSession().catch(() => undefined);
              setPhase("idle");
              return;
            }
            setStatus(prepared);
          }
          if (!holdingRef.current || generation !== startGenerationRef.current) {
            await cancelActiveVoiceSession().catch(() => undefined);
            setPhase("idle");
            return;
          }

          setPhase("listening");
          acceptPartialsRef.current = true;
          anchorRef.current = getInsertPosition();
          const { sessionId } = await startVoiceSession(voiceContextRef.current ?? undefined);
          if (!holdingRef.current || generation !== startGenerationRef.current) {
            acceptPartialsRef.current = false;
            await cancelVoiceSession(sessionId).catch(() => undefined);
            await cancelActiveVoiceSession().catch(() => undefined);
            sessionIdRef.current = null;
            setPhase("idle");
            return;
          }
          sessionIdRef.current = sessionId;
          await captureRef.current.start(sessionId);
          if (!holdingRef.current || generation !== startGenerationRef.current) {
            acceptPartialsRef.current = false;
            await captureRef.current.cancel().catch(() => undefined);
            sessionIdRef.current = null;
            setPhase("idle");
          }
        } catch (err) {
          if (generation !== startGenerationRef.current) return;
          acceptPartialsRef.current = false;
          holdingRef.current = false;
          sessionIdRef.current = null;
          clearProvisional();
          await cancelActiveVoiceSession().catch(() => undefined);
          setPhase("idle");
          onError(err instanceof Error ? err.message : String(err));
        }
      });
    }, [clearProvisional, enabled, enqueue, ensurePageReady, getInsertPosition, onError, status?.prepared]);

    const endHold = useCallback(() => {
      if (!holdingRef.current) return;
      holdingRef.current = false;
      acceptPartialsRef.current = false;
      startGenerationRef.current += 1;
      const sessionId = sessionIdRef.current;
      sessionIdRef.current = null;
      clearProvisional();

      void enqueue(async () => {
        if (!sessionId) {
          await cancelActiveVoiceSession().catch(() => undefined);
          setPhase("idle");
          return;
        }
        setPhase("finalizing");
        try {
          await captureRef.current.stopAndFinish(sessionId);
        } catch (err) {
          clearProvisional();
          await cancelVoiceSession(sessionId).catch(() => undefined);
          await cancelActiveVoiceSession().catch(() => undefined);
          setPhase("idle");
          onError(err instanceof Error ? err.message : String(err));
        }
      });
    }, [clearProvisional, enqueue, onError]);

    const cancelHold = useCallback(() => {
      holdingRef.current = false;
      acceptPartialsRef.current = false;
      startGenerationRef.current += 1;
      const sessionId = sessionIdRef.current;
      sessionIdRef.current = null;
      clearProvisional();

      void enqueue(async () => {
        try {
          await captureRef.current.cancel(sessionId);
        } catch {
          await cancelActiveVoiceSession().catch(() => undefined);
        }
        setPhase("idle");
        setHint(null);
      });
    }, [clearProvisional, enqueue]);

    useImperativeHandle(ref, () => ({ cancel: cancelHold }), [cancelHold]);

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
          "quick-note-dictation",
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
            beginHold();
          }}
          onPointerUp={(event) => {
            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
              event.currentTarget.releasePointerCapture(event.pointerId);
            }
            endHold();
          }}
          onPointerCancel={() => {
            cancelHold();
          }}
          onLostPointerCapture={() => {
            if (holdingRef.current) endHold();
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && holdingRef.current) {
              event.preventDefault();
              cancelHold();
            }
            if (event.key === " " || event.key === "Enter") {
              if (event.repeat) return;
              event.preventDefault();
              beginHold();
            }
          }}
          onKeyUp={(event) => {
            if (event.key === " " || event.key === "Enter") {
              event.preventDefault();
              endHold();
            }
          }}
        >
          <span className="dictation-ptt-glyph" aria-hidden>
            {busy ? (
              <CircleNotch size={15} className="dictation-spinner" />
            ) : (
              <Microphone size={15} weight={listening ? "fill" : "regular"} />
            )}
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
  },
);
