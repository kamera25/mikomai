import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useModel } from "../useModel";
import * as tauriApi from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

describe("useModel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should initialize with NotLoaded status", () => {
    vi.mocked(listen).mockResolvedValue(vi.fn());
    const { result } = renderHook(() => useModel("/fake/model/path"));
    expect(result.current.modelStatus).toBe("NotLoaded");
  });

  it("should listen to model status changes and update status", async () => {
    let statusCallback: ((event: any) => void) | undefined;
    vi.mocked(listen).mockImplementation(async (event, callback) => {
      if (event === "model-status-changed") {
        statusCallback = callback;
      }
      return () => {};
    });
    vi.mocked(tauriApi.invoke).mockResolvedValue("NotLoaded");

    const { result } = renderHook(() => useModel("/fake/model/path"));

    // Verify initial invoke happened
    expect(tauriApi.invoke).toHaveBeenCalledWith("get_model_status");

    // Initially "NotLoaded"
    await act(async () => {
      // Allow initial checkStatus promise to resolve
    });
    expect(result.current.modelStatus).toBe("NotLoaded");

    // Simulate event
    expect(statusCallback).toBeDefined();
    if (statusCallback) {
      await act(async () => {
        statusCallback!({ payload: "Loaded" });
      });
    }

    expect(result.current.modelStatus).toBe("Loaded");
  });

  it("should load the model successfully", async () => {
    vi.mocked(listen).mockResolvedValue(vi.fn());
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
    vi.mocked(listen).mockResolvedValue(vi.fn());
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
