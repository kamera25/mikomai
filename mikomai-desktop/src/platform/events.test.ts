import { describe, expect, it } from "vitest";
import { acceptEvent } from "./events";
describe("event boundary", () => {
  it("drops an older sequence for the same task", () => {
    expect(acceptEvent({ taskId: "t", type: "x", sequence: 2 }, { taskId: "t", type: "x", sequence: 1 })).toBe(false);
  });
  it("does not compare sequence numbers across tasks", () => {
    expect(acceptEvent({ taskId: "a", type: "x", sequence: 2 }, { taskId: "b", type: "x", sequence: 1 })).toBe(true);
  });
});
