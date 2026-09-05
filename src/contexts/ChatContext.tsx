import React, { createContext, useContext, useReducer, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Message, ChatSession, HistoryItem, SummaryItem, HistoryMutation, HistorySnapshot } from "../types";
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

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
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
        history: state.activeSessionId
          ? updateSessionMessagesInHistory(state.history, state.activeSessionId, nextMessages)
          : state.history,
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
      const updateStatus = (messages: Message[]) =>
        messages.map((msg) => {
          if (msg.task_id === taskId && msg.role === "user") {
            return { ...msg, status } as Message;
          }
          return msg;
        });

      // A queued message can be started before the asynchronous history save
      // has completed.  For the active session, state.messages is therefore
      // newer than the copy currently held in state.history.
      const currentMessages = isCurrentActive ? updateStatus(state.messages) : state.messages;

      const updateMessageStatusInHistory = (items: HistoryItem[]): HistoryItem[] => {
        return items.map((item) => {
          if (item.id === sessionId && item.type === "session") {
            return {
              ...item,
              messages: isCurrentActive ? currentMessages : updateStatus(item.messages),
            };
          }
          if (item.type === "folder") {
            return { ...item, items: updateMessageStatusInHistory(item.items) };
          }
          return item;
        });
      };

      const nextHistory = updateMessageStatusInHistory(state.history);

      return {
        ...state,
        messages: currentMessages,
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
  createNewSession: () => Promise<ChatSession | undefined>;
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
  const historyMutationQueue = useRef<Promise<void>>(Promise.resolve());

  const setInput = (input: string) => dispatch({ type: "SET_INPUT", payload: input });
  const setMessages = (update: Message[] | ((prev: Message[]) => Message[])) =>
    dispatch({ type: "SET_MESSAGES", payload: update });
  const setSummaries = (update: SummaryItem[] | ((prev: SummaryItem[]) => SummaryItem[])) =>
    dispatch({ type: "SET_SUMMARIES", payload: update });

  const activeSession = findSession(state.history, state.activeSessionId);

  const commitHistoryMutation = (mutation: HistoryMutation) => {
    // Streaming responses update messages rapidly. Serializing mutations keeps a
    // slower, older disk write from overwriting a later session update.
    const mutationPromise = historyMutationQueue.current.then(async () => {
      const snapshot = await invoke<HistorySnapshot>("mutate_history", { mutation });
      dispatch({ type: "SET_HISTORY", payload: snapshot.history });
      return snapshot;
    });

    // Keep the queue usable after a failed mutation; the caller still receives
    // the original rejection and can report it.
    historyMutationQueue.current = mutationPromise.then(
      () => undefined,
      () => undefined
    );
    return mutationPromise;
  };

  // Load history on mount
  useEffect(() => {
    const initHistory = async () => {
      try {
        const snapshot = await invoke<HistorySnapshot>("initialize_history");
        const activeSession = findSession(snapshot.history, snapshot.activeSessionId);
        dispatch({
          type: "INIT_HISTORY",
          payload: {
            history: snapshot.history,
            sessionId: snapshot.activeSessionId,
            messages: activeSession?.messages || [],
          },
        });
      } catch (e) {
        console.error("Failed to load history:", e);
        dispatch({ type: "SET_LOADED", payload: true });
      }
    };
    initHistory();
  }, []);

  // Message persistence is owned by the backend. Structural changes use the
  // same mutation API below, so the webview never writes history.json directly.
  useEffect(() => {
    if (!state.isLoaded || !state.activeSessionId) return;
    const save = async () => {
      try {
        await commitHistoryMutation({
          type: "updateSessionMessages",
          sessionId: state.activeSessionId,
          messages: state.messages,
        });
      } catch (e) {
        console.error("Failed to save history:", e);
      }
    };
    save();
  }, [state.messages, state.activeSessionId, state.isLoaded]);

  // Sync messages when active session changes
  const prevActiveSessionIdRef = useRef(state.activeSessionId);
  useEffect(() => {
    if (prevActiveSessionIdRef.current !== state.activeSessionId) {
      prevActiveSessionIdRef.current = state.activeSessionId;
      const session = findSession(state.history, state.activeSessionId);
      if (session) {
        dispatch({ type: "SET_MESSAGES", payload: session.messages });
      }
    }
  }, [state.activeSessionId, state.history]);

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
        onConfirm: async (folderName) => {
          if (folderName && folderName.trim()) {
            try {
              await commitHistoryMutation({ type: "createFolder", name: folderName.trim() });
            } catch (error) {
              console.error("Failed to create folder:", error);
            }
          }
          dispatch({ type: "SET_MODAL_CONFIG", payload: null });
        },
        onCancel: () => dispatch({ type: "SET_MODAL_CONFIG", payload: null }),
      },
    });
  };

  const createNewSession = async () => {
    try {
      const existingIds = new Set(state.history.filter((item): item is ChatSession => item.type === "session").map((item) => item.id));
      const snapshot = await commitHistoryMutation({ type: "createSession", title: i18n.t("history.new_session") });
      const newSession = snapshot.history.find((item) => item.type === "session" && !existingIds.has(item.id));
      if (newSession && newSession.type === "session") {
        dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: newSession.id });
        dispatch({ type: "SET_MESSAGES", payload: [] });
        return newSession;
      }
    } catch (error) {
      console.error("Failed to create session:", error);
    }
    return undefined;
  };

  const toggleFolder = async (folderId: string) => {
    try { await commitHistoryMutation({ type: "toggleFolder", folderId }); }
    catch (error) { console.error("Failed to toggle folder:", error); }
  };

  const switchSession = async (sessionId: string) => {
    dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: sessionId });
    const session = findSession(state.history, sessionId);
    if (session) {
      dispatch({ type: "SET_MESSAGES", payload: session.messages });
    }
  };

  const renameSession = async (sessionId: string, newTitle: string) => {
    if (newTitle && newTitle.trim()) {
      try { await commitHistoryMutation({ type: "renameSession", sessionId, title: newTitle.trim() }); }
      catch (error) { console.error("Failed to rename session:", error); }
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
        onConfirm: async () => {
          try {
            const snapshot = await commitHistoryMutation({ type: "deleteSession", sessionId });
            if (state.activeSessionId === sessionId) {
              dispatch({ type: "SET_ACTIVE_SESSION_ID", payload: snapshot.activeSessionId });
              const next = findSession(snapshot.history, snapshot.activeSessionId);
              dispatch({ type: "SET_MESSAGES", payload: next?.messages || [] });
            }
          } catch (error) {
            console.error("Failed to delete session:", error);
          }
          dispatch({ type: "SET_MODAL_CONFIG", payload: null });
        },
        onCancel: () => dispatch({ type: "SET_MODAL_CONFIG", payload: null }),
      },
    });
  };

  const updateSessionRecentIps = async (sessionId: string, ips: string[]) => {
    try { await commitHistoryMutation({ type: "updateSessionRecentIps", sessionId, recentIps: ips }); }
    catch (error) { console.error("Failed to update recent hosts:", error); }
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
