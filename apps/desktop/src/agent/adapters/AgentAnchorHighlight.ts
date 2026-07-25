import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorState, Transaction } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

import type { AnchorHighlightPurpose } from "./types";

export interface AgentAnchorHighlightEntry {
  overlayId: string;
  from: number;
  to: number;
  purpose: AnchorHighlightPurpose;
}

export type AgentAnchorHighlightState = AgentAnchorHighlightEntry[];

export const agentAnchorHighlightKey = new PluginKey<AgentAnchorHighlightState>(
  "latticeAgentAnchorHighlight",
);

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    agentAnchorHighlight: {
      highlightAgentAnchor: (
        overlayId: string,
        from: number,
        to: number,
        purpose: AnchorHighlightPurpose,
      ) => ReturnType;
      clearAgentAnchorOverlay: (overlayId: string) => ReturnType;
    };
  }
}

function applyHighlightMeta(
  tr: Transaction,
  value: AgentAnchorHighlightState,
): Transaction {
  return tr.setMeta(agentAnchorHighlightKey, value);
}

export function createAgentAnchorHighlightPlugin() {
  return new Plugin<AgentAnchorHighlightState>({
    key: agentAnchorHighlightKey,
    state: {
      init: () => [],
      apply(tr: Transaction, value: AgentAnchorHighlightState) {
        const meta = tr.getMeta(agentAnchorHighlightKey) as
          | AgentAnchorHighlightState
          | undefined;
        if (meta) return meta;
        if (!tr.docChanged) return value;
        return value
          .map((entry) => ({
            ...entry,
            from: tr.mapping.map(entry.from),
            to: tr.mapping.map(entry.to),
          }))
          .filter((entry) => entry.from < entry.to);
      },
    },
    props: {
      decorations(state: EditorState) {
        const entries = agentAnchorHighlightKey.getState(state) ?? [];
        if (entries.length === 0) return DecorationSet.empty;
        return DecorationSet.create(
          state.doc,
          entries.map((entry) =>
            Decoration.node(entry.from, entry.to, {
              class: `agent-anchor-highlight agent-anchor-highlight--${entry.purpose}`,
              "data-agent-overlay-id": entry.overlayId,
            }),
          ),
        );
      },
    },
  });
}

export const AgentAnchorHighlight = Extension.create({
  name: "agentAnchorHighlight",

  addCommands() {
    return {
      highlightAgentAnchor:
        (overlayId, from, to, purpose) =>
        ({ tr, dispatch, state }) => {
          const current = agentAnchorHighlightKey.getState(state) ?? [];
          const next = [
            ...current.filter((entry) => entry.overlayId !== overlayId),
            { overlayId, from, to, purpose },
          ];
          if (dispatch) dispatch(applyHighlightMeta(tr, next));
          return true;
        },
      clearAgentAnchorOverlay:
        (overlayId) =>
        ({ tr, dispatch, state }) => {
          const current = agentAnchorHighlightKey.getState(state) ?? [];
          const next = current.filter((entry) => entry.overlayId !== overlayId);
          if (next.length === current.length) return false;
          if (dispatch) dispatch(applyHighlightMeta(tr, next));
          return true;
        },
    };
  },

  addProseMirrorPlugins() {
    return [createAgentAnchorHighlightPlugin()];
  },
});
