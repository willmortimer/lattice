# ADR 0004: Client-owned audio capture

## Status

Accepted (voice subsystem).

## Context

Microphone permission, device selection, and CoreAudio integration sit naturally
beside the UI. A daemon that opens the microphone complicates permissions and
sandboxing.

## Decision

The **trusted native desktop client** (or Quick Note helper) owns microphone
access via platform capture (`AVAudioEngine` on macOS) and sends normalized
PCM to the voice service / `latticed`. The daemon does not access audio
hardware. “Client-owned” does **not** mean WebView/`getUserMedia` for
production — see [ADR 0008](0008-native-client-capture.md).

## Consequences

- The daemon does not require microphone permission.
- Platform-specific capture stays near the user interface.
- Local IPC must support sustained audio streaming.
- See [audio-capture.md](../audio-capture.md) and
  [macos-integration.md](../macos-integration.md).
