import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "katex/dist/katex.min.css";
import { useTranslation } from "react-i18next";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "./components/ScheduledTasksPanel";
import "./App.css";

import { SummaryItem } from "./types";
import { Chat } from "./components/Chat/Chat";
import { ChatInput } from "./components/ChatInput/ChatInput";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { ActivityBar } from "./components/ActivityBar/ActivityBar";
import { useMcp } from "./hooks/useMcp";
import { useHistory } from "./hooks/useHistory";
import { StatusBar } from "./components/StatusBar/StatusBar";
import { useSettingsContext } from "./contexts/SettingsContext";
import { useModel } from "./hooks/useModel";
import { useHostSuggestions } from "./hooks/useHostSuggestions";
import { CustomModal } from "./components/CustomModal";
import { SidebarIcon, ServerIcon } from "./components/Icons";

function App() {
  const { t } = useTranslation();
  const {
    history,
    activeSessionId,
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
    modalConfig,
  } = useHistory();

  const [isEditingHeader, setIsEditingHeader] = useState(false);
  const [headerTitle, setHeaderTitle] = useState("");

  const isComposingHeader = useRef(false);

  const handleStartRenameHeader = () => {
    if (activeSession) {
      setHeaderTitle(activeSession.title);
      setIsEditingHeader(true);
    }
  };

  const handleSaveRenameHeader = () => {
    if (activeSessionId && headerTitle.trim()) {
      renameSession(activeSessionId, headerTitle.trim());
    }
    setIsEditingHeader(false);
  };

  const { historyLimit, modelPath, mcpTimeout, recentIPs, setRecentIPs, saveAllSettings } =
    useSettingsContext();

  const { modelStatus, handleLoadModel } = useModel(modelPath);

  const [input, setInput] = useState("");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isConnectionOpen, setIsConnectionOpen] = useState(false);
  const [isScheduledTasksOpen, setIsScheduledTasksOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [summaries, setSummaries] = useState<SummaryItem[]>([]);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const {
    availableHosts,
    showSuggestions,
    setShowSuggestions,
    filteredSuggestions,
    setFilteredSuggestions,
    suggestionIndex,
    setSuggestionIndex,
    setCursorPos,
    fetchHosts,
    updateRecentHosts,
    handleSelectSuggestion,
  } = useHostSuggestions({
    recentIPs,
    setRecentIPs,
    activeSessionId,
    updateSessionRecentIps,
    saveAllSettings,
    input,
    setInput,
    textareaRef,
  });

  // Sync recentIPs with the active session's cached recent IPs when session changes
  useEffect(() => {
    const sessionIps = activeSession?.recentIps || [];
    const isDifferent =
      sessionIps.length !== recentIPs.length ||
      sessionIps.some((val, idx) => val !== recentIPs[idx]);
    if (isDifferent) {
      setRecentIPs(sessionIps);
    }
  }, [activeSessionId, activeSession?.recentIps, recentIPs, setRecentIPs]);

  const { handleMcpResponse } = useMcp({
    messages,
    setMessages,
    summaries,
    setSummaries,
    historyLimit,
    mcpTimeout,
    updateRecentHosts,
    recentIPs,
  });

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

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

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [input]);

  // Scroll to bottom when messages change
  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  // Close history sidebar when leaving chat screen
  useEffect(() => {
    if (isSettingsOpen || isConnectionOpen || isScheduledTasksOpen) {
      setIsSidebarOpen(false);
    }
  }, [isSettingsOpen, isConnectionOpen, isScheduledTasksOpen, setIsSidebarOpen]);

  const formatMessageTime = (isoString?: string) => {
    if (!isoString) return "";
    const date = new Date(isoString);
    const now = new Date();

    const isToday =
      date.getFullYear() === now.getFullYear() &&
      date.getMonth() === now.getMonth() &&
      date.getDate() === now.getDate();

    const timeStr = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

    if (isToday) {
      return timeStr;
    } else {
      const dateStr = date
        .toLocaleDateString([], { year: "numeric", month: "2-digit", day: "2-digit" })
        .replace(/\//g, "/");
      return `${dateStr} ${timeStr}`;
    }
  };

  const handleSend = async () => {
    if (!input.trim()) return;

    const userMessage = input.trim();
    const timestamp = new Date().toISOString();

    // Extract IP addresses and @hostnames to remember
    const ipRegex = /\b(?:\d{1,3}\.){3}\d{1,3}\b/g;
    const mentionRegex = /@([a-zA-Z0-9.-]+)/g;

    const foundIPs = userMessage.match(ipRegex) || [];
    const foundMentions = Array.from(userMessage.matchAll(mentionRegex)).map((m) => m[1]);

    const allFound = [...new Set([...foundMentions, ...foundIPs])];

    if (allFound.length > 0) {
      updateRecentHosts(allFound);
    }

    setInput("");
    setMessages((prev) => [
      ...prev,
      {
        role: "user",
        content: userMessage,
        timestamp,
        event_type: "UserInput",
        task_id: `task_user_${Date.now()}`,
      },
    ]);

    // Use MCP hook to handle the response
    handleMcpResponse(userMessage).catch((e) => {
      console.error("Failed to handle MCP response:", e);
    });
  };

  const scrollToMessage = (taskId: string) => {
    const element = document.getElementById(taskId);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  };

  return (
    <div className="app-container">
      <div className="main-layout">
        {/* Activity Bar (LM Studio style thin left bar) */}
        <ActivityBar
          isConnectionOpen={isConnectionOpen}
          setIsConnectionOpen={setIsConnectionOpen}
          isScheduledTasksOpen={isScheduledTasksOpen}
          setIsScheduledTasksOpen={setIsScheduledTasksOpen}
          isSettingsOpen={isSettingsOpen}
          setIsSettingsOpen={setIsSettingsOpen}
        />

        {/* Sidebar (History) */}
        <Sidebar
          isSidebarOpen={isSidebarOpen}
          history={history}
          activeSessionId={activeSessionId}
          messages={messages}
          createNewFolder={createNewFolder}
          createNewSession={createNewSession}
          toggleFolder={toggleFolder}
          onTimelineItemClick={scrollToMessage}
          switchSession={switchSession}
          renameSession={renameSession}
          deleteSession={deleteSession}
        />

        {/* Main Viewport (Grouping Chat and Settings) */}
        <div className="main-viewport">
          {isSettingsOpen ? (
            <SettingsPanel isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />
          ) : isConnectionOpen ? (
            <ConnectionSettingsPanel
              onClose={() => setIsConnectionOpen(false)}
              onConnectionsChanged={fetchHosts}
            />
          ) : isScheduledTasksOpen ? (
            <ScheduledTasksPanel onClose={() => setIsScheduledTasksOpen(false)} />
          ) : (
            <main className="main-chat">
              {/* Top Header */}
              <header className="chat-header">
                <div className="header-left">
                  <button
                    className="sidebar-toggle-button"
                    onClick={() => setIsSidebarOpen(!isSidebarOpen)}
                    title={isSidebarOpen ? t("app.sidebar_close") : t("app.sidebar_open")}
                  >
                    <SidebarIcon size={20} />
                  </button>
                  {isEditingHeader ? (
                    <input
                      className="header-title-input"
                      value={headerTitle}
                      onChange={(e) => setHeaderTitle(e.target.value)}
                      onBlur={handleSaveRenameHeader}
                      onCompositionStart={() => {
                        isComposingHeader.current = true;
                      }}
                      onCompositionEnd={() => {
                        setTimeout(() => {
                          isComposingHeader.current = false;
                        }, 150);
                      }}
                      onKeyDown={(e) => {
                        const isComp =
                          isComposingHeader.current ||
                          e.nativeEvent.isComposing ||
                          e.keyCode === 229;
                        if (isComp) {
                          return;
                        }
                        if (e.key === "Enter") {
                          handleSaveRenameHeader();
                        } else if (e.key === "Escape") {
                          setIsEditingHeader(false);
                        }
                      }}
                      autoFocus
                    />
                  ) : (
                    <h1
                      className="header-title clickable"
                      onDoubleClick={handleStartRenameHeader}
                      title={t("app.double_click_rename")}
                    >
                      {activeSession?.title || "mikomai"}
                    </h1>
                  )}
                  {recentIPs.length > 0 && (
                    <div style={{ display: "flex", alignItems: "center" }}>
                      <ServerIcon size={12} style={{ marginRight: "4px" }} />
                      <span className="header-hostname">
                        {(() => {
                          const current = recentIPs[0];
                          const host = availableHosts.find(
                            (h) => h.ip === current || h.hostname === current
                          );
                          if (host && host.hostname && host.ip && host.hostname !== host.ip) {
                            return `${host.hostname} (${host.ip})`;
                          }
                          return current;
                        })()}
                      </span>
                    </div>
                  )}
                </div>
              </header>

              {/* Chat History */}
              <Chat
                ref={messagesEndRef}
                messages={messages}
                formatMessageTime={formatMessageTime}
              />

              {/* Input Area */}
              <div className="input-area-wrapper">
                {messages.some((m) => m.status === "Running") && (
                  <div className="global-loading-indicator"></div>
                )}
                <ChatInput
                  ref={textareaRef}
                  modelStatus={modelStatus}
                  modelPath={modelPath}
                  input={input}
                  setInput={setInput}
                  showSuggestions={showSuggestions}
                  setShowSuggestions={setShowSuggestions}
                  filteredSuggestions={filteredSuggestions}
                  suggestionIndex={suggestionIndex}
                  setSuggestionIndex={setSuggestionIndex}
                  handleSelectSuggestion={handleSelectSuggestion}
                  handleSend={handleSend}
                  handleLoadModel={handleLoadModel}
                  setIsSettingsOpen={setIsSettingsOpen}
                  setCursorPos={setCursorPos}
                  availableHosts={availableHosts}
                  recentIPs={recentIPs}
                  setFilteredSuggestions={setFilteredSuggestions}
                />
              </div>
            </main>
          )}
        </div>
      </div>
      {/* Status Bar */}
      <StatusBar modelStatus={modelStatus} modelPath={modelPath} />
      {modalConfig && <CustomModal {...modalConfig} />}
    </div>
  );
}

export default App;
