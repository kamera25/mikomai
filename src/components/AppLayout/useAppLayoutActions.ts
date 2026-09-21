import { useCallback } from "react";
import type { Message } from "../../types";
import type { UIAction } from "../../contexts/UIContext";

interface UseAppLayoutActionsOptions {
  uiDispatch: React.Dispatch<UIAction>;
  activeSession?: { id: string; title: string };
  activeSessionId: string;
  headerTitle: string;
  renameSession: (sessionId: string, title: string) => void;
  isCurrentlyGenerating: boolean;
  setMessages: (update: Message[] | ((previous: Message[]) => Message[])) => void;
  setIsGenerating: React.Dispatch<React.SetStateAction<boolean>>;
  resumeAgent: (taskId: string) => Promise<void>;
}

export function useAppLayoutActions({
  uiDispatch,
  activeSession,
  activeSessionId,
  headerTitle,
  renameSession,
  isCurrentlyGenerating,
  setMessages,
  setIsGenerating,
  resumeAgent,
}: UseAppLayoutActionsOptions) {
  const handleStartRenameHeader = useCallback(() => {
    if (activeSession) uiDispatch({ type: "START_EDITING_HEADER", payload: activeSession.title });
  }, [activeSession, uiDispatch]);
  const handleSaveRenameHeader = useCallback(() => {
    if (activeSessionId && headerTitle.trim()) renameSession(activeSessionId, headerTitle.trim());
    uiDispatch({ type: "STOP_EDITING_HEADER" });
  }, [activeSessionId, headerTitle, renameSession, uiDispatch]);
  const resumeTask = useCallback(
    async (task: { taskId: string; goal: string }) => {
      if (isCurrentlyGenerating) return;
      uiDispatch({ type: "SET_TASK_AUDIT_OPEN", payload: false });
      setMessages((previous) => [
        ...previous,
        {
          role: "user",
          content: `前回の調査を再開: ${task.goal}`,
          timestamp: new Date().toISOString(),
          event_type: "UserInput",
          task_id: crypto.randomUUID(),
        },
      ]);
      setIsGenerating(true);
      try {
        await resumeAgent(task.taskId);
      } catch (reason) {
        setMessages((previous) => [
          ...previous,
          {
            role: "ai",
            content: `調査を再開できませんでした: ${String(reason)}`,
            timestamp: new Date().toISOString(),
            event_type: "SystemMessage",
          },
        ]);
      } finally {
        setIsGenerating(false);
      }
    },
    [isCurrentlyGenerating, resumeAgent, setIsGenerating, setMessages, uiDispatch]
  );
  return { handleStartRenameHeader, handleSaveRenameHeader, resumeTask };
}
