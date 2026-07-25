import { z } from "zod";

import { MAX_OVERLAY_ANCHORS, workspaceAnchorSchema } from "./anchors";
import { uiMessageChunkSchema } from "./messages";
import { providerKindSchema } from "./provider";
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
  /** Effective provider for this run (`pioneer` | `openai` | `fake`). */
  provider: providerKindSchema.optional(),
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

export const agentStepKindSchema = z.enum([
  "model",
  "tool",
  "search",
  "navigation",
  "draft",
  "execution",
  "validation",
  "proposal",
]);

export const overlayPurposeSchema = z.enum([
  "attention",
  "evidence",
  "warning",
  "change",
]);

export const stepStartedEventSchema = z.object({
  type: z.literal("step_started"),
  runId: z.string(),
  stepId: z.string(),
  kind: agentStepKindSchema,
  label: z.string(),
});

export const stepCompletedEventSchema = z.object({
  type: z.literal("step_completed"),
  runId: z.string(),
  stepId: z.string(),
  durationMs: z.number().nonnegative(),
  summary: z.string().optional(),
});

export const evidenceAddedEventSchema = z.object({
  type: z.literal("evidence_added"),
  runId: z.string(),
  evidenceId: z.string(),
  resourceId: z.string(),
  path: z.string(),
  revision: z.string().min(1).optional(),
  excerpt: z.string(),
  anchor: workspaceAnchorSchema.optional(),
  score: z.number().optional(),
});

export const overlayShowEventSchema = z.object({
  type: z.literal("overlay_show"),
  runId: z.string(),
  overlayId: z.string(),
  anchors: z
    .array(workspaceAnchorSchema)
    .min(1)
    .max(MAX_OVERLAY_ANCHORS),
  purpose: overlayPurposeSchema,
  commentary: z.string().optional(),
});

export const overlayClearEventSchema = z.object({
  type: z.literal("overlay_clear"),
  runId: z.string(),
  overlayId: z.string().optional(),
});

export const agentEventSchema = z.discriminatedUnion("type", [
  helloAckEventSchema,
  runStartedEventSchema,
  messageChunkEventSchema,
  runCompletedEventSchema,
  runFailedEventSchema,
  healthEventSchema,
  stepStartedEventSchema,
  stepCompletedEventSchema,
  evidenceAddedEventSchema,
  overlayShowEventSchema,
  overlayClearEventSchema,
]);

export type HelloAckEvent = z.infer<typeof helloAckEventSchema>;
export type RunStartedEvent = z.infer<typeof runStartedEventSchema>;
export type MessageChunkEvent = z.infer<typeof messageChunkEventSchema>;
export type RunCompletedEvent = z.infer<typeof runCompletedEventSchema>;
export type RunFailedEvent = z.infer<typeof runFailedEventSchema>;
export type HealthEvent = z.infer<typeof healthEventSchema>;
export type AgentStepKind = z.infer<typeof agentStepKindSchema>;
export type OverlayPurpose = z.infer<typeof overlayPurposeSchema>;
export type StepStartedEvent = z.infer<typeof stepStartedEventSchema>;
export type StepCompletedEvent = z.infer<typeof stepCompletedEventSchema>;
export type EvidenceAddedEvent = z.infer<typeof evidenceAddedEventSchema>;
export type OverlayShowEvent = z.infer<typeof overlayShowEventSchema>;
export type OverlayClearEvent = z.infer<typeof overlayClearEventSchema>;
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
