import { z } from "zod";

import { uiMessageSchema } from "./messages";
import { providerKindSchema } from "./provider";
import { PROTOCOL_VERSION } from "./version";

const protocolVersionSchema = z.literal(PROTOCOL_VERSION);

export const helloCommandSchema = z.object({
  type: z.literal("hello"),
  protocolVersion: protocolVersionSchema,
});

const startRunCommandBaseSchema = z.object({
  type: z.literal("start_run"),
  threadId: z.string(),
  runId: z.string(),
  provider: providerKindSchema,
  model: z.string(),
  messages: z.array(uiMessageSchema).optional(),
  prompt: z.string().optional(),
});

export const startRunCommandSchema = startRunCommandBaseSchema.refine(
  (value) => value.messages !== undefined || value.prompt !== undefined,
  { message: "start_run requires messages or prompt" },
);

export const cancelRunCommandSchema = z.object({
  type: z.literal("cancel_run"),
  runId: z.string(),
});

export const healthCommandSchema = z.object({
  type: z.literal("health"),
});

export const shutdownCommandSchema = z.object({
  type: z.literal("shutdown"),
});

export const agentCommandSchema = z
  .discriminatedUnion("type", [
    helloCommandSchema,
    startRunCommandBaseSchema,
    cancelRunCommandSchema,
    healthCommandSchema,
    shutdownCommandSchema,
  ])
  .superRefine((value, ctx) => {
    if (
      value.type === "start_run" &&
      value.messages === undefined &&
      value.prompt === undefined
    ) {
      ctx.addIssue({
        code: "custom",
        message: "start_run requires messages or prompt",
        path: ["messages"],
      });
    }
  });

export type HelloCommand = z.infer<typeof helloCommandSchema>;
export type StartRunCommand = z.infer<typeof startRunCommandSchema>;
export type CancelRunCommand = z.infer<typeof cancelRunCommandSchema>;
export type HealthCommand = z.infer<typeof healthCommandSchema>;
export type ShutdownCommand = z.infer<typeof shutdownCommandSchema>;
export type AgentCommand = z.infer<typeof agentCommandSchema>;

export function parseCommand(line: string): AgentCommand {
  const trimmed = line.trim();
  if (trimmed.length === 0) {
    throw new Error("command line is empty");
  }

  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    throw new Error("command line is not valid JSON");
  }

  return agentCommandSchema.parse(value);
}

export function serializeCommand(command: AgentCommand): string {
  return JSON.stringify(command);
}
