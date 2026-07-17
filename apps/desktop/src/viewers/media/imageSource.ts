import { inspectResource, readResourceRange, type ResourceInspection, type ResourceLocation } from "../../lib/resourceRuntime";
import { assertEncodedLimit, assertDecodedPixelLimit, MAX_IMAGE_ENCODED_BYTES } from "./mediaLimits";
import { readImageDimensions, type ImageDimensions } from "./imageMetadata";

export const IMAGE_READ_CHUNK_SIZE = 256 * 1024;

export interface ObjectUrlApi {
  createObjectURL: (blob: Blob) => string;
  revokeObjectURL: (url: string) => void;
}

export interface ObjectUrlLease {
  url: string;
  revoke: () => void;
}

export function createObjectUrlLease(blob: Blob, api: ObjectUrlApi = URL): ObjectUrlLease {
  const url = api.createObjectURL(blob);
  let revoked = false;
  return {
    url,
    revoke: () => {
      if (revoked) return;
      revoked = true;
      api.revokeObjectURL(url);
    },
  };
}

export interface ImageAsset {
  lease: ObjectUrlLease;
  dimensions: ImageDimensions | null;
  encodedBytes: number;
  mimeType: string;
}

export interface ImageSourceRuntime {
  inspect: (location: ResourceLocation, signal: AbortSignal) => Promise<ResourceInspection>;
  readRange: (range: { root: string; path: string; offset: number; length: number }, signal: AbortSignal) => Promise<Uint8Array>;
  objectUrls?: ObjectUrlApi;
}

function mimeType(path: string, data: Uint8Array): string {
  const extension = path.split(".").pop()?.toLowerCase();
  if (extension === "svg") return "image/svg+xml";
  if (extension === "png") return "image/png";
  if (extension === "jpg" || extension === "jpeg") return "image/jpeg";
  if (extension === "gif") return "image/gif";
  if (extension === "webp") return "image/webp";
  if (data[0] === 0x89 && data[1] === 0x50) return "image/png";
  if (data[0] === 0xff && data[1] === 0xd8) return "image/jpeg";
  return "application/octet-stream";
}

async function readBoundedBytes(
  location: ResourceLocation,
  size: number,
  runtime: ImageSourceRuntime,
  signal: AbortSignal,
): Promise<Uint8Array> {
  const result = new Uint8Array(size);
  for (let offset = 0; offset < size; offset += IMAGE_READ_CHUNK_SIZE) {
    if (signal.aborted) throw new DOMException("Image read was cancelled", "AbortError");
    const length = Math.min(IMAGE_READ_CHUNK_SIZE, size - offset);
    const chunk = await runtime.readRange({ root: location.root, path: location.path, offset, length }, signal);
    if (chunk.byteLength !== length) throw new Error("The image range ended before its reported size.");
    result.set(chunk, offset);
  }
  return result;
}

export async function loadImageAsset(
  location: ResourceLocation,
  signal: AbortSignal,
  runtime: ImageSourceRuntime = { inspect: inspectResource, readRange: readResourceRange },
): Promise<ImageAsset> {
  const inspection = await runtime.inspect(location, signal);
  assertEncodedLimit(inspection.size, MAX_IMAGE_ENCODED_BYTES, "Image");
  const bytes = await readBoundedBytes(location, inspection.size, runtime, signal);
  const dimensions = readImageDimensions(bytes);
  if (dimensions?.width && dimensions.height) assertDecodedPixelLimit(dimensions.width, dimensions.height);
  const blobBytes = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
  const lease = createObjectUrlLease(new Blob([blobBytes], { type: mimeType(location.path, bytes) }), runtime.objectUrls ?? URL);
  return { lease, dimensions, encodedBytes: inspection.size, mimeType: mimeType(location.path, bytes) };
}
