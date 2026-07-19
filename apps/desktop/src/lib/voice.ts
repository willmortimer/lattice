import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { invoke } from "./ipc";

export type VoiceStatus = {
  available: boolean;
  prepared: boolean;
  preparing: boolean;
  listening: boolean;
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

const TARGET_RATE = 16_000;
const CHUNK_SAMPLES = 2_560; // 160 ms @ 16 kHz

export async function getVoiceStatus(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_status");
}

export async function prepareVoiceModel(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_prepare");
}

export async function startVoiceSession(): Promise<VoiceSessionStart> {
  return invoke<VoiceSessionStart>("voice_start_session");
}

export async function pushVoiceAudio(sessionId: string, samples: Float32Array): Promise<void> {
  await invoke("voice_push_audio", {
    sessionId,
    samples: Array.from(samples),
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

function downsampleTo16k(input: Float32Array, inputRate: number): Float32Array {
  if (inputRate === TARGET_RATE) return input;
  if (inputRate <= 0) return new Float32Array(0);
  const ratio = inputRate / TARGET_RATE;
  const outLength = Math.max(1, Math.floor(input.length / ratio));
  const output = new Float32Array(outLength);
  for (let i = 0; i < outLength; i += 1) {
    const start = Math.floor(i * ratio);
    const end = Math.min(input.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    let count = 0;
    for (let j = start; j < end; j += 1) {
      sum += input[j] ?? 0;
      count += 1;
    }
    output[i] = count > 0 ? sum / count : (input[start] ?? 0);
  }
  return output;
}

function mixToMono(channelData: Float32Array[]): Float32Array {
  if (channelData.length === 0) return new Float32Array(0);
  if (channelData.length === 1) return channelData[0] ?? new Float32Array(0);
  const length = channelData[0]?.length ?? 0;
  const mono = new Float32Array(length);
  for (let i = 0; i < length; i += 1) {
    let sum = 0;
    for (const channel of channelData) {
      sum += channel[i] ?? 0;
    }
    mono[i] = sum / channelData.length;
  }
  return mono;
}

/**
 * Hold-to-talk microphone capture. Requests permission on first use,
 * streams Float32 mono @ 16 kHz to the Rust voice session.
 */
export class DictationCapture {
  private media: MediaStream | null = null;
  private context: AudioContext | null = null;
  private processor: ScriptProcessorNode | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private pending: number[] = [];
  private sessionId: string | null = null;
  private pushing = false;

  get active(): boolean {
    return this.sessionId !== null;
  }

  async start(sessionId: string): Promise<void> {
    await this.stopMediaOnly();
    this.sessionId = sessionId;
    this.pending = [];

    this.media = await navigator.mediaDevices.getUserMedia({
      audio: {
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
      video: false,
    });

    const context = new AudioContext();
    this.context = context;
    if (context.state === "suspended") {
      await context.resume();
    }

    const source = context.createMediaStreamSource(this.media);
    this.source = source;
    // ScriptProcessor is deprecated but widely available in Tauri WebView without
    // shipping a separate AudioWorklet module for M2.
    const processor = context.createScriptProcessor(4096, source.channelCount || 1, 1);
    this.processor = processor;

    processor.onaudioprocess = (event) => {
      if (!this.sessionId) return;
      const channels: Float32Array[] = [];
      for (let c = 0; c < event.inputBuffer.numberOfChannels; c += 1) {
        channels.push(event.inputBuffer.getChannelData(c).slice());
      }
      const mono = mixToMono(channels);
      const down = downsampleTo16k(mono, context.sampleRate);
      for (let i = 0; i < down.length; i += 1) {
        this.pending.push(down[i] ?? 0);
      }
      void this.flushChunks();
    };

    source.connect(processor);
    const mute = context.createGain();
    mute.gain.value = 0;
    processor.connect(mute);
    mute.connect(context.destination);
  }

  private async flushChunks(): Promise<void> {
    if (this.pushing || !this.sessionId) return;
    this.pushing = true;
    try {
      while (this.pending.length >= CHUNK_SAMPLES && this.sessionId) {
        const slice = this.pending.splice(0, CHUNK_SAMPLES);
        await pushVoiceAudio(this.sessionId, Float32Array.from(slice));
      }
    } finally {
      this.pushing = false;
    }
  }

  async stopAndFinish(sessionId = this.sessionId): Promise<void> {
    if (!sessionId) return;
    // Flush remaining audio before finish.
    if (this.pending.length > 0 && this.sessionId === sessionId) {
      const rest = this.pending.splice(0, this.pending.length);
      await pushVoiceAudio(sessionId, Float32Array.from(rest));
    }
    await this.stopMediaOnly();
    this.sessionId = null;
    await finishVoiceSession(sessionId);
  }

  async cancel(sessionId = this.sessionId): Promise<void> {
    await this.stopMediaOnly();
    this.sessionId = null;
    this.pending = [];
    if (sessionId) {
      await cancelVoiceSession(sessionId);
    } else {
      await cancelActiveVoiceSession();
    }
  }

  private async stopMediaOnly(): Promise<void> {
    this.processor?.disconnect();
    this.source?.disconnect();
    this.processor = null;
    this.source = null;
    if (this.context) {
      await this.context.close().catch(() => undefined);
      this.context = null;
    }
    this.media?.getTracks().forEach((track) => track.stop());
    this.media = null;
  }
}
