import { assertDecodedPixelLimit } from "./mediaLimits";

export interface ImageDimensions {
  width: number;
  height: number;
  source: "png" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "browser" | "unknown";
}

function uint16(data: Uint8Array, offset: number): number {
  return data[offset] | (data[offset + 1] << 8);
}

function uint32(data: Uint8Array, offset: number): number {
  return ((data[offset] * 0x1000000) + ((data[offset + 1] << 16) | (data[offset + 2] << 8) | data[offset + 3])) >>> 0;
}

function uint32Le(data: Uint8Array, offset: number): number {
  return (data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] * 0x1000000)) >>> 0;
}

function uint24(data: Uint8Array, offset: number): number {
  return data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16);
}

function jpegDimensions(data: Uint8Array): ImageDimensions | null {
  if (data.length < 4 || data[0] !== 0xff || data[1] !== 0xd8) return null;
  let offset = 2;
  while (offset + 3 < data.length) {
    if (data[offset] !== 0xff) {
      offset += 1;
      continue;
    }
    while (data[offset] === 0xff) offset += 1;
    const marker = data[offset++];
    if (marker === 0xd8 || marker === 0xd9) continue;
    if (marker === 0xda) break;
    if (offset + 1 >= data.length) break;
    const segmentLength = (data[offset] << 8) | data[offset + 1];
    if (segmentLength < 2 || offset + segmentLength > data.length) break;
    const isFrame = (marker >= 0xc0 && marker <= 0xc3) || (marker >= 0xc5 && marker <= 0xc7) ||
      (marker >= 0xc9 && marker <= 0xcb) || (marker >= 0xcd && marker <= 0xcf);
    if (isFrame && segmentLength >= 7) {
      return { width: (data[offset + 5] << 8) | data[offset + 6], height: (data[offset + 3] << 8) | data[offset + 4], source: "jpeg" };
    }
    offset += segmentLength;
  }
  return null;
}

function svgDimensions(data: Uint8Array): ImageDimensions | null {
  const text = new TextDecoder().decode(data.subarray(0, Math.min(data.length, 128 * 1024)));
  if (!/<svg(?:\s|>)/i.test(text)) return null;
  const width = /\bwidth=["']\s*([\d.]+)\s*(?:px)?["']/i.exec(text);
  const height = /\bheight=["']\s*([\d.]+)\s*(?:px)?["']/i.exec(text);
  if (width && height) {
    return { width: Math.round(Number(width[1])), height: Math.round(Number(height[1])), source: "svg" };
  }
  const viewBox = /\bviewBox=["']\s*[-\d.]+\s+[-\d.]+\s+([\d.]+)\s+([\d.]+)["']/i.exec(text);
  if (viewBox) {
    return { width: Math.round(Number(viewBox[1])), height: Math.round(Number(viewBox[2])), source: "svg" };
  }
  return { width: 0, height: 0, source: "svg" };
}

/** Read only container metadata; never parses SVG as HTML or executes it. */
export function readImageDimensions(data: Uint8Array): ImageDimensions | null {
  let dimensions: ImageDimensions | null = null;
  if (data.length >= 24 && data[0] === 0x89 && data[1] === 0x50 && data[2] === 0x4e && data[3] === 0x47) {
    dimensions = { width: uint32(data, 16), height: uint32(data, 20), source: "png" };
  } else if (data.length >= 10 && (data[0] === 0x47 && data[1] === 0x49 && data[2] === 0x46)) {
    dimensions = { width: uint16(data, 6), height: uint16(data, 8), source: "gif" };
  } else if (data.length >= 30 && data[0] === 0x52 && data[1] === 0x49 && data[2] === 0x46 && data[3] === 0x46 && data[8] === 0x57 && data[9] === 0x45 && data[10] === 0x42 && data[11] === 0x50) {
    if (data[12] === 0x56 && data[13] === 0x50 && data[14] === 0x38 && data[15] === 0x58 && data.length >= 30) {
      dimensions = { width: 1 + uint24(data, 24), height: 1 + uint24(data, 27), source: "webp" };
    }
  } else if (data.length >= 26 && data[0] === 0x42 && data[1] === 0x4d) {
    dimensions = { width: uint32Le(data, 18), height: uint32Le(data, 22), source: "bmp" };
  } else {
    dimensions = jpegDimensions(data) ?? svgDimensions(data);
  }
  if (dimensions && dimensions.width > 0 && dimensions.height > 0) {
    assertDecodedPixelLimit(dimensions.width, dimensions.height);
  }
  return dimensions;
}
