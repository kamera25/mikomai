import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Message, ChatSession, HistoryItem } from "../types";

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

export function useHistory() {
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);

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
          }
        } else {
          // Initialize with default session if empty
          const defaultId = "session-1";
          const defaultHistory: HistoryItem[] = [
            {
              id: defaultId,
              type: "session",
              title: "新しいセッション",
              messages: [],
            },
          ];
          setHistory(defaultHistory);
          setActiveSessionId(defaultId);
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

  // Sync messages when active session changes
  useEffect(() => {
    const session = findSession(history, activeSessionId);
    if (session) {
      setMessages(session.messages);
    }
  }, [activeSessionId]);

  // Update history when messages change
  useEffect(() => {
    if (messages.length === 0) return;
    setHistory((prev) => {
      const updateSessionMessages = (items: HistoryItem[]): HistoryItem[] => {
        return items.map((item) => {
          if (item.id === activeSessionId && item.type === "session") {
            return { ...item, messages };
          }
          if (item.type === "folder") {
            return { ...item, items: updateSessionMessages(item.items) };
          }
          return item;
        });
      };
      return updateSessionMessages(prev);
    });
  }, [messages, activeSessionId]);

  // Handlers for UI components
  const createNewFolder = () => {
    const folderName = prompt("フォルダ名を入力してください");
    if (folderName) {
      setHistory((prev) => [
        {
          id: `folder-${Date.now()}`,
          type: "folder",
          name: folderName,
          isOpen: true,
          items: [],
        },
        ...prev,
      ]);
    }
  };

  const createNewSession = () => {
    const id = `session-${Date.now()}`;
    setHistory((prev) => [
      {
        id,
        type: "session",
        title: "新しいセッション",
        messages: [],
      },
      ...prev,
    ]);
    setActiveSessionId(id);
    setMessages([]);
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
      setMessages(session.messages);
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
    if (confirm("このセッションを削除してもよろしいですか？")) {
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
            title: "新しいセッション",
            messages: [],
          },
        ];
        setHistory(updated);
        setActiveSessionId(defaultId);
        setMessages([]);
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
              title: "新しいセッション",
              messages: [],
            },
            ...prev,
          ]);
          setActiveSessionId(defaultId);
          setMessages([]);
        }
      }
    }
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
  };
}
