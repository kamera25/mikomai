import { describe, expect, it } from "vitest";
import { formatMessageTime } from "./messageTime";

describe("formatMessageTime", () => {
  it("returns an empty string for absent or invalid timestamps", () => {
    expect(formatMessageTime()).toBe("");
    expect(formatMessageTime("invalid")).toBe("");
  });

  it("formats same-day timestamps as a time", () => {
    const result = formatMessageTime("2026-08-30T01:02:00.000Z", new Date("2026-08-30T12:00:00.000Z"));
    expect(result).toMatch(/\d{2}:\d{2}/);
  });
});
