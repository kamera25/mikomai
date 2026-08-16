import React, { createContext, useContext, useReducer, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Message, ChatSession, HistoryItem, SummaryItem } from "../types";
import { useSettingsContext } from "./SettingsContext";
import i18n from "../i18n";

export interface ModalConfig {
  isOpen: boolean;
  type: "confirm" | "prompt";
  title: string;
  message: string;
  placeholder?: string;
  initialValue?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: (val?: string) => void;
  onCancel: () => void;
}

export interface ChatState {
  history: HistoryItem[];
  activeSessionId: string;
  messages: Message[];
  input: string;
  summaries: SummaryItem[];
  modalConfig: ModalConfig | null;
  isLoaded: boolean;
}

export type ChatAction =
  | { type: "INIT_HISTORY"; payload: { history: HistoryItem[]; sessionId: string; messages: Message[] } }
  | { type: "SET_HISTORY"; payload: HistoryItem[] }
  | { type: "SET_ACTIVE_SESSION_ID"; payload: string }
  | { type: "SET_MESSAGES"; payload: Message[] | ((prev: Message[]) => Message[]) }
  | { type: "SET_INPUT"; payload: string }
  | { type: "SET_SUMMARIES"; payload: SummaryItem[] | ((prev: SummaryItem[]) => SummaryItem[]) }
  | { type: "SET_MODAL_CONFIG"; payload: ModalConfig | null }
  | { type: "SET_LOADED"; payload: boolean }
  | { type: "SET_MESSAGE_STATUS"; payload: { sessionId: string; taskId: string; status: "Pending" | undefined } };

const initialState: ChatState = {
  history: [],
  activeSessionId: "",
  messages: [],
  input: "",
  summaries: [],
  modalConfig: null,
  isLoaded: false,
};

// Helper to find the first session in the history tree
export const findFirstSession = (items: HistoryItem[]): ChatSession | undefined => {
  for (const item of items) {
    if (item.type === "session") return item;
    if (item.type === "folder") {
      const found = findFirstSession(item.items);
      if (found) return found;
    }
  }
  return undefined;
};

// Helper to find a specific session in the history tree by ID
export const findSession = (items: HistoryItem[], id: string): ChatSession | undefined => {
  for (const item of items) {
    if (item.type === "session" && item.id === id) return item;
    if (item.type === "folder") {
      const found = findSession(item.items, id);
      if (found) return found;
    }
  }
  return undefined;
};

// Helper to update session messages in the history tree
export const updateSessionMessagesInHistory = (
  items: HistoryItem[],
  sessionId: string,
  messages: Message[]
): HistoryItem[] => {
  return items.map((item) => {
    if (item.id === sessionId && item.type === "session") {
      return { ...item, messages };
    }
    if (item.type === "folder") {
      return { ...item, items: updateSessionMessagesInHistory(item.items, sessionId, messages) };
    }
    return item;
  });
};

function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "INIT_HISTORY":
      return {
        ...state,
        history: action.payload.history,
        activeSessionId: action.payload.sessionId,
        messages: action.payload.messages,
        isLoaded: true,
      };
    case "SET_HISTORY":
      return { ...state, history: action.payload };
    case "SET_ACTIVE_SESSION_ID":
      return { ...state, activeSessionId: action.payload };
    case "SET_MESSAGES": {
      const nextMessages = typeof action.payload === "function"
        ? (action.payload as (prev: Message[]) => Message[])(state.messages)
        : action.payload;

      return {
        ...state,
        messages: nextMessages,
      };
    }
    case "SET_INPUT":
      return { ...state, input: action.payload };
    case "SET_SUMMARIES": {
      const nextSummaries = typeof action.payload === "function"
        ? (action.payload as (prev: SummaryItem[]) => SummaryItem[])(state.summaries)
        : action.payload;
      return { ...state, summaries: nextSummaries };
    }
    case "SET_MODAL_CONFIG":
      return { ...state, modalConfig: action.payload };
    case "SET_LOADED":
      return { ...state, isLoaded: action.payload };
    case "SET_MESSAGE_STATUS": {
      const { sessionId, taskId, status } = action.payload;
      const isCurrentActive = state.activeSessionId === sessionId;
      let updatedMessages: Message[] | null = null;

      const updateMessageStatusInHistory = (items: HistoryItem[]): HistoryItem[] => {
        return items.map((item) => {
          if (item.id === sessionId && item.type === "session") {
            updatedMessages = item.messages.map((msg) => {
              if (msg.task_id === taskId && msg.role === "user") {
                return { ...msg, status } as Message;
              }
              return msg;
            });
            return { ...item, messages: updatedMessages };
          }
          if (item.type === "folder") {
            return { ...item, items: updateMessageStatusInHistory(item.items) };
          }
          return item;
        });
      };

      const nextHistory = updateMessageStatusInHistory(state.history);
      const nextMessages = isCurrentActive && updatedMessages
        ? updatedMessages
        : state.messages;

      return {
        ...state,
        messages: nextMessages,
        history: nextHistory,
      };
    }
    default:
      return state;
  }
}

interface ChatContextType {
  state: ChatState;
  dispatch: React.Dispatch<ChatAction>;
  createNewFolder: () => void;
  createNewSession: () => void;
  toggleFolder: (folderId: string) => void;
  switchSession: (sessionId: string) => void;
  renameSession: (sessionId: string, newTitle: string) => void;
  deleteSession: (sessionId: string) => void;
  updateSessionRecentIps: (sessionId: string, ips: string[]) => void;
  setInput: (input: string) => void;
  setMessages: (update: Message[] | ((prev: Message[]) => Message[])) => void;
  setSummaries: (update: SummaryItem[] | ((prev: SummaryItem[]) => SummaryItem[])) => void;
  activeSession: ChatSession | undefined;
}

const ChatContext = createContext<ChatContextType | undefined>(undefined);

export const ChatProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, dispatch] = useReducer(chatReducer, initialState);
  const { recentIPs, setRecentIPs } = useSettingsContext();

  const setInput = (input: string) => dispatch({ type: "SET_INPUT", payload: input });
  const setMessages = (update: Message[] | ((prev: Message[]) => Message[])) =>
    dispatch({ type: "SET_MESSAGES", payload: update });
  const setSummaries = (update: SummaryItem[] | ((prev: SummaryItem[]) => SummaryItem[])) =>
    dispatch({ type: "SET_SUMMARIES", payload: update });

  const activeSession = findSession(state.history, state.activeSessionId);

  // Load history on mount
  useEffect(() => {
    const initHistory = async () => {
      try {
        const savedHistory: HistoryItem[] = await invoke("load_history");
        if (savedHistory && savedHistory.length > 0) {
          const firstSession = findFirstSession(savedHistory);
          const firstSessionId = firstSession ? firstSession.id : "";
          const firstSessionMessages = firstSession ? firstSession.messages : [];
          dispatch({
            type: "INIT_HISTORY",
            payload: { history: savedHistory, sessionId: firstSessionId, messages: firstSessionMessages },
          });
        } else {
          const defaultId = crypto.randomUUID();
          const defaultHistory: HistoryItem[] = [
            {
              id: defaultId,
              type: "session",
              title: i18n.t("history.new_session"),
              messages: [],
            },
          ];
          dispatch({
            type: "INIT_HISTORY",
            payload: { history: defaultHistory, sessionId: defaultId, messages: [] },
          });
        }
      } catch (e) {
        console.error("Failed to load history:", e);
        dispatch({ type: "SET_LOADED", payload: true });
      }
    };
    initHistory();
  }, []);

  // Save history whenever history or active session messages change
  useEffect(() => {
    if (!state.isLoaded) return;
    const save = async () => {
      try {
        const historyToSave = updateSessionMessagesInHistory(
          state.history,
          state.activeSessionId,
          state.messages
        );
        await invoke("save_history", { history: historyToSave });
      } catch (e) {
        console.error("Failed to save history:", e);
      }
    };
    save();
  }, [state.history, state.messages, state.activeSessionId, state.isLoaded]);

  // Sync messages when active session changes
  useEffect(() => {
    const session = findSession(state.history, state.activeSessionId);
    if (session) {
      dispatch({ type: "SET_MESSAGES", payload: session.messages });
    }
  }, [state.activeSessionId]);

  // Sync recentIPs with the active session's cached recent IPs when session changes
  useEffect(() => {
    const sessionIps = activeSession?.recentIps || [];
    const isDifferent =
      sessionIps.length !== recentIPs.length ||
      sessionIps.some((val, idx) => val !== recentIPs[idx]);
    if (isDifferent) {
      setRecentIPs(sessionIps);
    }
  }, [state.activeSessionId, activeSession?.recentIps, recentIPs, setRecentIPs]);

  // Load summaries from backend
  useEffect(() => {
    const initSummaries = async () => {
      try {
        const savedSummaries = await invoke<SummaryItem[]>("load_summaries");
        setSummaries(savedSummaries || []);
      } catch (e) {
        console.error("Failed to load summaries:", e);
      }
    };
    initSummaries();
  }, []);

  // Actions wrapped as helpers
  const createNewFolder = () => {
    dispatch({
      type: "SET_MODAL_CONFIG",
      payload: {
        isOpen: true,
        type: "prompt",
        title: i18n.t("history.new_folder"),
        message: i18n.t("history.folder_name_prompt"),
        placeholder: i18n.t("history.folder_name_placeholder"),
        initialValue: "",
        confirmLabel: i18n.t("history.create_label"),
        onConfirm: (folderName) => {
          if (folderName && folderName.trim()) {
            dispatch({
              type: "SET_HISTORY",
              payload: [
                {
                  id: crypto.randomUUID(),
                  type: "folder",
                  name: folderName.trim(),
                  isOpen: true,
                  items: [],
                },
                ...state.history,
              ],
            });
          }
          dispatch({ type: "SET_MODAL_CONFIG", payload: null });
        },
        onCancel: () => dispatch({ type: "SET_MODAL_CONFIG", payload: null }),
      },
    });
  };

  const createNewSession = () => {
    const id = crypto.randomUUID();
    const newSession: ChatSession = {
      id,
      type: "session",
      title: i18n.t("history.new_session"),
      messages: [],
    };
    dispatch({ type: "SET_HISTORY", payload: [newSession, ...state.history] });
    dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: id });
    dispatch({ type: "SET_MESSAGES", payload: [] });
  };

  const toggleFolder = (folderId: string) => {
    const toggleNode = (items: HistoryItem[]): HistoryItem[] => {
      return items.map((item) => {
        if (item.type === "folder") {
          if (item.id === folderId) {
            return { ...item, isOpen: !item.isOpen };
          }
          return { ...item, items: toggleNode(item.items) };
        }
        return item;
      });
    };
    dispatch({ type: "SET_HISTORY", payload: toggleNode(state.history) });
  };

  const switchSession = async (sessionId: string) => {
    dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: sessionId });
    const session = findSession(state.history, sessionId);
    if (session) {
      dispatch({ type: "SET_MESSAGES", payload: session.messages });
    }
  };

  const renameSession = (sessionId: string, newTitle: string) => {
    if (newTitle && newTitle.trim()) {
      const updateSessionTitle = (items: HistoryItem[]): HistoryItem[] => {
        return items.map((item) => {
          if (item.id === sessionId && item.type === "session") {
            return { ...item, title: newTitle.trim() };
          }
          if (item.type === "folder") {
            return { ...item, items: updateSessionTitle(item.items) };
          }
          return item;
        });
      };
      dispatch({ type: "SET_HISTORY", payload: updateSessionTitle(state.history) });
    }
  };

  const deleteSession = (sessionId: string) => {
    dispatch({
      type: "SET_MODAL_CONFIG",
      payload: {
        isOpen: true,
        type: "confirm",
        title: i18n.t("history.delete_session_title"),
        message: i18n.t("history.delete_session_msg"),
        confirmLabel: i18n.t("common.delete"),
        onConfirm: () => {
          const removeSession = (items: HistoryItem[]): HistoryItem[] => {
            return items
              .filter((item) => item.id !== sessionId)
              .map((item) => {
                if (item.type === "folder") {
                  return { ...item, items: removeSession(item.items) };
                }
                return item;
              });
          };

          let updated = removeSession(state.history);

          if (updated.length === 0) {
            const defaultId = crypto.randomUUID();
            updated = [
              {
                id: defaultId,
                type: "session",
                title: i18n.t("history.new_session"),
                messages: [],
              },
            ];
            dispatch({ type: "SET_HISTORY", payload: updated });
            dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: defaultId });
            dispatch({ type: "SET_MESSAGES", payload: [] });
            dispatch({ type: "SET_MODAL_CONFIG", payload: null });
            return;
          }

          dispatch({ type: "SET_HISTORY", payload: updated });

          if (state.activeSessionId === sessionId) {
            const firstSession = findFirstSession(updated);
            if (firstSession) {
              dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: firstSession.id });
            } else {
              const defaultId = crypto.randomUUID();
              dispatch({
                type: "SET_HISTORY",
                payload: [
                  {
                    id: defaultId,
                    type: "session",
                    title: i18n.t("history.new_session"),
                    messages: [],
                  },
                  ...updated,
                ],
              });
              dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: defaultId });
              dispatch({ type: "SET_MESSAGES", payload: [] });
            }
          }
          dispatch({ type: "SET_MODAL_CONFIG", payload: null });
        },
        onCancel: () => dispatch({ type: "SET_MODAL_CONFIG", payload: null }),
      },
    });
  };

  const updateSessionRecentIps = (sessionId: string, ips: string[]) => {
    const updateIps = (items: HistoryItem[]): HistoryItem[] => {
      return items.map((item) => {
        if (item.id === sessionId && item.type === "session") {
          return { ...item, recentIps: ips };
        }
        if (item.type === "folder") {
          return { ...item, items: updateIps(item.items) };
        }
        return item;
      });
    };
    dispatch({ type: "SET_HISTORY", payload: updateIps(state.history) });
  };

  return (
    <ChatContext.Provider
      value={{
        state,
        dispatch,
        createNewFolder,
        createNewSession,
        toggleFolder,
        switchSession,
        renameSession,
        deleteSession,
        updateSessionRecentIps,
        setInput,
        setMessages,
        setSummaries,
        activeSession,
      }}
    >
      {children}
    </ChatContext.Provider>
  );
};

export const useChatContext = () => {
  const context = useContext(ChatContext);
  if (!context) {
    throw new Error("useChatContext must be used within a ChatProvider");
  }
  return context;
};
