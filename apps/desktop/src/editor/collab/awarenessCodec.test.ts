import { describe, expect, it } from "vitest";
import * as Y from "yjs";
import { Awareness } from "y-protocols/awareness";

import { applyAwarenessDelta, encodeAwarenessDelta } from "./awarenessCodec";

describe("awarenessCodec", () => {
  it("encodes and applies awareness updates between peers", () => {
    const docA = new Y.Doc();
    const docB = new Y.Doc();
    const awarenessA = new Awareness(docA);
    const awarenessB = new Awareness(docB);

    awarenessA.setLocalStateField("user", { name: "Alice", color: "#ff0000" });

    const update = encodeAwarenessDelta(awarenessA, [awarenessA.clientID]);
    applyAwarenessDelta(awarenessB, update);

    const remote = awarenessB.states.get(awarenessA.clientID);
    expect(remote?.user).toEqual({ name: "Alice", color: "#ff0000" });

    awarenessA.destroy();
    awarenessB.destroy();
    docA.destroy();
    docB.destroy();
  });

  it("does not echo when applying with fanout origin", () => {
    const doc = new Y.Doc();
    const awareness = new Awareness(doc);
    let updateCount = 0;
    awareness.on("update", () => {
      updateCount += 1;
    });

    awareness.setLocalStateField("user", { name: "Bob", color: "#00ff00" });
    const update = encodeAwarenessDelta(awareness, [awareness.clientID]);
    applyAwarenessDelta(awareness, update);

    expect(updateCount).toBe(1);

    awareness.destroy();
    doc.destroy();
  });
});
