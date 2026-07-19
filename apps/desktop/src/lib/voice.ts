import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { parse as parseYaml } from "yaml";

import { splitFrontmatter } from "../editor/markdown";
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

export type VoiceSessionContextHints = {
  documentId?: string | null;
  documentPath?: string | null;
  pageTitle?: string | null;
  workspaceName?: string | null;
  tags?: string[];
  headingPath?: string[];
  glossaryTerms?: string[];
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

function serializeVoiceHints(hints: VoiceSessionContextHints) {
  return {
    documentId: hints.documentId ?? null,
    documentPath: hints.documentPath ?? null,
    pageTitle: hints.pageTitle ?? null,
    workspaceName: hints.workspaceName ?? null,
    tags: hints.tags ?? [],
    headingPath: hints.headingPath ?? [],
    glossaryTerms: hints.glossaryTerms ?? [],
  };
}

/** Build session context hints from the open page and workspace. */
export function voiceHintsFromPage(input: {
  documentPath: string;
  pageTitle: string;
  workspaceName?: string | null;
  rawContent?: string;
}): VoiceSessionContextHints {
  const hints: VoiceSessionContextHints = {
    documentId: input.documentPath,
    documentPath: input.documentPath,
    pageTitle: input.pageTitle,
    workspaceName: input.workspaceName ?? null,
  };

  if (!input.rawContent) {
    return hints;
  }

  const { frontmatter } = splitFrontmatter(input.rawContent);
  if (!frontmatter) {
    return hints;
  }

  const yamlBody = frontmatter
    .replace(/^---\r?\n/, "")
    .replace(/\r?\n---[ \t]*\r?\n?$/, "");
  try {
    const parsed = parseYaml(yamlBody) as { tags?: unknown; title?: unknown };
    if (typeof parsed?.title === "string" && parsed.title.trim()) {
      hints.pageTitle = parsed.title.trim();
    }
    if (Array.isArray(parsed?.tags)) {
      hints.tags = parsed.tags.filter(
        (tag): tag is string => typeof tag === "string" && tag.trim().length > 0,
      );
    }
  } catch {
    // Malformed frontmatter should not block dictation.
  }

  return hints;
}

export async function startVoiceSession(
  hints?: VoiceSessionContextHints,
): Promise<VoiceSessionStart> {
  return invoke<VoiceSessionStart>("voice_start_session", {
    hints: hints ? serializeVoiceHints(hints) : null,
  });
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
