import { describe, expect, it } from "vitest";

import { joinFrontmatter, parseMarkdownToJSON, serializeJSONToMarkdown, splitFrontmatter } from "./markdown";

/**
 * Each sample must be semantically stable under one parse/serialize cycle:
 * `parse(serialize(parse(md)))` must equal `parse(md)`. This is the
 * standard round-trip guarantee for a markdown editor — exact byte
 * equality isn't required (the serializer normalizes whitespace and list
 * markers), but re-parsing the serialized output must never lose or
 * reshape content.
 */
const CORPUS: Record<string, string> = {
  headings: "# Title\n\n## Subtitle\n\n###### Deepest\n\nParagraph text after headings.\n",

  paragraphsAndEmphasis:
    "A paragraph with *italic*, **bold**, ~~strikethrough~~, and `inline code`.\n\n" +
    "A second paragraph with **bold *and italic* together**.\n",

  bulletList: "- First item\n- Second item\n- Third item with **bold**\n",

  nestedBulletList: "- Parent\n  - Child one\n  - Child two\n- Sibling\n",

  orderedList: "1. First\n2. Second\n3. Third\n",

  orderedListCustomStart: "5. Fifth\n6. Sixth\n7. Seventh\n",

  blockquote: "> A quoted line.\n> A second quoted line.\n",

  nestedBlockquote: "> Outer quote\n>\n> > Inner quote\n",

  fencedCodeBlock: "```ts\nconst x: number = 1;\nconsole.log(x);\n```\n",

  fencedCodeBlockNoLanguage: "```\nplain fenced content\n```\n",

  codeBlockWithBackticks: "````\nhas ``` inside it\n````\n",

  links: "See [the docs](https://example.com/docs \"Docs\") for more, or just [plain](https://example.com).\n",

  table:
    "| Name | Qty | Price |\n" +
    "| :-- | :-: | --: |\n" +
    "| Widget | 3 | 9.99 |\n" +
    "| Gadget | 1 | 19.5 |\n",

  tableNoAlignment: "| A | B |\n| --- | --- |\n| 1 | 2 |\n",

  horizontalRule: "Above.\n\n---\n\nBelow.\n",

  mixedDocument:
    "# Project Notes\n\n" +
    "An overview paragraph with a [link](https://example.com) and `code`.\n\n" +
    "## Tasks\n\n" +
    "- Ship the editor\n- Preserve frontmatter\n\n" +
    "> Ship v0 first, polish later.\n\n" +
    "```bash\npnpm test\n```\n",
};

describe("markdown round-trip corpus", () => {
  for (const [name, markdown] of Object.entries(CORPUS)) {
    it(`round-trips ${name}`, () => {
      const firstParse = parseMarkdownToJSON(markdown);
      const serialized = serializeJSONToMarkdown(firstParse);
      const secondParse = parseMarkdownToJSON(serialized);

      expect(secondParse).toEqual(firstParse);
    });
  }
});

describe("markdown serializer output", () => {
  it("serializes headings with the correct number of hashes", () => {
    const json = parseMarkdownToJSON("### Three\n");
    expect(serializeJSONToMarkdown(json)).toBe("### Three\n");
  });

  it("serializes a simple bullet list with dash markers", () => {
    const json = parseMarkdownToJSON("- one\n- two\n");
    expect(serializeJSONToMarkdown(json)).toBe("- one\n- two\n");
  });

  it("serializes a fenced code block with its language", () => {
    const json = parseMarkdownToJSON("```rust\nfn main() {}\n```\n");
    expect(serializeJSONToMarkdown(json)).toBe("```rust\nfn main() {}\n```\n");
  });

  it("escapes pipes inside table cells", () => {
    const json = parseMarkdownToJSON("| A |\n| --- |\n| has \\| pipe |\n");
    expect(serializeJSONToMarkdown(json)).toContain("has \\| pipe");
  });
});

describe("frontmatter split/join", () => {
  it("splits a leading YAML block from the body", () => {
    const raw = "---\nid: abc123\ntitle: Hello\n---\n# Hello\n\nBody text.\n";
    const { frontmatter, body } = splitFrontmatter(raw);

    expect(frontmatter).toBe("---\nid: abc123\ntitle: Hello\n---\n");
    expect(body).toBe("# Hello\n\nBody text.\n");
  });

  it("returns null frontmatter when the file has none", () => {
    const raw = "# Hello\n\nJust a page, no frontmatter.\n";
    const { frontmatter, body } = splitFrontmatter(raw);

    expect(frontmatter).toBeNull();
    expect(body).toBe(raw);
  });

  it("does not treat a leading horizontal rule as frontmatter", () => {
    const raw = "---\n\nJust an hr up top, then prose.\n";
    const { frontmatter, body } = splitFrontmatter(raw);

    expect(frontmatter).toBeNull();
    expect(body).toBe(raw);
  });

  it("round-trips split + join back to the exact original bytes", () => {
    const raw = "---\nid: xyz\ntags: [a, b]\n---\n\n## Section\n\nContent.\n";
    const { frontmatter, body } = splitFrontmatter(raw);

    expect(joinFrontmatter(frontmatter, body)).toBe(raw);
  });

  it("preserves frontmatter verbatim through a full editor save cycle", () => {
    const raw =
      "---\nid: 0198-page\ntitle: Roadmap\ncreated: 2026-01-01\n---\n" +
      "# Roadmap\n\n- Q1: ship editor\n- Q2: ship search\n";
    const { frontmatter, body } = splitFrontmatter(raw);

    const edited = serializeJSONToMarkdown(parseMarkdownToJSON(body));
    const saved = joinFrontmatter(frontmatter, edited);

    expect(saved.startsWith(frontmatter!)).toBe(true);
    expect(parseMarkdownToJSON(splitFrontmatter(saved).body)).toEqual(parseMarkdownToJSON(body));
  });
});
