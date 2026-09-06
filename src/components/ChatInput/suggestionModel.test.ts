import { describe, expect, it } from "vitest";
import { findHostSuggestions } from "./suggestionModel";

describe("host suggestion model", () => {
  it("deduplicates recent IPs already represented by hosts", () => {
    const result = findHostSuggestions("10", [{ hostname: "router", ip: "10.0.0.1" }], ["10.0.0.1", "10.0.0.2"], { localhost: "localhost", pastIps: "past" });
    expect(result.map((item) => item.hostname)).toEqual(["router", "10.0.0.2"]);
  });
});
