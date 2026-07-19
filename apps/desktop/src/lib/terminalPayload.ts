/** Decode PTY output bytes from the Tauri event payload. */
export function decodeTerminalOutput(data: number[]): Uint8Array {
  return Uint8Array.from(data);
}
