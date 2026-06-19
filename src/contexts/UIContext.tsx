import React, { createContext, useContext, useReducer } from "react";

export interface UIState {
  isSidebarOpen: boolean;
  isSettingsOpen: boolean;
  isConnectionOpen: boolean;
  isScheduledTasksOpen: boolean;
  isEditingHeader: boolean;
  headerTitle: string;
}

export type UIAction =
  | { type: "SET_SIDEBAR_OPEN"; payload: boolean }
  | { type: "SET_SETTINGS_OPEN"; payload: boolean }
  | { type: "SET_CONNECTION_OPEN"; payload: boolean }
  | { type: "SET_SCHEDULED_TASKS_OPEN"; payload: boolean }
  | { type: "START_EDITING_HEADER"; payload: string }
  | { type: "SET_HEADER_TITLE"; payload: string }
  | { type: "STOP_EDITING_HEADER" };

const initialState: UIState = {
  isSidebarOpen: true,
  isSettingsOpen: false,
  isConnectionOpen: false,
  isScheduledTasksOpen: false,
  isEditingHeader: false,
  headerTitle: "",
};

function uiReducer(state: UIState, action: UIAction): UIState {
  switch (action.type) {
    case "SET_SIDEBAR_OPEN":
      return { ...state, isSidebarOpen: action.payload };
    case "SET_SETTINGS_OPEN": {
      const nextOpen = action.payload;
      return {
        ...state,
        isSettingsOpen: nextOpen,
        ...(nextOpen ? { isConnectionOpen: false, isScheduledTasksOpen: false, isSidebarOpen: false } : {}),
      };
    }
    case "SET_CONNECTION_OPEN": {
      const nextOpen = action.payload;
      return {
        ...state,
        isConnectionOpen: nextOpen,
        ...(nextOpen ? { isSettingsOpen: false, isScheduledTasksOpen: false, isSidebarOpen: false } : {}),
      };
    }
    case "SET_SCHEDULED_TASKS_OPEN": {
      const nextOpen = action.payload;
      return {
        ...state,
        isScheduledTasksOpen: nextOpen,
        ...(nextOpen ? { isSettingsOpen: false, isConnectionOpen: false, isSidebarOpen: false } : {}),
      };
    }
    case "START_EDITING_HEADER":
      return { ...state, isEditingHeader: true, headerTitle: action.payload };
    case "SET_HEADER_TITLE":
      return { ...state, headerTitle: action.payload };
    case "STOP_EDITING_HEADER":
      return { ...state, isEditingHeader: false };
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
  const [state, dispatch] = useReducer(uiReducer, initialState);

  return <UIContext.Provider value={{ state, dispatch }}>{children}</UIContext.Provider>;
};

export const useUIContext = () => {
  const context = useContext(UIContext);
  if (!context) {
    throw new Error("useUIContext must be used within a UIProvider");
  }
  return context;
};
