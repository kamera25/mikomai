import React, { createContext, useContext, useReducer } from "react";
import { transitionWorkspace } from "../gui/stateMachine";

export interface DiffLine {
  type: "normal" | "insert" | "delete";
  oldLine: number | null;
  newLine: number | null;
  content: string;
}

export interface ConfigDiffData {
  fileName: string;
  additions: number;
  deletions: number;
  diffLines: DiffLine[];
  hostname?: string;
  ip?: string;
}

export interface UIState {
  activePanel: "chat" | "settings" | "connections" | "scheduledTasks" | "taskAudit";
  isSidebarOpen: boolean;
  isSettingsOpen: boolean;
  isConnectionOpen: boolean;
  isScheduledTasksOpen: boolean;
  isTaskAuditOpen: boolean;
  isEditingHeader: boolean;
  headerTitle: string;
  isConfigDiffOpen: boolean;
  configDiffData: ConfigDiffData | null;
}

export type UIAction =
  | { type: "NAVIGATE"; panel: UIState["activePanel"] }
  | { type: "SET_SIDEBAR_OPEN"; payload: boolean }
  | { type: "SET_SETTINGS_OPEN"; payload: boolean }
  | { type: "SET_CONNECTION_OPEN"; payload: boolean }
  | { type: "SET_SCHEDULED_TASKS_OPEN"; payload: boolean }
  | { type: "SET_TASK_AUDIT_OPEN"; payload: boolean }
  | { type: "START_EDITING_HEADER"; payload: string }
  | { type: "SET_HEADER_TITLE"; payload: string }
  | { type: "STOP_EDITING_HEADER" }
  | { type: "SET_CONFIG_DIFF_OPEN"; payload: boolean }
  | { type: "SET_CONFIG_DIFF_DATA"; payload: ConfigDiffData | null };

export const initialUIState: UIState = {
  activePanel: "chat",
  isSidebarOpen: true,
  isSettingsOpen: false,
  isConnectionOpen: false,
  isScheduledTasksOpen: false,
  isTaskAuditOpen: false,
  isEditingHeader: false,
  headerTitle: "",
  isConfigDiffOpen: false,
  configDiffData: null,
};

export function uiReducer(state: UIState, action: UIAction): UIState {
  switch (action.type) {
    case "NAVIGATE":
      return transitionWorkspace(state, action.panel);
    case "SET_SIDEBAR_OPEN":
      return state.activePanel === "chat" ? { ...state, isSidebarOpen: action.payload } : state;
    case "SET_SETTINGS_OPEN":
      return transitionWorkspace(
        state,
        action.payload ? "settings" : state.activePanel === "settings" ? "chat" : state.activePanel
      );
    case "SET_CONNECTION_OPEN":
      return transitionWorkspace(
        state,
        action.payload
          ? "connections"
          : state.activePanel === "connections"
            ? "chat"
            : state.activePanel
      );
    case "SET_SCHEDULED_TASKS_OPEN":
      return transitionWorkspace(
        state,
        action.payload
          ? "scheduledTasks"
          : state.activePanel === "scheduledTasks"
            ? "chat"
            : state.activePanel
      );
    case "SET_TASK_AUDIT_OPEN":
      return transitionWorkspace(
        state,
        action.payload
          ? "taskAudit"
          : state.activePanel === "taskAudit"
            ? "chat"
            : state.activePanel
      );
    case "SET_CONFIG_DIFF_OPEN":
      return state.activePanel === "chat" ? { ...state, isConfigDiffOpen: action.payload } : state;
    case "START_EDITING_HEADER":
      return { ...state, isEditingHeader: true, headerTitle: action.payload };
    case "SET_HEADER_TITLE":
      return { ...state, headerTitle: action.payload };
    case "STOP_EDITING_HEADER":
      return { ...state, isEditingHeader: false };
    case "SET_CONFIG_DIFF_DATA":
      return { ...state, configDiffData: action.payload };
    default:
      return state;
  }
}

interface UIContextType {
  state: UIState;
  dispatch: React.Dispatch<UIAction>;
}

const UIContext = createContext<UIContextType | undefined>(undefined);

export const UIProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, dispatch] = useReducer(uiReducer, initialUIState);

  return <UIContext.Provider value={{ state, dispatch }}>{children}</UIContext.Provider>;
};

export const useUIContext = () => {
  const context = useContext(UIContext);
  if (!context) {
    throw new Error("useUIContext must be used within a UIProvider");
  }
  return context;
};
