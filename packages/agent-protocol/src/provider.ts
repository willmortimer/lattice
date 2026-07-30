import { z } from "zod";

export const providerKindSchema = z.enum(["pioneer", "openai", "local", "fake"]);

export type ProviderKind = z.infer<typeof providerKindSchema>;
