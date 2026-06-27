import { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { SettingsPanel } from "../SettingsPanel";
import { ConnectionSettingsPanel } from "../ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "../ScheduledTasksPanel";
import "../../App.css";

import { Chat } from "../Chat/Chat";
import { ChatInput } from "../ChatInput/ChatInput";
import { Sidebar } from "../Sidebar/Sidebar";
import { ActivityBar } from "../ActivityBar/ActivityBar";
import { useMcp } from "../../hooks/useMcp";
import { StatusBar } from "../StatusBar/StatusBar";
import { useSettingsContext } from "../../contexts/SettingsContext";
import { useUIContext } from "../../contexts/UIContext";
import { useChatContext } from "../../contexts/ChatContext";
import { useModelContext } from "../../contexts/ModelContext";
import { useHostSuggestions } from "../../hooks/useHostSuggestions";
import { CustomModal } from "../CustomModal";
import { SidebarIcon, ServerIcon, DiffIcon } from "../Icons";
import { ConfigDiffPanel } from "../ConfigDiffPanel/ConfigDiffPanel";

export function AppLayout() {
  const { t } = useTranslation();
  const {
    historyLimit,
    modelPath,
    mcpTimeout,
    recentIPs,
    setRecentIPs,
    saveAllSettings,
  } = useSettingsContext();

  const { state: uiState, dispatch: uiDispatch } = useUIContext();
  const {
    state: chatState,
    dispatch: chatDispatch,
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
  } = useChatContext();
  const { state: modelState, handleLoadModel } = useModelContext();

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const isComposingHeader = useRef(false);
  const isExecutingRef = useRef(false);
  const queueRef = useRef<{
    content: string;
    timestamp: string;
    task_id: string;
    sessionId: string;
  }[]>([]);

  const handleStartRenameHeader = () => {
    if (activeSession) {
      uiDispatch({ type: "START_EDITING_HEADER", payload: activeSession.title });
    }
  };

  const handleSaveRenameHeader = () => {
    if (chatState.activeSessionId && uiState.headerTitle.trim()) {
      renameSession(chatState.activeSessionId, uiState.headerTitle.trim());
    }
    uiDispatch({ type: "STOP_EDITING_HEADER" });
  };

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
    activeSessionId: chatState.activeSessionId,
    updateSessionRecentIps,
    saveAllSettings,
    input: chatState.input,
    setInput,
    textareaRef,
  });

  const { handleMcpResponse } = useMcp({
    messages: chatState.messages,
    setMessages,
    summaries: chatState.summaries,
    setSummaries,
    historyLimit,
    mcpTimeout,
    updateRecentHosts,
    recentIPs,
  });

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [chatState.input]);

  // Scroll to bottom when messages change
  useEffect(() => {
    scrollToBottom();
  }, [chatState.messages]);

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

  const executeMessage = async (userMessage: string) => {
    isExecutingRef.current = true;
    try {
      await handleMcpResponse(userMessage);
    } catch (e) {
      console.error("Failed to handle MCP response:", e);
    } finally {
      if (queueRef.current.length > 0) {
        const next = queueRef.current.shift()!;
        chatDispatch({
          type: "SET_MESSAGE_STATUS",
          payload: { sessionId: next.sessionId, taskId: next.task_id, status: undefined },
        });
        executeMessage(next.content);
      } else {
        isExecutingRef.current = false;
      }
    }
  };

  const handleSend = async () => {
    if (!chatState.input.trim()) return;

    const userMessage = chatState.input.trim();
    const timestamp = new Date().toISOString();
    const taskId = `task_user_${Date.now()}`;
    const sessionId = chatState.activeSessionId;

    const ipRegex = /\b(?:\d{1,3}\.){3}\d{1,3}\b/g;
    const mentionRegex = /@([a-zA-Z0-9.-]+)/g;

    const foundIPs = userMessage.match(ipRegex) || [];
    const foundMentions = Array.from(userMessage.matchAll(mentionRegex)).map((m) => m[1]);

    const allFound = [...new Set([...foundMentions, ...foundIPs])];

    if (allFound.length > 0) {
      updateRecentHosts(allFound);
    }

    setInput("");

    const isPending = isExecutingRef.current;

    setMessages((prev) => [
      ...prev,
      {
        role: "user",
        content: userMessage,
        timestamp,
        event_type: "UserInput",
        task_id: taskId,
        status: isPending ? "Pending" : undefined,
      },
    ]);

    if (isPending) {
      queueRef.current.push({
        content: userMessage,
        timestamp,
        task_id: taskId,
        sessionId,
      });
    } else {
      executeMessage(userMessage);
    }
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
        <ActivityBar
          isConnectionOpen={uiState.isConnectionOpen}
          setIsConnectionOpen={(valueOrFn) => {
            const nextVal = typeof valueOrFn === "function" ? valueOrFn(uiState.isConnectionOpen) : valueOrFn;
            uiDispatch({ type: "SET_CONNECTION_OPEN", payload: nextVal });
          }}
          isScheduledTasksOpen={uiState.isScheduledTasksOpen}
          setIsScheduledTasksOpen={(valueOrFn) => {
            const nextVal = typeof valueOrFn === "function" ? valueOrFn(uiState.isScheduledTasksOpen) : valueOrFn;
            uiDispatch({ type: "SET_SCHEDULED_TASKS_OPEN", payload: nextVal });
          }}
          isSettingsOpen={uiState.isSettingsOpen}
          setIsSettingsOpen={(valueOrFn) => {
            const nextVal = typeof valueOrFn === "function" ? valueOrFn(uiState.isSettingsOpen) : valueOrFn;
            uiDispatch({ type: "SET_SETTINGS_OPEN", payload: nextVal });
          }}
        />

        <Sidebar
          isSidebarOpen={uiState.isSidebarOpen}
          history={chatState.history}
          activeSessionId={chatState.activeSessionId}
          messages={chatState.messages}
          createNewFolder={createNewFolder}
          createNewSession={createNewSession}
          toggleFolder={toggleFolder}
          onTimelineItemClick={scrollToMessage}
          switchSession={switchSession}
          renameSession={renameSession}
          deleteSession={deleteSession}
        />

        <div className="main-viewport">
          {uiState.isSettingsOpen ? (
            <SettingsPanel
              isOpen={uiState.isSettingsOpen}
              onClose={() => uiDispatch({ type: "SET_SETTINGS_OPEN", payload: false })}
            />
          ) : uiState.isConnectionOpen ? (
            <ConnectionSettingsPanel
              onClose={() => uiDispatch({ type: "SET_CONNECTION_OPEN", payload: false })}
              onConnectionsChanged={fetchHosts}
            />
          ) : uiState.isScheduledTasksOpen ? (
            <ScheduledTasksPanel
              onClose={() => uiDispatch({ type: "SET_SCHEDULED_TASKS_OPEN", payload: false })}
            />
          ) : (
            <div className="chat-workspace-container">
              <main className="main-chat">
                <header className="chat-header">
                  <div className="header-left">
                    <button
                      className="sidebar-toggle-button"
                      onClick={() => uiDispatch({ type: "SET_SIDEBAR_OPEN", payload: !uiState.isSidebarOpen })}
                      title={uiState.isSidebarOpen ? t("app.sidebar_close") : t("app.sidebar_open")}
                    >
                      <SidebarIcon size={20} />
                    </button>
                    {uiState.isEditingHeader ? (
                      <input
                        className="header-title-input"
                        value={uiState.headerTitle}
                        onChange={(e) => uiDispatch({ type: "SET_HEADER_TITLE", payload: e.target.value })}
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
                            uiDispatch({ type: "STOP_EDITING_HEADER" });
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
                  <div className="header-right">
                    <button
                      className={`sidebar-toggle-button ${uiState.isConfigDiffOpen ? "active" : ""}`}
                      onClick={() => uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: !uiState.isConfigDiffOpen })}
                      title={uiState.isConfigDiffOpen ? "Close Config Diff" : "Open Config Diff"}
                    >
                      <DiffIcon size={20} />
                    </button>
                  </div>
                </header>

                <Chat
                  ref={messagesEndRef}
                  messages={chatState.messages}
                  formatMessageTime={formatMessageTime}
                />

                <div className="input-area-wrapper">
                  {chatState.messages.some((m) => m.status === "Running") && (
                    <div className="global-loading-indicator"></div>
                  )}
                  <ChatInput
                    ref={textareaRef}
                    modelStatus={modelState.modelStatus}
                    modelPath={modelPath}
                    input={chatState.input}
                    setInput={setInput}
                    showSuggestions={showSuggestions}
                    setShowSuggestions={setShowSuggestions}
                    filteredSuggestions={filteredSuggestions}
                    suggestionIndex={suggestionIndex}
                    setSuggestionIndex={setSuggestionIndex}
                    handleSelectSuggestion={handleSelectSuggestion}
                    handleSend={handleSend}
                    handleLoadModel={handleLoadModel}
                    setIsSettingsOpen={(open) => uiDispatch({ type: "SET_SETTINGS_OPEN", payload: open })}
                    setCursorPos={setCursorPos}
                    availableHosts={availableHosts}
                    recentIPs={recentIPs}
                    setFilteredSuggestions={setFilteredSuggestions}
                  />
                </div>
              </main>
              <ConfigDiffPanel
                isOpen={uiState.isConfigDiffOpen}
                onClose={() => uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: false })}
              />
            </div>
          )}
        </div>
      </div>
      <StatusBar modelStatus={modelState.modelStatus} modelPath={modelPath} />
      {chatState.modalConfig && <CustomModal {...chatState.modalConfig} />}
    </div>
  );
}
