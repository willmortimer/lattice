import { z } from "zod";

export const providerKindSchema = z.enum(["pioneer", "openai", "fake"]);

export type ProviderKind = z.infer<typeof providerKindSchema>;
