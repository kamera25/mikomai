import { useCallback, useRef } from "react";
import type { Message } from "../../types";
import type { UIAction, UIState } from "../../contexts/UIContext";

interface UseAppLayoutActionsOptions {
  uiState: UIState;
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
  uiState,
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
  const isComposingHeader = useRef(false);
  const handleStartRenameHeader = useCallback(() => {
    if (activeSession) uiDispatch({ type: "START_EDITING_HEADER", payload: activeSession.title });
  }, [activeSession, uiDispatch]);
  const handleSaveRenameHeader = useCallback(() => {
    if (activeSessionId && headerTitle.trim()) renameSession(activeSessionId, headerTitle.trim());
    uiDispatch({ type: "STOP_EDITING_HEADER" });
  }, [activeSessionId, headerTitle, renameSession, uiDispatch]);
  const panelSetter = useCallback((type: UIAction["type"], valueOrFn: boolean | ((value: boolean) => boolean), current: boolean) => {
    if (type === "SET_SIDEBAR_OPEN") return uiDispatch({ type, payload: typeof valueOrFn === "function" ? valueOrFn(current) : valueOrFn });
    if (type === "SET_SETTINGS_OPEN") return uiDispatch({ type, payload: typeof valueOrFn === "function" ? valueOrFn(current) : valueOrFn });
    if (type === "SET_CONNECTION_OPEN") return uiDispatch({ type, payload: typeof valueOrFn === "function" ? valueOrFn(current) : valueOrFn });
    if (type === "SET_SCHEDULED_TASKS_OPEN") return uiDispatch({ type, payload: typeof valueOrFn === "function" ? valueOrFn(current) : valueOrFn });
    return uiDispatch({ type: "SET_TASK_AUDIT_OPEN", payload: typeof valueOrFn === "function" ? valueOrFn(current) : valueOrFn });
  }, [uiDispatch]);
  const handleSetConnectionOpen = useCallback((value: boolean | ((previous: boolean) => boolean)) => panelSetter("SET_CONNECTION_OPEN", value, uiState.isConnectionOpen), [panelSetter, uiState.isConnectionOpen]);
  const handleSetScheduledTasksOpen = useCallback((value: boolean | ((previous: boolean) => boolean)) => panelSetter("SET_SCHEDULED_TASKS_OPEN", value, uiState.isScheduledTasksOpen), [panelSetter, uiState.isScheduledTasksOpen]);
  const handleSetSettingsOpen = useCallback((value: boolean | ((previous: boolean) => boolean)) => panelSetter("SET_SETTINGS_OPEN", value, uiState.isSettingsOpen), [panelSetter, uiState.isSettingsOpen]);
  const handleSetTaskAuditOpen = useCallback((value: boolean | ((previous: boolean) => boolean)) => panelSetter("SET_TASK_AUDIT_OPEN", value, uiState.isTaskAuditOpen), [panelSetter, uiState.isTaskAuditOpen]);
  const resumeTask = useCallback(async (task: { taskId: string; goal: string }) => {
    if (isCurrentlyGenerating) return;
    uiDispatch({ type: "SET_TASK_AUDIT_OPEN", payload: false });
    setMessages((previous) => [...previous, { role: "user", content: `前回の調査を再開: ${task.goal}`, timestamp: new Date().toISOString(), event_type: "UserInput", task_id: crypto.randomUUID() }]);
    setIsGenerating(true);
    try { await resumeAgent(task.taskId); }
    catch (reason) { setMessages((previous) => [...previous, { role: "ai", content: `調査を再開できませんでした: ${String(reason)}`, timestamp: new Date().toISOString(), event_type: "SystemMessage" }]); }
    finally { setIsGenerating(false); }
  }, [isCurrentlyGenerating, resumeAgent, setIsGenerating, setMessages, uiDispatch]);
  return { isComposingHeaderRef: isComposingHeader, handleStartRenameHeader, handleSaveRenameHeader, handleSetConnectionOpen, handleSetScheduledTasksOpen, handleSetSettingsOpen, handleSetTaskAuditOpen, resumeTask };
}
