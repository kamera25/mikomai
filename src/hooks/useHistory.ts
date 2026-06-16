import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Message, ChatSession, HistoryItem } from "../types";
import i18n from "../i18n";

// Helper to find the first session in the history tree
const findFirstSession = (items: HistoryItem[]): ChatSession | undefined => {
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
const findSession = (items: HistoryItem[], id: string): ChatSession | undefined => {
  for (const item of items) {
    if (item.type === "session" && item.id === id) return item;
    if (item.type === "folder") {
      const found = findSession(item.items, id);
      if (found) return found;
    }
  }
  return undefined;
};

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

export function useHistory() {
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [messages, setMessagesState] = useState<Message[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [modalConfig, setModalConfig] = useState<ModalConfig | null>(null);

  // Load history from backend on mount
  useEffect(() => {
    const initHistory = async () => {
      try {
        const savedHistory: HistoryItem[] = await invoke("load_history");
        if (savedHistory && savedHistory.length > 0) {
          setHistory(savedHistory);
          // Set active session to the first one found
          const firstSession = findFirstSession(savedHistory);
          if (firstSession) {
            setActiveSessionId(firstSession.id);
            setMessagesState(firstSession.messages);
          }
        } else {
          // Initialize with default session if empty
          const defaultId = "session-1";
          const defaultHistory: HistoryItem[] = [
            {
              id: defaultId,
              type: "session",
              title: i18n.t("history.new_session"),
              messages: [],
            },
          ];
          setHistory(defaultHistory);
          setActiveSessionId(defaultId);
          setMessagesState([]);
        }
      } catch (e) {
        console.error("Failed to load history:", e);
      } finally {
        setIsLoaded(true);
      }
    };
    initHistory();
  }, []);

  // Save history to backend whenever it changes
  useEffect(() => {
    if (!isLoaded) return;
    const save = async () => {
      try {
        await invoke("save_history", { history });
      } catch (e) {
        console.error("Failed to save history:", e);
      }
    };
    save();
  }, [history, isLoaded]);

  // Sync messages when active session or history changes (e.g. loaded from backend)
  useEffect(() => {
    const session = findSession(history, activeSessionId);
    if (session) {
      setMessagesState((prev) => {
        // Simple optimization: only update if stringified value changed
        if (JSON.stringify(prev) === JSON.stringify(session.messages)) {
          return prev;
        }
        return session.messages;
      });
    } else {
      setMessagesState((prev) => (prev.length === 0 ? prev : []));
    }
  }, [activeSessionId, history]);

  // Custom wrapped setMessages that updates messages AND history immediately to prevent loop
  const setMessages = (
    update: Message[] | ((prev: Message[]) => Message[])
  ) => {
    setMessagesState((prevMessages) => {
      const nextMessages = typeof update === "function" ? update(prevMessages) : update;

      setHistory((prevHistory) => {
        const updateSessionMessages = (items: HistoryItem[]): HistoryItem[] => {
          return items.map((item) => {
            if (item.id === activeSessionId && item.type === "session") {
              return { ...item, messages: nextMessages };
            }
            if (item.type === "folder") {
              return { ...item, items: updateSessionMessages(item.items) };
            }
            return item;
          });
        };
        return updateSessionMessages(prevHistory);
      });

      return nextMessages;
    });
  };

  // Handlers for UI components using custom modal trigger instead of native prompt/confirm
  const createNewFolder = () => {
    setModalConfig({
      isOpen: true,
      type: "prompt",
      title: i18n.t("history.new_folder"),
      message: i18n.t("history.folder_name_prompt"),
      placeholder: i18n.t("history.folder_name_placeholder"),
      initialValue: "",
      confirmLabel: i18n.t("history.create_label"),
      onConfirm: (folderName) => {
        if (folderName && folderName.trim()) {
          setHistory((prev) => [
            {
              id: `folder-${Date.now()}`,
              type: "folder",
              name: folderName.trim(),
              isOpen: true,
              items: [],
            },
            ...prev,
          ]);
        }
        setModalConfig(null);
      },
      onCancel: () => setModalConfig(null),
    });
  };

  const createNewSession = () => {
    const id = `session-${Date.now()}`;
    setHistory((prev) => [
      {
        id,
        type: "session",
        title: i18n.t("history.new_session"),
        messages: [],
      },
      ...prev,
    ]);
    setActiveSessionId(id);
    setMessagesState([]);
  };

  const toggleFolder = (folderId: string) => {
    setHistory((prev) => {
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
      return toggleNode(prev);
    });
  };

  const switchSession = async (sessionId: string) => {
    setActiveSessionId(sessionId);
    const session = findSession(history, sessionId);
    if (session) {
      setMessagesState(session.messages);
    }
  };

  const renameSession = (sessionId: string, newTitle: string) => {
    if (newTitle && newTitle.trim()) {
      setHistory((prev) => {
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
        return updateSessionTitle(prev);
      });
    }
  };

  const deleteSession = (sessionId: string) => {
    setModalConfig({
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

        let updated = removeSession(history);

        if (updated.length === 0) {
          const defaultId = `session-${Date.now()}`;
          updated = [
            {
              id: defaultId,
              type: "session",
              title: i18n.t("history.new_session"),
              messages: [],
            },
          ];
          setHistory(updated);
          setActiveSessionId(defaultId);
          setMessagesState([]);
          setModalConfig(null);
          return;
        }

        setHistory(updated);

        if (activeSessionId === sessionId) {
          const firstSession = findFirstSession(updated);
          if (firstSession) {
            setActiveSessionId(firstSession.id);
          } else {
            const defaultId = `session-${Date.now()}`;
            setHistory((prev) => [
              {
                id: defaultId,
                type: "session",
                title: i18n.t("history.new_session"),
                messages: [],
              },
              ...prev,
            ]);
            setActiveSessionId(defaultId);
            setMessagesState([]);
          }
        }
        setModalConfig(null);
      },
      onCancel: () => setModalConfig(null),
    });
  };

  const updateSessionRecentIps = (sessionId: string, ips: string[]) => {
    setHistory((prev) => {
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
      return updateIps(prev);
    });
  };

  const activeSession = findSession(history, activeSessionId);

  return {
    history,
    setHistory,
    activeSessionId,
    setActiveSessionId,
    activeSession,
    messages,
    setMessages,
    createNewFolder,
    createNewSession,
    toggleFolder,
    switchSession,
    renameSession,
    deleteSession,
    updateSessionRecentIps,
    isLoaded,
    modalConfig,
  };
}
