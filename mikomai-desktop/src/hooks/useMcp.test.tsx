import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMcp } from "./useMcp";

const mockInvoke = vi.fn().mockResolvedValue("Mock response containing no json");
const mockListen = vi.fn().mockResolvedValue(() => {});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: any[]) => mockListen(...args),
}));

describe("useMcp", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
  });

  it("should directly call ask_llm regardless of user queries (unified backend routing)", async () => {
    const setMessages = vi.fn();
    const setSummaries = vi.fn();
    const updateRecentHosts = vi.fn();

    const { result } = renderHook(() =>
      useMcp({
        messages: [],
        setMessages,
        summaries: [],
        setSummaries,
        historyLimit: 5,
        updateRecentHosts,
        recentIPs: [],
      })
    );

    // Test a shortcut query like "ping 8.8.8.8" which previously had regex logic on the frontend
    await act(async () => {
      await result.current.handleMcpResponse("ping 8.8.8.8");
    });

    // It should invoke the handle_mcp_message backend command
    expect(mockInvoke).toHaveBeenCalledWith("handle_mcp_message", expect.any(Object));
  });
});
