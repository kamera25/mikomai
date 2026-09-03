import { describe, expect, it } from "vitest";
import { chatReducer, ChatState } from "./ChatContext";
import { Message } from "../types";

describe("chatReducer", () => {
  it("keeps a newly queued message when it starts before history persistence finishes", () => {
    const queuedMessage: Message = {
      role: "user",
      content: "ロード中に送信したメッセージ",
      timestamp: "2026-09-03T00:00:00.000Z",
      event_type: "UserInput",
      task_id: "queued-task",
      status: "Pending",
    };
    const state: ChatState = {
      history: [{ id: "session-1", type: "session", title: "新しいセッション", messages: [] }],
      activeSessionId: "session-1",
      messages: [queuedMessage],
      input: "",
      summaries: [],
      modalConfig: null,
      isLoaded: true,
    };

    const next = chatReducer(state, {
      type: "SET_MESSAGE_STATUS",
      payload: { sessionId: "session-1", taskId: "queued-task", status: undefined },
    });

    expect(next.messages).toEqual([{ ...queuedMessage, status: undefined }]);
    expect(next.history[0]).toMatchObject({ messages: [{ ...queuedMessage, status: undefined }] });
  });
});
