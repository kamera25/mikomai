import { describe, expect, it } from "vitest";
import { findPreset, PRESET_MODELS } from "./settingsModelPresets";

describe("settings model presets", () => {
  it("keeps the shipped preset catalog stable", () => {
    expect(PRESET_MODELS).toHaveLength(3);
    expect(findPreset(PRESET_MODELS[0].repo, PRESET_MODELS[0].filename)?.id).toBe("gemma-4-e4b-ud");
  });

  it("returns no preset for custom model coordinates", () => {
    expect(findPreset("custom/repository", "custom.gguf")).toBeUndefined();
  });
});
