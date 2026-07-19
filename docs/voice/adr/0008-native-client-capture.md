# ADR 0008: Native client capture, not WebView PCM

## Status

Accepted (voice subsystem). Supersedes the WebView-capture reading of
[ADR 0004](0004-client-owned-audio-capture.md) for macOS production.

## Context

ADR 0004 correctly requires client-owned microphone access (not `latticed`).
The M2 prototype implemented that via WebView `getUserMedia`,
`ScriptProcessorNode`, and JavaScript resampling, then shipped PCM as JSON
`number[]` arrays over Tauri. That path clips first phonemes (no pre-roll),
uses a crude box-filter resampler, and burns CPU on representation conversion.

## Decision

On macOS, **client-owned capture** means a **trusted native desktop component**
(`AVAudioEngine` + `AVAudioConverter`) owned by the Tauri/Rust shell (or a
small native helper), not the WebView:

- Convert to 16 kHz mono Float32 in native code.
- Maintain ~250–500 ms pre-roll while dictation is armed.
- Stream packed binary frames with monotonic timestamps and sequence numbers.
- Bound queues and surface gap/drop events.

The WebView retains session intent and presentation only. `latticed` still does
not open the microphone ([ADR 0004](0004-client-owned-audio-capture.md)).

## Consequences

- New crates: `lattice-audio` / `lattice-audio-macos` (or equivalent).
- Retire production dependence on `ScriptProcessorNode` and JSON PCM arrays.
- Binary audio IPC aligns with [ADR 0041](../../decisions/0041-daemon-ipc-protobuf.md)
  bulk-plane guidance.
