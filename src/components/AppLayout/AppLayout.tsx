import { lazy, Suspense, useRef, useEffect, useState, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
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
import { useResizablePane } from "../../hooks/useResizablePane";
import { useQuestionQueue } from "../../hooks/useQuestionQueue";
import { useConfigDiffEvents } from "../../hooks/useConfigDiffEvents";
import { QuestionPanel } from "./QuestionPanel";
import { CustomModal } from "../CustomModal";
import { SidebarIcon, ServerIcon, DiffIcon } from "../Icons";
import { Attachment, Message } from "../../types";
import { formatMessageTime } from "../../utils/messageTime";

// These panels are not part of the chat's critical rendering path. Loading
// them only when opened reduces startup parsing and keeps their effects idle.
const SettingsPanel = lazy(() =>
  import("../SettingsPanel").then(({ SettingsPanel }) => ({ default: SettingsPanel }))
);
const ConnectionSettingsPanel = lazy(() =>
  import("../ConnectionSettingsPanel").then(({ ConnectionSettingsPanel }) => ({
    default: ConnectionSettingsPanel,
  }))
);
const ScheduledTasksPanel = lazy(() =>
  import("../ScheduledTasksPanel").then(({ ScheduledTasksPanel }) => ({
    default: ScheduledTasksPanel,
  }))
);
const ConfigDiffPanel = lazy(() =>
  import("../ConfigDiffPanel/ConfigDiffPanel").then(({ ConfigDiffPanel }) => ({
    default: ConfigDiffPanel,
  }))
);

export function AppLayout() {
  const { t } = useTranslation();
  const { historyLimit, modelPath, mcpTimeout, recentIPs, setRecentIPs, saveAllSettings } =
    useSettingsContext();

  const { state: uiState, dispatch: uiDispatch } = useUIContext();

  const { diffCommitId, setDiffCommitId } = useConfigDiffEvents();

  const handleCloseConfigDiff = useCallback(() => {
    uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: false });
    if (diffCommitId) {
      invoke("submit_user_choice", { id: diffCommitId, choice: "cancel" }).catch((err) => {
        console.error("Failed to cancel user choice on close:", err);
      });
      setDiffCommitId(null);
    }
  }, [diffCommitId, setDiffCommitId, uiDispatch]);

  // Custom hooks for extracted concerns
  const {
    sidebarWidth,
    diffWidth,
    isResizingLeft,
    isResizingRight,
    handleLeftMouseDown,
    handleRightMouseDown,
  } = useResizablePane({
    onSidebarCollapse: (collapsed) => {
      uiDispatch({ type: "SET_SIDEBAR_OPEN", payload: !collapsed });
    },
    onDiffCollapse: (collapsed) => {
      if (collapsed) {
        handleCloseConfigDiff();
      } else {
        uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: true });
      }
    },
  });

  const {
    questionQueue,
    totalQuestionsCount,
    handleSelectChoice,
    handleCancelChoice,
    handleSelectInterface,
    handleCancelInterface,
    handleSelectIpAddress,
    handleCancelIpAddress,
  } = useQuestionQueue();

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
  const modelStatusRef = useRef(modelState.modelStatus);
  useEffect(() => {
    modelStatusRef.current = modelState.modelStatus;
  }, [modelState.modelStatus]);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const isComposingHeader = useRef(false);
  const isExecutingRef = useRef(false);
  const queueRef = useRef<
    {
      content: string;
      timestamp: string;
      task_id: string;
      sessionId: string;
      attachments?: Attachment[];
    }[]
  >([]);

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
    cursorPos,
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

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [chatState.input]);

  const [isGenerating, setIsGenerating] = useState(false);
  const isCurrentlyGenerating =
    isGenerating || chatState.messages.some((m) => m.status === "Running" || m.isToolLoading);

  const handleStop = async () => {
    try {
      await invoke("stop_llm");
    } catch (err) {
      console.error("Failed to stop LLM:", err);
    }
    queueRef.current = [];
    isExecutingRef.current = false;
    setIsGenerating(false);

    setMessages((prev) =>
      prev.map((msg) => {
        if (msg.status === "Running" || msg.isToolLoading) {
          return {
            ...msg,
            isToolLoading: false,
            status: "Failed",
            summary_text: msg.summary_text
              ? `${msg.summary_text} (${t("chat.stopped")})`
              : t("chat.stopped"),
          } as Message;
        }
        return msg;
      })
    );
  };

  const executeMessage = async (userMessage: string, attachments?: Attachment[]) => {
    isExecutingRef.current = true;
    setIsGenerating(true);
    try {
      await handleMcpResponse(userMessage, attachments);
    } catch (e) {
      console.error("Failed to handle MCP response:", e);
    } finally {
      if (queueRef.current.length > 0 && modelStatusRef.current === "Loaded") {
        const next = queueRef.current.shift()!;
        chatDispatch({
          type: "SET_MESSAGE_STATUS",
          payload: { sessionId: next.sessionId, taskId: next.task_id, status: undefined },
        });
        void executeMessage(next.content, next.attachments);
      } else {
        isExecutingRef.current = false;
        setIsGenerating(false);
      }
    }
  };

  const executeMessageRef = useRef(executeMessage);
  useEffect(() => {
    executeMessageRef.current = executeMessage;
  }, [executeMessage]);

  // Messages submitted while the model is loading (or unavailable) stay in
  // the same queue as messages submitted during generation. Start them in
  // order as soon as the model reports that it is ready.
  useEffect(() => {
    if (modelState.modelStatus !== "Loaded" || isExecutingRef.current || queueRef.current.length === 0) {
      return;
    }
    const next = queueRef.current.shift()!;
    chatDispatch({
      type: "SET_MESSAGE_STATUS",
      payload: { sessionId: next.sessionId, taskId: next.task_id, status: undefined },
    });
    void executeMessageRef.current(next.content, next.attachments);
  }, [modelState.modelStatus, chatDispatch]);

  const sendMessage = async (text?: string, attachments?: Attachment[]) => {
    const messageText = text !== undefined ? text : chatState.input.trim();
    if (!messageText && (!attachments || attachments.length === 0)) return;

    const timestamp = new Date().toISOString();
    const taskId = crypto.randomUUID();
    let sessionId = chatState.activeSessionId;
    if (!sessionId) {
      const newSession = await createNewSession();
      if (!newSession) return;
      sessionId = newSession.id;
    }

    const ipRegex = /\b(?:\d{1,3}\.){3}\d{1,3}\b/g;
    const mentionRegex = /@([a-zA-Z0-9.-]+)/g;

    const foundIPs = messageText.match(ipRegex) || [];
    const foundMentions = Array.from(messageText.matchAll(mentionRegex)).map((m) => m[1]);

    const allFound = [...new Set([...foundMentions, ...foundIPs])];

    if (allFound.length > 0) {
      updateRecentHosts(allFound);
    }

    if (text === undefined) {
      setInput("");
    }

    const isPending = isExecutingRef.current || modelStatusRef.current !== "Loaded";

    setMessages((prev) => [
      ...prev,
      {
        role: "user",
        content: messageText,
        timestamp,
        event_type: "UserInput",
        task_id: taskId,
        status: isPending ? "Pending" : undefined,
        attachments,
      },
    ]);

    if (isPending) {
      queueRef.current.push({
        content: messageText,
        timestamp,
        task_id: taskId,
        sessionId,
        attachments,
      });
    } else {
      executeMessage(messageText, attachments);
    }
  };

  // Keep callbacks passed to the chat stable. In particular, typing in the
  // input must not re-render the full message timeline.
  const sendMessageRef = useRef(sendMessage);
  useEffect(() => {
    sendMessageRef.current = sendMessage;
  }, [sendMessage]);
  const handleSend = useCallback(
    (text?: string, attachments?: Attachment[]) => sendMessageRef.current(text, attachments),
    []
  );

  const scrollToMessage = useCallback((taskId: string) => {
    const element = document.getElementById(taskId);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, []);

  const sidebarStyle = useMemo(() => ({ width: sidebarWidth }), [sidebarWidth]);
  const diffPanelStyle = useMemo(
    () => ({ width: diffWidth, maxWidth: "none", minWidth: "none" }),
    [diffWidth]
  );

  const handleSetConnectionOpen = useCallback(
    (valueOrFn: React.SetStateAction<boolean>) => {
      uiDispatch({
        type: "SET_CONNECTION_OPEN",
        payload: typeof valueOrFn === "function" ? valueOrFn(uiState.isConnectionOpen) : valueOrFn,
      });
    },
    [uiState.isConnectionOpen, uiDispatch]
  );

  const handleSetScheduledTasksOpen = useCallback(
    (valueOrFn: React.SetStateAction<boolean>) => {
      uiDispatch({
        type: "SET_SCHEDULED_TASKS_OPEN",
        payload:
          typeof valueOrFn === "function" ? valueOrFn(uiState.isScheduledTasksOpen) : valueOrFn,
      });
    },
    [uiState.isScheduledTasksOpen, uiDispatch]
  );

  const handleSetSettingsOpen = useCallback(
    (valueOrFn: React.SetStateAction<boolean>) => {
      uiDispatch({
        type: "SET_SETTINGS_OPEN",
        payload: typeof valueOrFn === "function" ? valueOrFn(uiState.isSettingsOpen) : valueOrFn,
      });
    },
    [uiState.isSettingsOpen, uiDispatch]
  );

  return (
    <div className="app-container">
      <div className="main-layout">
        <ActivityBar
          isConnectionOpen={uiState.isConnectionOpen}
          setIsConnectionOpen={handleSetConnectionOpen}
          isScheduledTasksOpen={uiState.isScheduledTasksOpen}
          setIsScheduledTasksOpen={handleSetScheduledTasksOpen}
          isSettingsOpen={uiState.isSettingsOpen}
          setIsSettingsOpen={handleSetSettingsOpen}
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
          style={sidebarStyle}
          isResizing={isResizingLeft}
        />
        {uiState.isSidebarOpen && (
          <div
            className={`resize-handle ${isResizingLeft ? "active" : ""}`}
            onMouseDown={handleLeftMouseDown}
          />
        )}

        <div className="main-viewport">
          <Suspense fallback={null}>
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
                        onClick={() =>
                          uiDispatch({ type: "SET_SIDEBAR_OPEN", payload: !uiState.isSidebarOpen })
                        }
                        title={
                          uiState.isSidebarOpen ? t("app.sidebar_close") : t("app.sidebar_open")
                        }
                      >
                        <SidebarIcon size={20} />
                      </button>
                      {uiState.isEditingHeader ? (
                        <input
                          className="header-title-input"
                          value={uiState.headerTitle}
                          onChange={(e) =>
                            uiDispatch({ type: "SET_HEADER_TITLE", payload: e.target.value })
                          }
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
                        onClick={() =>
                          uiDispatch({
                            type: "SET_CONFIG_DIFF_OPEN",
                            payload: !uiState.isConfigDiffOpen,
                          })
                        }
                        title={
                          uiState.isConfigDiffOpen ? t("app.diff_close") : t("app.diff_open")
                        }
                      >
                        <DiffIcon size={20} />
                      </button>
                    </div>
                  </header>

                  <Chat
                    ref={messagesEndRef}
                    messages={chatState.messages}
                    formatMessageTime={formatMessageTime}
                    sendMessage={handleSend}
                    isResizing={isResizingLeft || isResizingRight}
                  />

                  <div
                    className="input-area-wrapper"
                    style={{ display: "flex", flexDirection: "column", gap: "8px" }}
                  >
                    {chatState.messages.some((m) => m.status === "Running") && (
                      <div className="global-loading-indicator"></div>
                    )}
                    <QuestionPanel
                      questionQueue={questionQueue}
                      totalQuestionsCount={totalQuestionsCount}
                      handleSelectChoice={handleSelectChoice}
                      handleCancelChoice={handleCancelChoice}
                      handleSelectInterface={handleSelectInterface}
                      handleCancelInterface={handleCancelInterface}
                      handleSelectIpAddress={handleSelectIpAddress}
                      handleCancelIpAddress={handleCancelIpAddress}
                    />
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
                      handleStop={handleStop}
                      isGenerating={isCurrentlyGenerating}
                      handleLoadModel={handleLoadModel}
                      setIsSettingsOpen={handleSetSettingsOpen}
                      cursorPos={cursorPos}
                      setCursorPos={setCursorPos}
                      availableHosts={availableHosts}
                      recentIPs={recentIPs}
                      setFilteredSuggestions={setFilteredSuggestions}
                    />
                  </div>
                </main>
                {uiState.isConfigDiffOpen && (
                  <div
                    className={`resize-handle ${isResizingRight ? "active" : ""}`}
                    onMouseDown={handleRightMouseDown}
                  />
                )}
                {uiState.isConfigDiffOpen && (
                  <ConfigDiffPanel
                    id={diffCommitId}
                    isOpen={uiState.isConfigDiffOpen}
                    style={diffPanelStyle}
                    isResizing={isResizingRight}
                    onClose={handleCloseConfigDiff}
                  />
                )}
              </div>
            )}
          </Suspense>
        </div>
      </div>
      <StatusBar modelStatus={modelState.modelStatus} modelPath={modelPath} />
      {chatState.modalConfig && <CustomModal {...chatState.modalConfig} />}
    </div>
  );
}
