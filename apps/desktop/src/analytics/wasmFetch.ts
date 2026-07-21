/** Fetch a `.wasm` URL and reject HTML/empty responses before `WebAssembly.compile`. */
export async function fetchWasmBytes(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch WASM (${response.status}): ${url}`);
  }
  const buffer = await response.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  // `\0asm` magic — byte-0 compile errors usually mean HTML/404 was served instead.
  if (
    bytes.byteLength < 8 ||
    bytes[0] !== 0x00 ||
    bytes[1] !== 0x61 ||
    bytes[2] !== 0x73 ||
    bytes[3] !== 0x6d
  ) {
    throw new Error(
      `Invalid WASM at ${url} (${bytes.byteLength} bytes). Packaged CSP must allow same-origin connect-src.`,
    );
  }
  return buffer;
}
