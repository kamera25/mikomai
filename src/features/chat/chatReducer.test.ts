import { describe, expect, it } from "vitest";
import { chatReducer, initialChatReducerState } from "./chatReducer";
import type { ChatEvent } from "../../platform";

const event = (type: ChatEvent["type"], taskId: string, payload: ChatEvent["payload"], sequence?: number): ChatEvent => ({ type, taskId, payload, sequence } as ChatEvent);

describe("chatReducer", () => {
  it("ignores duplicate and out-of-order events per task", () => {
    const started = chatReducer(initialChatReducerState, { type: "event", event: event("mcpInitialStarted", "task-1", { taskId: "task-1" }, 2) });
    const duplicate = chatReducer(started, { type: "event", event: event("mcpInitialFinished", "task-1", { taskId: "task-1", content: "new" }, 2) });
    const older = chatReducer(started, { type: "event", event: event("mcpInitialFinished", "task-1", { taskId: "task-1", content: "old" }, 1) });
    expect(duplicate).toEqual(started);
    expect(older).toEqual(started);
  });

  it("preserves event order for streaming content and finishes the task", () => {
    let state = chatReducer(initialChatReducerState, { type: "event", event: event("mcpInitialStarted", "task-1", { taskId: "task-1" }) });
    state = chatReducer(state, { type: "event", event: event("llmChunk", "task-1", "hello") });
    state = chatReducer(state, { type: "event", event: event("llmChunk", "task-1", " world") });
    state = chatReducer(state, { type: "event", event: event("mcpInitialFinished", "task-1", { taskId: "task-1", content: "hello world" }) });
    expect(state.messages[0].content).toBe("hello world");
    expect(state.messages[0].isToolLoading).toBe(false);
  });

  it("keeps at most twenty summaries", () => {
    let state = initialChatReducerState;
    for (let i = 0; i < 21; i++) state = chatReducer(state, { type: "event", event: event("mcpSummarySaved", `task-${i}`, { taskId: `task-${i}`, summary: { timestamp: String(i), content: String(i) }, content: String(i) }) });
    expect(state.summaries).toHaveLength(20);
    expect(state.summaries[0].content).toBe("1");
  });
});
