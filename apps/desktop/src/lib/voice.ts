import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { invoke } from "./ipc";

export type VoiceStatus = {
  available: boolean;
  prepared: boolean;
  preparing: boolean;
  listening: boolean;
  /** True when Rust owns mic capture (no WebView PCM). */
  nativeCapture: boolean;
  platform: string;
  message: string | null;
};

export type VoiceSessionStart = {
  sessionId: string;
};

export type VoiceUiEvent =
  | { type: "partial"; sessionId: string; revision: number; text: string }
  | {
      type: "final";
      sessionId: string;
      text: string;
      replacesRevision: number | null;
    }
  | { type: "status"; state: string; message: string | null }
  | { type: "failed"; sessionId: string | null; message: string };

export async function getVoiceStatus(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_status");
}

export async function prepareVoiceModel(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_prepare");
}

export async function startVoiceSession(): Promise<VoiceSessionStart> {
  return invoke<VoiceSessionStart>("voice_start_session");
}

export async function finishVoiceSession(sessionId: string): Promise<void> {
  await invoke("voice_finish_session", { sessionId });
}

export async function cancelVoiceSession(sessionId: string): Promise<void> {
  await invoke("voice_cancel_session", { sessionId });
}

/** Cancel any active session without needing its id (release-during-start). */
export async function cancelActiveVoiceSession(): Promise<void> {
  await invoke("voice_cancel_active");
}

export async function listenVoiceEvents(
  onEvent: (event: VoiceUiEvent) => void,
): Promise<UnlistenFn> {
  return listen<VoiceUiEvent>("voice-event", (event) => {
    onEvent(event.payload);
  });
}

/**
 * Hold-to-talk session intent for the WebView.
 *
 * Microphone capture and PCM transport are owned by the native Tauri path
 * (`lattice-audio-macos`). This class only tracks the active session id and
 * calls finish/cancel.
 */
export class DictationCapture {
  private sessionId: string | null = null;

  get active(): boolean {
    return this.sessionId !== null;
  }

  /** Bind UI hold to a Rust-owned capture session (no WebView DSP). */
  async start(sessionId: string): Promise<void> {
    this.sessionId = sessionId;
  }

  async stopAndFinish(sessionId = this.sessionId): Promise<void> {
    if (!sessionId) return;
    this.sessionId = null;
    await finishVoiceSession(sessionId);
  }

  async cancel(sessionId = this.sessionId): Promise<void> {
    this.sessionId = null;
    if (sessionId) {
      await cancelVoiceSession(sessionId);
    } else {
      await cancelActiveVoiceSession();
    }
  }
}
