import YAML from "yaml";

import { splitFrontmatter } from "../editor/markdown";

export type HelpNavItem = {
  label: string;
  file: string;
};

export type HelpNavSection = {
  label: string;
  items: HelpNavItem[];
};

export type HelpPageMeta = {
  title: string;
  description: string;
  anchor: string | null;
};

export type HelpPage = {
  stem: string;
  file: string;
  navLabel: string;
  title: string;
  description: string;
  anchor: string | null;
  body: string;
  raw: string;
};

export function stemFromHelpFile(file: string): string {
  return file.replace(/\.md$/i, "");
}

export function parseHelpNavigation(navigation: HelpNavSection[]): HelpNavSection[] {
  return navigation.map((section) => ({
    label: section.label.trim(),
    items: section.items.map((item) => ({
      label: item.label.trim(),
      file: item.file.trim(),
    })),
  }));
}

function parseHelpFrontmatter(frontmatterBlock: string | null): HelpPageMeta {
  if (!frontmatterBlock) {
    return { title: "", description: "", anchor: null };
  }
  const inner = frontmatterBlock
    .replace(/^---\r?\n/, "")
    .replace(/\r?\n---[ \t]*\r?\n?$/, "");
  const parsed = YAML.parse(inner);
  if (!parsed || typeof parsed !== "object") {
    return { title: "", description: "", anchor: null };
  }
  const record = parsed as Record<string, unknown>;
  const title = typeof record.title === "string" ? record.title.trim() : "";
  const description =
    typeof record.description === "string" ? record.description.trim() : "";
  const anchor =
    typeof record.anchor === "string" && record.anchor.trim()
      ? record.anchor.trim()
      : null;
  return { title, description, anchor };
}

export function parseHelpPageRaw(file: string, raw: string, navLabel: string): HelpPage {
  const { frontmatter, body } = splitFrontmatter(raw);
  const meta = parseHelpFrontmatter(frontmatter);
  const stem = stemFromHelpFile(file);
  const title = meta.title || navLabel || stem;
  return {
    stem,
    file,
    navLabel,
    title,
    description: meta.description,
    anchor: meta.anchor,
    body,
    raw,
  };
}

export function buildHelpCorpus(
  navigation: HelpNavSection[],
  rawByFile: Record<string, string>,
): { navigation: HelpNavSection[]; pages: HelpPage[] } {
  const parsedNavigation = parseHelpNavigation(navigation);
  const pages: HelpPage[] = [];
  for (const section of parsedNavigation) {
    for (const item of section.items) {
      const raw = rawByFile[item.file];
      if (!raw) continue;
      pages.push(parseHelpPageRaw(item.file, raw, item.label));
    }
  }
  return { navigation: parsedNavigation, pages };
}

export function filterHelpPages(pages: HelpPage[], query: string): HelpPage[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return pages;
  return pages.filter((page) => {
    const haystack = [
      page.title,
      page.navLabel,
      page.description,
      page.body,
      page.stem,
    ]
      .join("\n")
      .toLowerCase();
    return haystack.includes(trimmed);
  });
}

export function findHelpPageByStem(pages: HelpPage[], stem: string): HelpPage | undefined {
  const normalized = stem.trim().toLowerCase();
  return pages.find((page) => page.stem.toLowerCase() === normalized);
}
