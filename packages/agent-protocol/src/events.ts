import { z } from "zod";

import { uiMessageChunkSchema } from "./messages";
import { PROTOCOL_VERSION } from "./version";

const protocolVersionSchema = z.literal(PROTOCOL_VERSION);

export const helloAckEventSchema = z.object({
  type: z.literal("hello_ack"),
  protocolVersion: protocolVersionSchema,
});

export const runStartedEventSchema = z.object({
  type: z.literal("run_started"),
  runId: z.string(),
  threadId: z.string(),
});

export const messageChunkEventSchema = z.object({
  type: z.literal("message_chunk"),
  runId: z.string(),
  chunk: uiMessageChunkSchema,
});

export const runCompletedEventSchema = z.object({
  type: z.literal("run_completed"),
  runId: z.string(),
});

export const runFailedEventSchema = z.object({
  type: z.literal("run_failed"),
  runId: z.string(),
  message: z.string(),
  retryable: z.boolean(),
});

export const healthEventSchema = z.object({
  type: z.literal("health"),
  ok: z.boolean(),
});

export const agentEventSchema = z.discriminatedUnion("type", [
  helloAckEventSchema,
  runStartedEventSchema,
  messageChunkEventSchema,
  runCompletedEventSchema,
  runFailedEventSchema,
  healthEventSchema,
]);

export type HelloAckEvent = z.infer<typeof helloAckEventSchema>;
export type RunStartedEvent = z.infer<typeof runStartedEventSchema>;
export type MessageChunkEvent = z.infer<typeof messageChunkEventSchema>;
export type RunCompletedEvent = z.infer<typeof runCompletedEventSchema>;
export type RunFailedEvent = z.infer<typeof runFailedEventSchema>;
export type HealthEvent = z.infer<typeof healthEventSchema>;
export type AgentEvent = z.infer<typeof agentEventSchema>;

export function parseEvent(line: string): AgentEvent {
  const trimmed = line.trim();
  if (trimmed.length === 0) {
    throw new Error("event line is empty");
  }

  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    throw new Error("event line is not valid JSON");
  }

  return agentEventSchema.parse(value);
}

export function serializeEvent(event: AgentEvent): string {
  return JSON.stringify(event);
}
