import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSettings } from "../useSettings";
import * as tauriApi from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("useSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should load settings on mount", async () => {
    const mockSettings = {
      historyLimit: 8,
      temperature: 0.5,
      repetitionPenalty: 1.2,
      modelPath: "/path/to/model.gguf",
      recentIps: ["8.8.8.8"],
      mcpTimeout: 20,
      cacheExpiryMinutes: 5,
      ipVersion: "ipv4",
      consolePort: "COM3",
      consoleBaudRate: 115200,
      preloadKnowledge: false,
      preloadAnalysis: false,
      preloadRag: false,
    };
    vi.mocked(tauriApi.invoke).mockResolvedValue(mockSettings);

    const { result } = renderHook(() => useSettings());

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(result.current.historyLimit).toBe(8);
    expect(result.current.temperature).toBe(0.5);
    expect(result.current.repetitionPenalty).toBe(1.2);
    expect(result.current.modelPath).toBe("/path/to/model.gguf");
    expect(result.current.recentIPs).toEqual(["8.8.8.8"]);
    expect(result.current.consolePort).toBe("COM3");
    expect(result.current.consoleBaudRate).toBe(115200);
  });

  it("should save settings when saveAllSettings is called", async () => {
    vi.mocked(tauriApi.invoke).mockResolvedValue(null);
    const { result } = renderHook(() => useSettings());

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    vi.mocked(tauriApi.invoke).mockClear();

    await act(async () => {
      await result.current.saveAllSettings({ temperature: 0.9 });
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith(
      "save_settings",
      expect.objectContaining({
        settings: expect.objectContaining({
          temperature: 0.9,
        }),
      })
    );
  });
});
