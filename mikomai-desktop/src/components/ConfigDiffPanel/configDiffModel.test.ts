import { describe, expect, it } from "vitest";
import { isActiveCommitPhase, STEP_DEFINITIONS } from "./configDiffModel";

describe("config diff execution model", () => {
  it("recognizes only the four execution phases", () => {
    expect(STEP_DEFINITIONS).toHaveLength(4);
    expect(isActiveCommitPhase("deploying")).toBe(true);
    expect(isActiveCommitPhase("success")).toBe(false);
  });
});
