import { describe, it, expect, vi, beforeEach } from "vitest";
import { 
  extractJsonBlocks, 
  keysToCamelCase, 
  resolveDeviceNameStep, 
  resolveHostStep, 
  applyFallbackHostStep, 
  normalizeArgs 
} from "./helpers";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("extractJsonBlocks", () => {
  it("should extract simple JSON blocks", () => {
    const text = 'Some text {"key": "value"} some other text';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "value"}']);
  });

  it("should handle nested objects/braces", () => {
    const text = 'Nested block: {"outer": {"inner": "val"}} ok';
    expect(extractJsonBlocks(text)).toEqual(['{"outer": {"inner": "val"}}']);
  });

  it("should ignore braces inside string literals", () => {
    const text = 'Braces inside string: {"key": "val {with} braces", "another": "val2"}';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "val {with} braces", "another": "val2"}']);

    const textUnbalanced = 'Braces inside string: {"key": "val {with braces", "another": "val2"}';
    expect(extractJsonBlocks(textUnbalanced)).toEqual(['{"key": "val {with braces", "another": "val2"}']);
  });

  it("should handle escaped quotes inside string literals", () => {
    const text = 'Escaped quotes: {"key": "escaped \\" quote and {braces}"}';
    expect(extractJsonBlocks(text)).toEqual(['{"key": "escaped \\" quote and {braces}"}']);
  });

  it("should extract multiple JSON blocks", () => {
    const text = 'First {"a": 1} second {"b": 2}';
    expect(extractJsonBlocks(text)).toEqual(['{"a": 1}', '{"b": 2}']);
  });
});

describe("keysToCamelCase", () => {
  it("should convert snake_case keys to camelCase", () => {
    const obj = {
      device_name: "router-1",
      user_message: "hello",
      alreadyCamel: "ok",
    };
    expect(keysToCamelCase(obj)).toEqual({
      deviceName: "router-1",
      userMessage: "hello",
      alreadyCamel: "ok",
    });
  });
});

describe("resolveDeviceNameStep", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should return existing device name fields from args", async () => {
    const args1 = { deviceName: "dev-1" };
    expect(await resolveDeviceNameStep(args1)).toBe("dev-1");

    const args2 = { device_name: "dev-2" };
    expect(await resolveDeviceNameStep(args2)).toBe("dev-2");

    const args3 = { device: "dev-3" };
    expect(await resolveDeviceNameStep(args3)).toBe("dev-3");

    const args4 = { host: "dev-4" };
    expect(await resolveDeviceNameStep(args4)).toBe("dev-4");
  });

  it("should scan user message if not in args", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "load_connections") {
        return [{ hostname: "router-ios", ip: "192.168.1.1" }];
      }
      return [];
    });

    const args = {};
    const result = await resolveDeviceNameStep(args, "Please connect to router-ios");
    expect(result).toBe("router-ios");
  });
});

describe("resolveHostStep", () => {
  it("should return the host candidate from args", () => {
    expect(resolveHostStep({ host: "h1" })).toBe("h1");
    expect(resolveHostStep({ device: "h2" })).toBe("h2");
    expect(resolveHostStep({ deviceName: "h3" })).toBe("h3");
    expect(resolveHostStep({ device_name: "h4" })).toBe("h4");
    expect(resolveHostStep({ ip: "h5" })).toBe("h5");
  });
});

describe("applyFallbackHostStep", () => {
  it("should return value if exists", () => {
    expect(applyFallbackHostStep("existing", ["fallback"])).toBe("existing");
  });

  it("should fallback to recentIPs[0] if value is falsy", () => {
    expect(applyFallbackHostStep(undefined, ["192.168.1.1"])).toBe("192.168.1.1");
    expect(applyFallbackHostStep("", ["192.168.1.1"])).toBe("192.168.1.1");
  });
});

describe("normalizeArgs", () => {
  it("should retain and add user message fields, camelcasing all keys", async () => {
    const result = await normalizeArgs(
      "fetch_config",
      "user prompt",
      { device_name: "my-device" }
    );
    expect(result).toEqual({
      deviceName: "my-device",
      userMessage: "user prompt",
      user_message: "user prompt",
    });
  });
});
