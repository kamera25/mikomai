import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useHistory } from "../useHistory";
import { ChatSession } from "../../types";
import * as tauriApi from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("useHistory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should load history on mount", async () => {
    const mockHistory = [
      {
        id: "session-test",
        type: "session" as const,
        title: "Mock Session",
        messages: [],
      },
    ];
    vi.mocked(tauriApi.invoke).mockResolvedValue(mockHistory);

    const { result } = renderHook(() => useHistory());

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(result.current.history).toEqual(mockHistory);
    expect(result.current.activeSessionId).toBe("session-test");
  });

  it("should initialize with default session if history is empty", async () => {
    vi.mocked(tauriApi.invoke).mockResolvedValue([]);

    const { result } = renderHook(() => useHistory());

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(result.current.history).toHaveLength(1);
    expect((result.current.history[0] as ChatSession).title).toBe("新しいセッション");
  });

  it("should save history when history changes", async () => {
    vi.mocked(tauriApi.invoke).mockResolvedValue([]);
    const { result } = renderHook(() => useHistory());

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Reset mock calls after initialization
    vi.mocked(tauriApi.invoke).mockClear();

    act(() => {
      result.current.createNewSession();
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("save_history", expect.any(Object));
  });
});
