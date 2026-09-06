import { useCallback, useRef, useState } from "react";
import { ipc } from "../../platform";
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

export function useMessageExecution(options: UseMessageExecutionOptions) {
  const [isGenerating, setIsGenerating] = useState(false);
  const executingRef = useRef(false);
  const queueRef = useRef<QueuedMessage[]>([]);

  const execute = useCallback(async (message: string, attachments?: Attachment[]) => {
    executingRef.current = true;
    setIsGenerating(true);
    try {
      await options.handleMcpResponse(message, attachments);
    } catch (error) {
      console.error("Failed to handle MCP response:", error);
    } finally {
      const next = queueRef.current.shift();
      if (next) {
        void execute(next.content, next.attachments);
      } else {
        executingRef.current = false;
        setIsGenerating(false);
      }
    }
  }, [options.handleMcpResponse]);

  const sendMessage = useCallback(async (text?: string, attachments?: Attachment[]) => {
    const messageText = text !== undefined ? text : options.input.trim();
    if (!messageText && (!attachments || attachments.length === 0)) return;

    let sessionId = options.activeSessionId;
    if (!sessionId) {
      const session = await options.createNewSession();
      if (!session) return;
      sessionId = session.id;
    }
    const taskId = crypto.randomUUID();
    const timestamp = new Date().toISOString();
    const foundHosts = [...new Set([
      ...(messageText.match(/@([a-zA-Z0-9.-]+)/g) || []).map((value) => value.slice(1)),
      ...(messageText.match(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g) || []),
    ])];
    if (foundHosts.length) options.updateRecentHosts(foundHosts);
    if (text === undefined) options.setInput("");

    const queued = executingRef.current;
    options.setMessages((previous) => [...previous, {
      role: "user", content: messageText, timestamp, event_type: "UserInput", task_id: taskId,
      status: queued ? "Pending" : undefined, attachments,
    }]);
    if (queued) {
      queueRef.current.push({ content: messageText, taskId, sessionId, attachments });
    } else {
      void execute(messageText, attachments);
    }
  }, [execute, options]);

  const stop = useCallback(async () => {
    try { await ipc.command("stop_llm"); } catch (error) { console.error("Failed to stop LLM:", error); }
    queueRef.current = [];
    executingRef.current = false;
    setIsGenerating(false);
    options.setMessages((previous) => previous.map((message) =>
      message.status === "Running" || message.isToolLoading
        ? { ...message, isToolLoading: false, status: "Failed", summary_text: message.summary_text ? `${message.summary_text} (${options.stoppedLabel})` : options.stoppedLabel } as Message
        : message
    ));
  }, [options]);

  return { isGenerating, setIsGenerating, isCurrentlyGenerating: isGenerating, sendMessage, stop };
}
