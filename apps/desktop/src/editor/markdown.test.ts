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

  wikiLinks: "See [[Product/Vision]] and [[Home|back home]].\n",

  table:
    "| Name | Qty | Price |\n" +
    "| :-- | :-: | --: |\n" +
    "| Widget | 3 | 9.99 |\n" +
    "| Gadget | 1 | 19.5 |\n",

  tableNoAlignment: "| A | B |\n| --- | --- |\n| 1 | 2 |\n",

  horizontalRule: "Above.\n\n---\n\nBelow.\n",

  image: 'An inline ![alt text](./diagram.png "Diagram") within a paragraph.\n',

  imageNoTitleOrAlt: "![](assets/photo.png)\n",

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

  it("serializes an image with alt text and a title", () => {
    const json = parseMarkdownToJSON('![alt text](./diagram.png "Diagram")\n');
    expect(serializeJSONToMarkdown(json)).toBe('![alt text](./diagram.png "Diagram")\n');
  });

  it("serializes an image with neither alt text nor a title", () => {
    const json = parseMarkdownToJSON("![](assets/photo.png)\n");
    expect(serializeJSONToMarkdown(json)).toBe("![](assets/photo.png)\n");
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

describe(":::lattice-embed directives", () => {
  const EMBED_SAMPLE =
    ":::lattice-embed\n" +
    "resource: ../Data/Services.data/views/Active.view.yaml\n" +
    "view: table\n" +
    "height: 640\n" +
    "lines: 10-20\n" +
    'fallback: "[Open active services](../Data/Services.data/views/Active.view.yaml)"\n' +
    "custom-flag: enabled\n" +
    ":::\n";

  it("round-trips a lattice-embed with known and unknown fields", () => {
    const firstParse = parseMarkdownToJSON(EMBED_SAMPLE);
    const serialized = serializeJSONToMarkdown(firstParse);
    const secondParse = parseMarkdownToJSON(serialized);

    expect(secondParse).toEqual(firstParse);
    expect(firstParse.content?.[0]).toMatchObject({
      type: "latticeEmbed",
      attrs: {
        resource: "../Data/Services.data/views/Active.view.yaml",
        view: "table",
        height: "640",
        lines: "10-20",
        fallback: "[Open active services](../Data/Services.data/views/Active.view.yaml)",
        extraFields: { "custom-flag": "enabled" },
        extraFieldKeys: ["custom-flag"],
      },
    });
  });

  it("round-trips mode: interactive", () => {
    const raw =
      ":::lattice-embed\n" +
      "resource: Artifacts/ContactPulse.artifact\n" +
      "mode: interactive\n" +
      "height: 320\n" +
      ":::\n";
    const firstParse = parseMarkdownToJSON(raw);
    const serialized = serializeJSONToMarkdown(firstParse);
    expect(serialized).toContain("mode: interactive\n");
    expect(parseMarkdownToJSON(serialized)).toEqual(firstParse);
    expect(firstParse.content?.[0]).toMatchObject({
      type: "latticeEmbed",
      attrs: {
        resource: "Artifacts/ContactPulse.artifact",
        mode: "interactive",
        height: "320",
      },
    });
  });

  it("serializes lattice-embed fields in documented order", () => {
    const json = parseMarkdownToJSON(EMBED_SAMPLE);
    const markdown = serializeJSONToMarkdown(json);

    expect(markdown).toContain(":::lattice-embed\n");
    expect(markdown).toContain("resource: ../Data/Services.data/views/Active.view.yaml\n");
    expect(markdown).toContain("view: table\n");
    expect(markdown).toContain("height: 640\n");
    expect(markdown).toContain("lines: 10-20\n");
    expect(markdown).toContain(
      'fallback: "[Open active services](../Data/Services.data/views/Active.view.yaml)"\n',
    );
    expect(markdown).toContain("custom-flag: enabled\n");
    expect(markdown.trimEnd().endsWith(":::")).toBe(true);
  });

  it("preserves unsupported directives as opaque raw blocks", () => {
    const raw =
      ":::lattice-code\n" +
      "source: ../src/parser.rs\n" +
      "symbol: Parser::parse_document\n" +
      "language: rust\n" +
      ":::\n";

    const firstParse = parseMarkdownToJSON(raw);
    const serialized = serializeJSONToMarkdown(firstParse);
    const secondParse = parseMarkdownToJSON(serialized);

    expect(secondParse).toEqual(firstParse);
    expect(serialized).toBe(raw);
    expect(firstParse.content?.[0]).toMatchObject({
      type: "opaqueDirective",
      attrs: { raw },
    });
  });

  it("round-trips a page with prose and an embed", () => {
    const markdown =
      "# Deployment\n\n" +
      "See the active services view.\n\n" +
      ":::lattice-embed\n" +
      "resource: ../Data/Services.data/views/Active.view.yaml\n" +
      "height: 640\n" +
      ":::\n";

    const firstParse = parseMarkdownToJSON(markdown);
    const serialized = serializeJSONToMarkdown(firstParse);
    const secondParse = parseMarkdownToJSON(serialized);

    expect(secondParse).toEqual(firstParse);
  });
});
