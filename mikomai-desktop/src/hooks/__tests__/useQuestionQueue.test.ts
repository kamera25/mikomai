import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri modules
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

import { invoke } from "@tauri-apps/api/core";
import { useQuestionQueue } from "../useQuestionQueue";

describe("useQuestionQueue", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("initializes with empty question queue", () => {
    const { result } = renderHook(() => useQuestionQueue());
    expect(result.current.questionQueue).toEqual([]);
    expect(result.current.totalQuestionsCount).toBe(0);
  });

  it("submits user choice and removes item from queue", async () => {
    const { result } = renderHook(() => useQuestionQueue());

    // Call submit choice handler
    await act(async () => {
      await result.current.handleSelectChoice("q1", "Option A");
    });

    expect(invoke).toHaveBeenCalledWith("submit_user_choice", {
      id: "q1",
      choice: "Option A",
    });
  });

  it("cancels user choice and removes item from queue", async () => {
    const { result } = renderHook(() => useQuestionQueue());

    await act(async () => {
      await result.current.handleCancelChoice("q1");
    });

    expect(invoke).toHaveBeenCalledWith("submit_user_choice", {
      id: "q1",
      choice: "cancelled",
    });
  });

  it("submits interface choice and removes item from queue", async () => {
    const { result } = renderHook(() => useQuestionQueue());

    await act(async () => {
      await result.current.handleSelectInterface("q2", "GigabitEthernet0/1");
    });

    expect(invoke).toHaveBeenCalledWith("submit_interface_choice", {
      id: "q2",
      choice: "GigabitEthernet0/1",
    });
  });

  it("submits IP address choice and removes item from queue", async () => {
    const { result } = renderHook(() => useQuestionQueue());

    await act(async () => {
      await result.current.handleSelectIpAddress("q3", "192.168.1.1/24");
    });

    expect(invoke).toHaveBeenCalledWith("submit_ipaddress_choice", {
      id: "q3",
      choice: "192.168.1.1/24",
    });
  });
});
