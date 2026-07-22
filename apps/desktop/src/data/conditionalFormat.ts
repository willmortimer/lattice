import type { Theme } from "@glideapps/glide-data-grid";

/** Mirrors `lattice_data::ConditionalFormatStyle`. */
export interface ConditionalFormatStyle {
  bg?: string;
  text?: string;
}

/** Mirrors `lattice_data::ConditionalFormatRule`. */
export interface ConditionalFormatRule {
  field: string;
  operator: "equals" | "contains";
  value: string;
  style: ConditionalFormatStyle;
}

const TOKEN_FALLBACKS: Record<string, string> = {
  bg: "#0a0d13",
  "bg-raise": "#11161f",
  panel: "#131923",
  text: "#f2ede3",
  "text-soft": "#c9c2b7",
  muted: "#9d9891",
  faint: "#77736e",
  accent: "#d69b45",
  "accent-bright": "#efb85f",
  "accent-deep": "#d98615",
  "accent-wash": "#372b1f",
  danger: "#ff9d8a",
};

function readCssToken(name: string, fallback: string): string {
  if (typeof document === "undefined") {
    return fallback;
  }
  return (
    getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback
  );
}

/** Resolve a semantic token name (`accent-wash`) to a concrete CSS color via `--lt-*`. */
export function resolveLtToken(tokenName: string | undefined): string | undefined {
  if (!tokenName) return undefined;
  const fallback = TOKEN_FALLBACKS[tokenName] ?? tokenName;
  return readCssToken(`--lt-${tokenName}`, fallback);
}

export function ruleMatchesDisplay(
  rule: ConditionalFormatRule,
  display: string,
): boolean {
  const value = display.toLowerCase();
  const needle = rule.value.toLowerCase();
  switch (rule.operator) {
    case "equals":
      return value === needle;
    case "contains":
      return value.includes(needle);
    default: {
      const _exhaustive: never = rule.operator;
      return _exhaustive;
    }
  }
}

/**
 * First matching rule for `field` wins. Returns a Glide `themeOverride` using
 * resolved `--lt-*` colors when a rule matches.
 */
export function themeOverrideForCell(
  field: string,
  display: string,
  rules: readonly ConditionalFormatRule[] | undefined,
): Partial<Theme> | undefined {
  if (!rules || rules.length === 0) return undefined;
  const match = rules.find(
    (rule) => rule.field === field && ruleMatchesDisplay(rule, display),
  );
  if (!match) return undefined;
  const bgCell = resolveLtToken(match.style.bg);
  const textDark = resolveLtToken(match.style.text);
  if (!bgCell && !textDark) return undefined;
  return {
    ...(bgCell ? { bgCell } : {}),
    ...(textDark ? { textDark } : {}),
  };
}
