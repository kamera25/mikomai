import { describe, expect, it } from "vitest";
import { mergeTaskContent, typingStep } from "../mcpListenerState";

describe("mcp listener content state", () => {
  it("merges streamed prefixes without duplicating content", () => {
    expect(mergeTaskContent("abc", "abcdef")).toBe("abcdef");
    expect(mergeTaskContent("abcdef", "cde")).toBe("abcdef");
    expect(mergeTaskContent("", "abc")).toBe("abc");
  });

  it("appends an independent final report", () => {
    expect(mergeTaskContent("step log", "final report")).toBe("step log\n\nfinal report");
  });

  it("scales typing work for long messages", () => {
    expect(typingStep(10)).toBe(1);
    expect(typingStep(30)).toBe(2);
    expect(typingStep(80)).toBe(3);
    expect(typingStep(300)).toBe(10);
  });
});
