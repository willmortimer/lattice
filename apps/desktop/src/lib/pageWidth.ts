export type PageWidth = "standard" | "wide" | "full";

export const PAGE_WIDTHS = ["standard", "wide", "full"] as const satisfies readonly PageWidth[];

/** Coerce unknown persisted values to a supported page width. */
export function normalizePageWidth(value: unknown): PageWidth {
  if (value === "standard" || value === "wide" || value === "full") return value;
  return "standard";
}
