import { describe, expect, it } from "vitest";
import { EditorState } from "@tiptap/pm/state";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";

import {
  agentAnchorHighlightKey,
  createAgentAnchorHighlightPlugin,
} from "./AgentAnchorHighlight";

describe("AgentAnchorHighlight", () => {
  it("stores overlay highlights in plugin state", () => {
    const headless = new Editor({
      extensions: [StarterKit],
      content: {
        type: "doc",
        content: [{ type: "paragraph", content: [{ type: "text", text: "Hello" }] }],
      },
    });
    const plugin = createAgentAnchorHighlightPlugin();
    const state = EditorState.create({
      schema: headless.schema,
      doc: headless.state.doc,
      plugins: [...headless.extensionManager.plugins, plugin],
    });
    headless.destroy();

    expect(agentAnchorHighlightKey.getState(state)).toEqual([]);
    const next = state.apply(
      state.tr.setMeta(agentAnchorHighlightKey, [
        { overlayId: "overlay-1", from: 1, to: 6, purpose: "attention" },
      ]),
    );
    expect(agentAnchorHighlightKey.getState(next)).toEqual([
      { overlayId: "overlay-1", from: 1, to: 6, purpose: "attention" },
    ]);
  });
});
