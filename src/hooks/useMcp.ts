import { chatService } from "../features/chat/chatService";
import { UseMcpProps } from "./useMcp/types";
import { Attachment } from "../types";
import { useCallback, useEffect, useRef } from "react";
import { chatReducer, initialChatReducerState } from "../features/chat/chatReducer";
import { useChatEvents } from "../features/chat/useChatEvents";
import type { EventEnvelope } from "../platform";

export function useMcp({
  messages,
  setMessages,
  summaries,
  setSummaries,
  historyLimit,
  mcpTimeout = 30,
  updateRecentHosts,
  recentIPs,
}: UseMcpProps) {
  const stateRef = useRef({ ...initialChatReducerState, messages, summaries });
  const publishedMessagesRef = useRef(messages);
  const publishedSummariesRef = useRef(summaries);
  const setMessagesRef = useRef(setMessages);
  const setSummariesRef = useRef(setSummaries);
  const updateRecentHostsRef = useRef(updateRecentHosts);

  // Keep user edits and session switches authoritative while preserving the
  // reducer's task/sequence bookkeeping for backend events.
  useEffect(() => {
    setMessagesRef.current = setMessages;
    setSummariesRef.current = setSummaries;
    updateRecentHostsRef.current = updateRecentHosts;
  }, [setMessages, setSummaries, updateRecentHosts]);
  useEffect(() => {
    if (messages !== publishedMessagesRef.current) {
      stateRef.current = { ...stateRef.current, messages };
      publishedMessagesRef.current = messages;
    }
    if (summaries !== publishedSummariesRef.current) {
      stateRef.current = { ...stateRef.current, summaries };
      publishedSummariesRef.current = summaries;
    }
  }, [messages, summaries]);

  const consumeEvent = useCallback((event: EventEnvelope) => {
    if (event.type === "mcpToolStarted") {
      const host = event.payload?.resolvedHost;
      if (typeof host === "string" && host.trim()) updateRecentHostsRef.current?.([host.trim()]);
    }
    const next = chatReducer(stateRef.current, { type: "event", event });
    if (next === stateRef.current) return;
    stateRef.current = next;
    publishedMessagesRef.current = next.messages;
    publishedSummariesRef.current = next.summaries;
    setMessagesRef.current(next.messages);
    setSummariesRef.current(next.summaries);
  }, []);
  useChatEvents(consumeEvent);

  const handleMcpResponse = async (userMessage: string, attachments?: Attachment[]) => {
    try {
      await chatService.send({
        userMessage,
        summaries,
        recentIps: recentIPs || [],
        historyLimit,
        mcpTimeout,
        attachments,
      });
    } catch (e: unknown) {
      console.error("Failed to execute MCP message:", e);
    }
  };

  return { handleMcpResponse };
}
