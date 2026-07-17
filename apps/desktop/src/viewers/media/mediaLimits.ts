export const MAX_IMAGE_ENCODED_BYTES = 64 * 1024 * 1024;
export const MAX_IMAGE_DECODED_PIXELS = 100_000_000;

// PDF.js is range-backed, but a corrupt or unusually large document can still
// retain substantial parser state. Keep this limit explicit and visible.
export const MAX_PDF_ENCODED_BYTES = 64 * 1024 * 1024;
export const PDF_RANGE_CHUNK_SIZE = 256 * 1024;
export const MAX_RENDERED_PDF_CANVASES = 3;

export class MediaLimitError extends Error {
  readonly limit: number;
  readonly actual: number;

  constructor(message: string, limit: number, actual: number) {
    super(message);
    this.name = "MediaLimitError";
    this.limit = limit;
    this.actual = actual;
  }
}

export function assertEncodedLimit(size: number, limit: number, label: string): void {
  if (!Number.isSafeInteger(size) || size < 0) {
    throw new Error(`${label} reported an invalid encoded size.`);
  }
  if (size > limit) {
    throw new MediaLimitError(
      `${label} is too large to preview (${formatBytes(size)}; limit ${formatBytes(limit)}).`,
      limit,
      size,
    );
  }
}

export function assertDecodedPixelLimit(width: number, height: number): void {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0) return;
  const pixels = width * height;
  if (pixels > MAX_IMAGE_DECODED_PIXELS) {
    throw new MediaLimitError(
      `This image is too large to decode safely (${formatInteger(pixels)} pixels; limit ${formatInteger(MAX_IMAGE_DECODED_PIXELS)}).`,
      MAX_IMAGE_DECODED_PIXELS,
      pixels,
    );
  }
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MiB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GiB`;
}

function formatInteger(value: number): string {
  return value.toLocaleString("en-US");
}
