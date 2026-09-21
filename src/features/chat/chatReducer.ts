import type { ChatEvent, EventEnvelope } from "../../platform";
import type { Message, SummaryItem } from "../../types";

export interface ChatReducerState {
  messages: Message[];
  summaries: SummaryItem[];
  activeInitialTaskId: string | null;
  activeAnalysisTaskId: string | null;
  lastSequenceByTask: Record<string, number>;
}

export const initialChatReducerState: ChatReducerState = {
  messages: [], summaries: [], activeInitialTaskId: null, activeAnalysisTaskId: null, lastSequenceByTask: {},
};

/** Remove duplicate progress cards from replayed or duplicated IPC events. */
export function dedupeTimelineMessages(messages: Message[]): Message[] {
  const seenInitialProgress = new Set<string>();
  return messages.filter((message) => {
    if (
      message.event_type === "AgentResponse" &&
      message.isToolLoading &&
      message.task_id &&
      message.content.startsWith("考えています")
    ) {
      if (seenInitialProgress.has(message.task_id)) return false;
      seenInitialProgress.add(message.task_id);
    }
    return true;
  });
}

type ChatAction =
  | { type: "event"; event: EventEnvelope }
  | { type: "setMessages"; messages: Message[] }
  | { type: "setSummaries"; summaries: SummaryItem[] };

const text = (value: unknown): string => typeof value === "string" ? value : "";
const taskId = (event: ChatEvent): string | undefined => {
  const payload = event.payload as Record<string, unknown> | undefined;
  return typeof payload?.taskId === "string" ? payload.taskId : undefined;
};

function activeTaskId(state: ChatReducerState, event: ChatEvent): string | undefined {
  return taskId(event) ?? state.activeInitialTaskId ?? state.activeAnalysisTaskId ?? undefined;
}

function updateTask(messages: Message[], id: string, update: (message: Message) => Message): Message[] {
  return messages.map((message) => message.task_id === id ? update(message) : message);
}

export function chatReducer(state: ChatReducerState, action: ChatAction): ChatReducerState {
  if (action.type === "setMessages") return { ...state, messages: action.messages };
  if (action.type === "setSummaries") return { ...state, summaries: action.summaries };

  const event = action.event;
  const id = activeTaskId(state, event);
  if (id && event.sequence !== undefined && event.sequence <= (state.lastSequenceByTask[id] ?? -1)) return state;
  const nextSequence = id && event.sequence !== undefined ? { ...state.lastSequenceByTask, [id]: event.sequence } : state.lastSequenceByTask;
  const next = (changes: Partial<ChatReducerState>): ChatReducerState => ({ ...state, ...changes, lastSequenceByTask: nextSequence });

  switch (event.type) {
    case "mcpToolStarted": {
      if (!id) return state;
      const payload = event.payload as { toolId: string; args?: Record<string, unknown>; resolvedHost?: string };
      const message: Message = { role: "ai", content: "", timestamp: new Date().toISOString(), isToolLoading: true, task_id: id, event_type: "ToolExecution", status: "Running", action_name: payload.toolId, tool_id: payload.toolId, summary_text: payload.toolId, raw_data: null, args: payload.args };
      return next({ messages: [...state.messages, message] });
    }
    case "mcpToolFinished": {
      if (!id) return state;
      const payload = event.payload as { success: boolean; output?: string; savedPath?: string; isCached?: boolean; cacheTime?: string };
      return next({ messages: updateTask(state.messages, id, (message) => message.event_type === "ToolExecution" ? { ...message, isToolLoading: false, status: payload.success ? "Success" : "Failed", raw_data: payload.output ?? "No output provided", saved_path: payload.savedPath, is_cached: payload.isCached, cache_time: payload.cacheTime } : message) });
    }
    case "mcpInitialStarted": {
      if (!id) return state;
      if (state.messages.some((message) =>
        message.task_id === id && message.event_type === "AgentResponse" && message.isToolLoading
      )) return state;
      const payload = event.payload as { hasImage?: boolean };
      const message: Message = { role: "ai", content: payload.hasImage ? "画像を読み込んでいます..." : "考えています...", timestamp: new Date().toISOString(), isToolLoading: true, task_id: id, event_type: "AgentResponse" };
      return next({ messages: [...state.messages, message], activeInitialTaskId: id });
    }
    case "mcpAnalysisStarted": {
      if (!id) return state;
      const analysisTaskId = (event.payload as { analysisTaskId?: string }).analysisTaskId ?? id;
      const message: Message = { role: "ai", content: "解析しています...", timestamp: new Date().toISOString(), isToolLoading: true, isHidden: true, task_id: analysisTaskId, event_type: "AgentResponse" };
      return next({ messages: [...state.messages, message], activeAnalysisTaskId: analysisTaskId });
    }
    case "llmChunk": {
      const target = state.activeInitialTaskId ?? state.activeAnalysisTaskId;
      if (!target) return state;
      const chunk = text(event.payload);
      return next({ messages: updateTask(state.messages, target, (message) => ({ ...message, content: `${message.content}${chunk}`, isToolLoading: true })) });
    }
    case "agentSelected": {
      const target = state.activeAnalysisTaskId ?? state.activeInitialTaskId;
      if (!target) return state;
      const label = text(event.payload);
      return next({ messages: updateTask(state.messages, target, (message) => message.event_type === "AgentResponse" ? { ...message, summary_text: label, isHidden: false } : message) });
    }
    case "mcpInitialFinished": {
      if (!id) return state;
      const content = text((event.payload as { content?: unknown }).content);
      return next({ messages: updateTask(state.messages, id, (message) => ({ ...message, content, isToolLoading: false })), activeInitialTaskId: state.activeInitialTaskId === id ? null : state.activeInitialTaskId });
    }
    case "mcpSummarySaved": {
      if (!id) return state;
      const payload = event.payload as { content?: string; summaryText?: string; summary?: SummaryItem };
      const hidden = payload.content === "PENDING_DECISION" || payload.content === "他の質問への回答を待っています...";
      const summaries = payload.summary ? [...state.summaries, payload.summary].slice(-20) : state.summaries;
      return next({ messages: updateTask(state.messages, id, (message) => {
        if (message.event_type === "ToolExecution") return { ...message, content: payload.content ?? message.content, summary_text: payload.summaryText ?? message.summary_text, isHidden: hidden, isToolLoading: false };
        if (message.event_type === "AgentResponse") return { ...message, content: payload.content ?? message.content, summary_text: payload.summaryText, isHidden: hidden, isToolLoading: false };
        return message;
      }), summaries, activeAnalysisTaskId: state.activeAnalysisTaskId === id ? null : state.activeAnalysisTaskId });
    }
    case "arpYamlSaved": {
      const payload = event.payload as { deviceName?: string; savedPath?: string };
      if (!payload.deviceName || !payload.savedPath) return state;
      return next({ messages: state.messages.map((message) => {
        if (message.event_type !== "ToolExecution" || message.tool_id !== "fetch_arp" || message.saved_path) return message;
        const device = message.args?.deviceName ?? message.args?.device_name;
        return device === payload.deviceName ? { ...message, saved_path: payload.savedPath } : message;
      }) });
    }
    case "routeYamlSaved": {
      const payload = event.payload as { deviceName?: string; savedPath?: string };
      if (!payload.deviceName || !payload.savedPath) return state;
      return next({ messages: state.messages.map((message) => {
        if (message.event_type !== "ToolExecution" || message.tool_id !== "fetch_routing" || message.saved_path) return message;
        const device = message.args?.deviceName ?? message.args?.device_name;
        return device === payload.deviceName ? { ...message, saved_path: payload.savedPath } : message;
      }) });
    }
    default: return state;
  }
}

export type { ChatAction };
