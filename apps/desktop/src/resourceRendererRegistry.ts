import type { ComponentType } from "react";
import type { Resource, ResourceKind } from "./types";

export type RendererSurface = "main" | "embed" | "inspect";
export type RendererKey = ResourceKind | "*";
export type RendererTarget = ResourceKind | "*";

export interface ResourceRendererProps<TContext, TSession> {
  context: TContext;
  session: TSession;
}

export type ResourceRendererComponent<TContext, TSession> = ComponentType<
  ResourceRendererProps<TContext, TSession>
>;

export type ResourceRendererLoad<TContext, TSession> = (
  signal: AbortSignal,
) => Promise<ResourceRendererComponent<TContext, TSession>>;

export interface LazyRendererView<TContext, TSession> {
  load: ResourceRendererLoad<TContext, TSession>;
}

export interface ResourceRendererLifecyclePolicy {
  active: "mount" | "reuse";
  inactive: "suspend" | "unmount";
  cache: "none" | "module" | "instance";
}

export interface ResourceRendererDefinition<TContext, TSession> {
  id: string;
  /** Kind fallback. Prefer formatIds when a renderer is format-specific. */
  kind?: RendererTarget | readonly ResourceKind[];
  /** Stable IDs such as `file:image`, `file:pdf`, or a native format ID. */
  formatIds?: readonly string[];
  surfaces?: readonly RendererSurface[];
  profiles?: readonly string[];
  priority?: number;
  capabilities?: readonly string[];
  load: ResourceRendererLoad<TContext, TSession>;
  lifecycle?: Partial<ResourceRendererLifecyclePolicy>;
  inspect?: LazyRendererView<TContext, TSession>;
  embed?: LazyRendererView<TContext, TSession>;
}

export interface RendererResolution<TContext, TSession> {
  definition: ResourceRendererDefinition<TContext, TSession>;
  mode: "native" | "capability-fallback" | "unknown-fallback";
  missingCapabilities: readonly string[];
  formatId: string;
}

export interface ResourceRendererRegistryOptions<TContext, TSession> {
  capabilityFallback: ResourceRendererDefinition<TContext, TSession>;
  unknownFallback: ResourceRendererDefinition<TContext, TSession>;
}

const DEFAULT_LIFECYCLE: ResourceRendererLifecyclePolicy = {
  active: "mount",
  inactive: "unmount",
  cache: "module",
};

function missingCapabilities<TContext, TSession>(
  definition: ResourceRendererDefinition<TContext, TSession>,
  capabilities: ReadonlySet<string>,
): string[] {
  return (definition.capabilities ?? []).filter((capability) => !capabilities.has(capability));
}

/**
 * Small deterministic registry used by the shell. Format-ID registrations
 * score ahead of kind fallbacks; profile and surface constraints are applied
 * before priority and registration order break ties. Capability absence is
 * resolved explicitly instead of silently falling through to an arbitrary
 * renderer.
 */
export class ResourceRendererRegistry<TContext, TSession> {
  private readonly definitions: Array<{
    definition: ResourceRendererDefinition<TContext, TSession>;
    order: number;
  }> = [];

  private readonly capabilityFallback: ResourceRendererDefinition<TContext, TSession>;

  private readonly unknownFallback: ResourceRendererDefinition<TContext, TSession>;

  private registrationOrder = 0;

  constructor(options: ResourceRendererRegistryOptions<TContext, TSession>) {
    this.capabilityFallback = options.capabilityFallback;
    this.unknownFallback = options.unknownFallback;
    this.assertSpecialDefinition(options.capabilityFallback, "capability-fallback");
    this.assertSpecialDefinition(options.unknownFallback, "unknown-fallback");
  }

  register(definition: ResourceRendererDefinition<TContext, TSession>): this {
    const keys = this.definitionKeys(definition);
    if (keys.length === 0) {
      throw new Error(`Resource renderer ${definition.id} must declare a kind or formatId`);
    }
    const surfaces = definition.surfaces ?? ["main"];
    for (const current of this.definitions) {
      const currentKeys = this.definitionKeys(current.definition);
      const overlaps = keys.some((key) => currentKeys.includes(key));
      const sharedSurface = surfaces.some((surface) =>
        (current.definition.surfaces ?? ["main"]).includes(surface),
      );
      const sharedProfile =
        !definition.profiles ||
        !current.definition.profiles ||
        definition.profiles.some((profile) => current.definition.profiles?.includes(profile));
      if (overlaps && sharedSurface && sharedProfile) {
        throw new Error(`Duplicate resource renderer target for ${keys.join(", ")}: ${definition.id}`);
      }
    }
    if (this.definitions.some((entry) => entry.definition.id === definition.id)) {
      throw new Error(`Duplicate resource renderer id: ${definition.id}`);
    }
    this.definitions.push({
      definition: {
        ...definition,
        surfaces,
        lifecycle: { ...DEFAULT_LIFECYCLE, ...definition.lifecycle },
      },
      order: this.registrationOrder++,
    });
    return this;
  }

  resolve(
    resourceOrKind: Resource | ResourceKind,
    capabilities: Iterable<string> = [],
    profile?: string,
    surface: RendererSurface = "main",
  ): RendererResolution<TContext, TSession> {
    const resource: Resource =
      typeof resourceOrKind === "string" ? { path: "", kind: resourceOrKind } : resourceOrKind;
    const formatId = resource.formatId ?? deriveResourceFormatId(resource);
    const candidates = this.definitions
      .map(({ definition, order }) => ({
        definition,
        order,
        score: this.matchScore(definition, resource, formatId, profile, surface),
      }))
      .filter((candidate) => candidate.score >= 0)
      .sort((left, right) => right.score - left.score || left.order - right.order);
    const selected = candidates[0]?.definition;
    if (!selected) {
      return {
        definition: this.unknownFallback,
        mode: "unknown-fallback",
        missingCapabilities: [],
        formatId,
      };
    }

    const missing = missingCapabilities(selected, new Set(capabilities));
    if (missing.length > 0) {
      return {
        definition: this.capabilityFallback,
        mode: "capability-fallback",
        missingCapabilities: missing,
        formatId,
      };
    }

    return { definition: selected, mode: "native", missingCapabilities: [], formatId };
  }

  entries(): readonly ResourceRendererDefinition<TContext, TSession>[] {
    return this.definitions.map((entry) => entry.definition);
  }

  private definitionKeys(definition: ResourceRendererDefinition<TContext, TSession>): string[] {
    const kindKeys = definition.kind
      ? (Array.isArray(definition.kind) ? definition.kind : [definition.kind])
      : [];
    return [...kindKeys, ...(definition.formatIds ?? [])];
  }

  private matchScore(
    definition: ResourceRendererDefinition<TContext, TSession>,
    resource: Resource,
    formatId: string,
    profile: string | undefined,
    surface: RendererSurface,
  ): number {
    if (!(definition.surfaces ?? ["main"]).includes(surface)) return -1;
    if (definition.profiles && (!profile || !definition.profiles.includes(profile))) return -1;
    const formatMatch = definition.formatIds?.includes(formatId) ? 1000 : -1;
    const kindMatch =
      definition.kind === "*" ||
      (Array.isArray(definition.kind) && definition.kind.includes(resource.kind)) ||
      definition.kind === resource.kind
        ? 500
        : -1;
    const match = Math.max(formatMatch, kindMatch);
    if (match < 0) return -1;
    return match + (definition.profiles?.includes(profile ?? "") ? 100 : 0) + (definition.priority ?? 0);
  }

  private assertSpecialDefinition(
    definition: ResourceRendererDefinition<TContext, TSession>,
    name: string,
  ): void {
    if (!definition.id || !definition.load) {
      throw new Error(`Invalid ${name} resource renderer definition`);
    }
  }
}

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "tiff"]);
const CODE_EXTENSIONS = new Set(["js", "jsx", "ts", "tsx", "rs", "py", "go", "java", "c", "cpp", "h", "css", "html", "sql", "sh"]);

/** Stable browser/native-independent format IDs for ordinary files. */
export function deriveResourceFormatId(resource: Resource): string {
  if (resource.formatId) return resource.formatId;
  if (resource.kind !== "file") return resource.kind;
  const extension = resource.path.split(".").pop()?.toLowerCase() ?? "";
  if (extension === "svg" || IMAGE_EXTENSIONS.has(extension)) return "file:image";
  if (extension === "pdf") return "file:pdf";
  if (["txt", "md", "markdown", "log"].includes(extension)) return "file:text";
  if (CODE_EXTENSIONS.has(extension)) return "file:code";
  if (extension === "json") return "file:json";
  if (["yaml", "yml"].includes(extension)) return "file:yaml";
  return "file:unknown";
}

/** Load helper kept separate from React so cancellation and stale-load tests
 * can exercise the contract without a browser renderer. */
export async function loadResourceRenderer<TContext, TSession>(
  definition: ResourceRendererDefinition<TContext, TSession>,
  signal: AbortSignal,
): Promise<ResourceRendererComponent<TContext, TSession>> {
  if (signal.aborted) throw createAbortError();
  const component = await definition.load(signal);
  if (signal.aborted) throw createAbortError();
  return component;
}

export function createAbortError(): DOMException {
  return new DOMException("Resource renderer load was cancelled", "AbortError");
}
