import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useModel } from "../useModel";
import * as tauriApi from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("useModel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("should initialize with NotLoaded status", () => {
    const { result } = renderHook(() => useModel("/fake/model/path"));
    expect(result.current.modelStatus).toBe("NotLoaded");
  });

  it("should check model status periodically", async () => {
    vi.mocked(tauriApi.invoke).mockResolvedValue("Loaded");

    renderHook(() => useModel("/fake/model/path"));

    // Fast-forward interval
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("get_model_status");
  });

  it("should load the model successfully", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd) => {
      if (cmd === "load_model") return Promise.resolve();
      return Promise.resolve("NotLoaded");
    });

    const { result } = renderHook(() => useModel("/fake/model/path"));

    await act(async () => {
      await result.current.handleLoadModel();
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("load_model", { path: "/fake/model/path" });
    expect(result.current.modelStatus).toBe("Loaded");
  });

  it("should set status to Error on load model failure", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd) => {
      if (cmd === "load_model") return Promise.reject(new Error("Load failed"));
      return Promise.resolve("NotLoaded");
    });

    const { result } = renderHook(() => useModel("/fake/model/path"));

    await act(async () => {
      await result.current.handleLoadModel();
    });

    expect(result.current.modelStatus).toBe("Error");
  });
});
