import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, COMMANDS } from "../../platform";
import { Attachment, Message } from "../../types";

interface QueuedMessage {
  content: string;
  taskId: string;
  sessionId: string;
  attachments?: Attachment[];
}

interface UseMessageExecutionOptions {
  input: string;
  setInput: (value: string) => void;
  activeSessionId: string | null;
  createNewSession: () => Promise<{ id: string } | null | undefined>;
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  updateRecentHosts: (hosts: string[]) => void;
  handleMcpResponse: (message: string, attachments?: Attachment[]) => Promise<void>;
  stoppedLabel: string;
}

export function useMessageExecution({
  input,
  setInput,
  activeSessionId,
  createNewSession,
  setMessages,
  updateRecentHosts,
  handleMcpResponse,
  stoppedLabel,
}: UseMessageExecutionOptions) {
  const [isGenerating, setIsGenerating] = useState(false);
  const executingRef = useRef(false);
  const queueRef = useRef<QueuedMessage[]>([]);
  const executeRef = useRef<((message: string, attachments?: Attachment[]) => Promise<void>) | undefined>(undefined);

  const execute = useCallback(async (message: string, attachments?: Attachment[]) => {
    executingRef.current = true;
    setIsGenerating(true);
    try {
      await handleMcpResponse(message, attachments);
    } catch (error) {
      console.error("Failed to handle MCP response:", error);
    } finally {
      const next = queueRef.current.shift();
      if (next) {
        void executeRef.current?.(next.content, next.attachments);
      } else {
        executingRef.current = false;
        setIsGenerating(false);
      }
    }
  }, [handleMcpResponse]);
  useEffect(() => {
    executeRef.current = execute;
  }, [execute]);

  const sendMessage = useCallback(async (text?: string, attachments?: Attachment[]) => {
    const messageText = text !== undefined ? text : input.trim();
    if (!messageText && (!attachments || attachments.length === 0)) return;

    let sessionId = activeSessionId;
    if (!sessionId) {
      const session = await createNewSession();
      if (!session) return;
      sessionId = session.id;
    }
    const taskId = crypto.randomUUID();
    const timestamp = new Date().toISOString();
    const foundHosts = [...new Set([
      ...(messageText.match(/@([a-zA-Z0-9.-]+)/g) || []).map((value) => value.slice(1)),
      ...(messageText.match(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g) || []),
    ])];
    if (foundHosts.length) updateRecentHosts(foundHosts);
    if (text === undefined) setInput("");

    const queued = executingRef.current;
    setMessages((previous) => [...previous, {
      role: "user", content: messageText, timestamp, event_type: "UserInput", task_id: taskId,
      status: queued ? "Pending" : undefined, attachments,
    }]);
    if (queued) {
      queueRef.current.push({ content: messageText, taskId, sessionId, attachments });
    } else {
      void execute(messageText, attachments);
    }
  }, [activeSessionId, createNewSession, execute, input, setInput, setMessages, updateRecentHosts]);

  const stop = useCallback(async () => {
    try { await ipc.command(COMMANDS.stopLlm); } catch (error) { console.error("Failed to stop LLM:", error); }
    queueRef.current = [];
    executingRef.current = false;
    setIsGenerating(false);
    setMessages((previous) => previous.map((message) =>
      message.status === "Running" || message.isToolLoading
        ? { ...message, isToolLoading: false, status: "Failed", summary_text: message.summary_text ? `${message.summary_text} (${stoppedLabel})` : stoppedLabel } as Message
        : message
    ));
  }, [setMessages, stoppedLabel]);

  return { isGenerating, setIsGenerating, isCurrentlyGenerating: isGenerating, sendMessage, stop };
}
