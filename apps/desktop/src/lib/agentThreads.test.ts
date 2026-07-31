import { describe, expect, it } from "vitest";

import {
  displayTitleForThread,
  uiMessageFromStoredContent,
  uiMessagesFromThreadMessages,
  type AgentThreadMessage,
} from "./agentThreads";

describe("displayTitleForThread", () => {
  it("prefers an explicit title", () => {
    expect(displayTitleForThread({ id: "abc", title: " Investigate docs " })).toBe(
      "Investigate docs",
    );
  });

  it("falls back to a short id label", () => {
    expect(
      displayTitleForThread({
        id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        title: null,
      }),
    ).toBe("Thread a1b2c3d4");
  });
});

describe("uiMessageFromStoredContent", () => {
  it("unwraps B2 uiMessage envelopes", () => {
    const message: AgentThreadMessage = {
      id: "stored-1",
      threadId: "t1",
      role: "user",
      content: {
        type: "uiMessage",
        id: "u1",
        role: "user",
        parts: [{ type: "text", text: "hello" }],
      },
      createdAt: 1,
    };

    expect(uiMessageFromStoredContent(message)).toEqual({
      id: "u1",
      role: "user",
      parts: [{ type: "text", text: "hello" }],
    });
  });

  it("maps plain text content", () => {
    expect(
      uiMessageFromStoredContent({
        id: "m1",
        role: "assistant",
        content: { text: "reply" },
      }),
    ).toEqual({
      id: "m1",
      role: "assistant",
      parts: [{ type: "text", text: "reply" }],
    });
  });

  it("skips unknown roles", () => {
    expect(
      uiMessageFromStoredContent({
        id: "m1",
        role: "tool",
        content: { text: "x" },
      }),
    ).toBeNull();
  });
});

describe("uiMessagesFromThreadMessages", () => {
  it("preserves order and drops unsupported roles", () => {
    const messages: AgentThreadMessage[] = [
      {
        id: "1",
        threadId: "t",
        role: "user",
        content: { type: "uiMessage", parts: [{ type: "text", text: "a" }] },
        createdAt: 1,
      },
      {
        id: "2",
        threadId: "t",
        role: "tool",
        content: { text: "skip" },
        createdAt: 2,
      },
      {
        id: "3",
        threadId: "t",
        role: "assistant",
        content: { type: "uiMessage", parts: [{ type: "text", text: "b" }] },
        createdAt: 3,
      },
    ];

    expect(uiMessagesFromThreadMessages(messages).map((message) => message.id)).toEqual([
      "1",
      "3",
    ]);
  });
});
