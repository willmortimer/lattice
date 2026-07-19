import { parseDictationSegments } from "../editor/dictationFinal";

function segmentsToPlainText(segments: ReturnType<typeof parseDictationSegments>): string {
  let out = "";
  for (const segment of segments) {
    if (segment.kind === "text") {
      out += segment.value;
    } else if (segment.kind === "newline") {
      out += "\n";
    } else {
      out += "\n\n";
    }
  }
  return out;
}

function needsSpaceBetween(left: string, right: string): boolean {
  if (!left || !right) return false;
  const leftChar = left[left.length - 1]!;
  const rightChar = right[0]!;
  if (/\s/.test(leftChar) || /\s/.test(rightChar)) return false;
  if (/[-([{]/.test(leftChar)) return false;
  if (/^[,.;:!?)]/.test(rightChar)) return false;
  return true;
}

function joinTextParts(parts: string[]): string {
  let result = "";
  for (const part of parts) {
    if (!part) continue;
    if (result && needsSpaceBetween(result, part)) {
      result += " ";
    }
    result += part;
  }
  return result;
}

/**
 * Insert a final voice transcript into plain Markdown at `insertAt`.
 * Voice structure markers (`new line`, `new paragraph`) become plain newlines.
 */
export function mergeDictationPlainText(
  content: string,
  finalText: string,
  insertAt: number,
): string {
  const insertion = segmentsToPlainText(parseDictationSegments(finalText));
  if (!insertion) return content;

  const at = Math.min(Math.max(0, insertAt), content.length);
  const before = content.slice(0, at);
  const after = content.slice(at);
  return joinTextParts([before, insertion, after]);
}
