import { z } from "zod";

/**
 * Minimal chat message envelope for `start_run`. Full AI SDK `UIMessage` JSON is
 * accepted via `z.unknown()` so callers can pass provider-specific shapes.
 */
export const uiMessageSchema = z
  .object({
    id: z.string(),
    role: z.enum(["user", "assistant", "system"]),
  })
  .passthrough();

export type UiMessage = z.infer<typeof uiMessageSchema>;

/**
 * Opaque AI SDK `UIMessageChunk` payload carried inside `message_chunk` events.
 */
export const uiMessageChunkSchema = z.unknown();

export type UiMessageChunk = z.infer<typeof uiMessageChunkSchema>;
