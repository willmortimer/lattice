import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * Guard the sidebar flex/overflow contract that keeps the virtualized resource
 * tree from painting over Connected roots / Proposals.
 *
 * Absolute `.resource-tree-row` nodes scroll against `.resource-list`. If
 * `.resource-tree-virtual` is flex-constrained (`flex: 1`) while those rows
 * overflow a transparent `.proposal-inbox`, the inbox appears to overlap the
 * file tree.
 */
describe("sidebar layout CSS contract", () => {
  const css = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "../styles.css"),
    "utf8",
  );

  function ruleBody(selector: string): string {
    const match = css.match(new RegExp(`${selector.replace(".", "\\.")}\\s*\\{([^}]*)\\}`));
    expect(match, `missing rule for ${selector}`).not.toBeNull();
    return match?.[1] ?? "";
  }

  it("lets .resource-list shrink and scroll inside the sidebar", () => {
    const sidebar = ruleBody(".sidebar");
    expect(sidebar).toMatch(/min-height:\s*0/);
    expect(sidebar).toMatch(/overflow:\s*hidden/);

    const list = ruleBody(".resource-list");
    expect(list).toMatch(/flex:\s*1/);
    expect(list).toMatch(/min-height:\s*0/);
    expect(list).toMatch(/overflow-y:\s*auto/);
  });

  it("sizes the virtual tree from its spacer, not a flex:1 box", () => {
    const tree = ruleBody(".resource-tree-virtual");
    expect(tree).not.toMatch(/flex:\s*1/);
    expect(tree).toMatch(/flex-shrink:\s*0/);
  });

  it("keeps the proposal inbox opaque, capped, and non-collapsing", () => {
    const inbox = ruleBody(".proposal-inbox");
    expect(inbox).toMatch(/flex-shrink:\s*0/);
    expect(inbox).toMatch(/max-height:\s*42%/);
    expect(inbox).toMatch(/overflow-y:\s*auto/);
    expect(inbox).toMatch(/background:\s*var\(--lt-bg-raise\)/);
  });
});
