import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMcpListeners } from "../useMcpListeners";
import { Message, ChatEvent } from "../../../types";

type ListenerCallback = (event: { payload: ChatEvent }) => void;

let chatEventListener: ListenerCallback | null = null;

const mockListen = vi.fn().mockImplementation((event: string, callback: ListenerCallback) => {
  if (event === "chat-event") {
    chatEventListener = callback;
  }
  return Promise.resolve(() => {});
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, cb: any) => mockListen(event, cb),
}));

describe("useMcpListeners typing effect", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockListen.mockClear();
    chatEventListener = null;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("should display content immediately when chunk is 5 characters or fewer", async () => {
    let messages: Message[] = [];
    const setMessages = vi.fn((updater) => {
      messages = typeof updater === "function" ? updater(messages) : updater;
    });
    const setSummaries = vi.fn();

    renderHook(() =>
      useMcpListeners({
        setMessages,
        setSummaries,
      })
    );

    // Initial started event
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialStarted",
          payload: { taskId: "task-1", hasImage: false },
        },
      });
    });

    expect(messages.length).toBe(1);
    expect(messages[0].task_id).toBe("task-1");

    // Send 3 characters (<= 5)
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "llmChunk",
          payload: "abc",
        },
      });
    });

    // Should be displayed immediately
    expect(messages[0].content).toBe("abc");
  });

  it("should display character-by-character when 6 or more characters are received at once", async () => {
    let messages: Message[] = [];
    const setMessages = vi.fn((updater) => {
      messages = typeof updater === "function" ? updater(messages) : updater;
    });
    const setSummaries = vi.fn();

    renderHook(() =>
      useMcpListeners({
        setMessages,
        setSummaries,
      })
    );

    // Initial started event
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialStarted",
          payload: { taskId: "task-2", hasImage: false },
        },
      });
    });

    // Send 6 characters (>= 6)
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "llmChunk",
          payload: "ABCDEF",
        },
      });
    });

    // Before timer advances, it shouldn't show all 6 characters immediately
    expect(messages[0].content).not.toBe("ABCDEF");

    // Advance timer step 1 (10ms) -> "A"
    await act(async () => {
      vi.advanceTimersByTime(10);
    });
    expect(messages[0].content).toBe("A");

    // Advance timer step 2 (10ms) -> "AB"
    await act(async () => {
      vi.advanceTimersByTime(10);
    });
    expect(messages[0].content).toBe("AB");

    // Advance timer through the rest of the characters
    await act(async () => {
      vi.advanceTimersByTime(10 * 10);
    });
    expect(messages[0].content).toBe("ABCDEF");
  });

  it("should finish typing correctly on mcpInitialFinished", async () => {
    let messages: Message[] = [];
    const setMessages = vi.fn((updater) => {
      messages = typeof updater === "function" ? updater(messages) : updater;
    });
    const setSummaries = vi.fn();

    renderHook(() =>
      useMcpListeners({
        setMessages,
        setSummaries,
      })
    );

    // Initial started event
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialStarted",
          payload: { taskId: "task-3", hasImage: false },
        },
      });
    });

    // Send full response at once on finished event (>= 6 chars)
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialFinished",
          payload: { taskId: "task-3", content: "完了メッセージです" },
        },
      });
    });

    // Initially starts typing
    expect(messages[0].content).not.toBe("完了メッセージです");

    // Advance timers until finished
    await act(async () => {
      vi.advanceTimersByTime(10 * 15);
    });

    expect(messages[0].content).toBe("完了メッセージです");
    expect(messages[0].isToolLoading).toBe(false);
  });

  it("should not duplicate content when llmChunk is followed by mcpInitialFinished with same content", async () => {
    let messages: Message[] = [];
    const setMessages = vi.fn((updater) => {
      messages = typeof updater === "function" ? updater(messages) : updater;
    });
    const setSummaries = vi.fn();

    renderHook(() =>
      useMcpListeners({
        setMessages,
        setSummaries,
      })
    );

    // Initial started event
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialStarted",
          payload: { taskId: "task-4", hasImage: false },
        },
      });
    });

    const greeting = "こんにちは！どのようなご用件でしょうか？";

    // Send chunk
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "llmChunk",
          payload: greeting,
        },
      });
    });

    // Backend finishes with the same message
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialFinished",
          payload: { taskId: "task-4", content: greeting },
        },
      });
    });

    // Advance timers until complete
    await act(async () => {
      vi.advanceTimersByTime(10 * 50);
    });

    // Should strictly equal greeting once, not repeated twice
    expect(messages[0].content).toBe(greeting);
  });

  it("should correctly append final report after agent-step logs when AgentLoop finishes", async () => {
    let messages: Message[] = [];
    const setMessages = vi.fn((updater) => {
      messages = typeof updater === "function" ? updater(messages) : updater;
    });
    const setSummaries = vi.fn();

    renderHook(() =>
      useMcpListeners({
        setMessages,
        setSummaries,
      })
    );

    // Initial started event
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialStarted",
          payload: { taskId: "task-agent", hasImage: false },
        },
      });
    });

    const stepLog = "```agent-step\nphase: planning\nstep: 1\n```\n";
    const decisionLog = "```agent-decision\nstep: 1\naction: FINISH\nobjective: 設定確認\n```\n";

    // Send agent chunks
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "llmChunk",
          payload: stepLog,
        },
      });
      chatEventListener?.({
        payload: {
          type: "llmChunk",
          payload: decisionLog,
        },
      });
    });

    // Agent finishes with final report
    const finalReport = "目標が達成されました。Fa0/1の設定が正常に完了しました。";
    await act(async () => {
      chatEventListener?.({
        payload: {
          type: "mcpInitialFinished",
          payload: { taskId: "task-agent", content: finalReport },
        },
      });
    });

    // Advance timers until complete
    await act(async () => {
      vi.advanceTimersByTime(10 * 100);
    });

    expect(messages[0].content).toContain(stepLog);
    expect(messages[0].content).toContain(decisionLog);
    expect(messages[0].content).toContain(finalReport);
    expect(messages[0].isToolLoading).toBe(false);
  });
});
