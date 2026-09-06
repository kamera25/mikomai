import { describe, expect, it } from "vitest";
import { defaultFilename, isChoiceTool, isNetworkDatabaseTool } from "./timelineModel";

describe("timeline model helpers", () => {
  it("classifies backend tool events", () => {
    expect(isNetworkDatabaseTool("network_query_nw_db")).toBe(true);
    expect(isChoiceTool("ask_ipaddress_choice")).toBe(true);
    expect(isChoiceTool("ping")).toBe(false);
  });

  it("extracts portable filenames", () => {
    expect(defaultFilename("C:\\tmp\\router.txt")).toBe("router.txt");
    expect(defaultFilename("/tmp/router.txt")).toBe("router.txt");
  });
});
