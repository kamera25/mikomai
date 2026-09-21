import { lazy, Suspense, useRef, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ipc, COMMANDS } from "../../platform";
import "../../App.css";

import { ChatPanel } from "../../features/chat/ChatPanel";
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
import { ChatHeader } from "./ChatHeader";
import { Attachment } from "../../types";
import { formatMessageTime } from "../../utils/messageTime";
import { useMessageExecution } from "./useMessageExecution";
import { useAppLayoutActions } from "./useAppLayoutActions";
import { GuiEventScope, type GuiEvent } from "../../gui/events";
import { WatchNotificationToast } from "../WatchNotificationToast";
import { KeyringAccessModal } from "../KeyringAccessModal";

// These panels are not part of the chat's critical rendering path. Loading
// them only when opened reduces startup parsing and keeps their effects idle.
const SettingsPanel = lazy(() =>
  import("../../features/settings/SettingsPanel").then(({ SettingsPanel }) => ({
    default: SettingsPanel,
  }))
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
  import("../../features/operations/ConfigDiffPanel").then(({ ConfigDiffPanel }) => ({
    default: ConfigDiffPanel,
  }))
);
const TaskAuditPanel = lazy(() =>
  import("../TaskAuditPanel").then(({ TaskAuditPanel }) => ({ default: TaskAuditPanel }))
);

function useRootMediator() {
  const { t } = useTranslation();
  const { historyLimit, modelPath, mcpTimeout, recentIPs, setRecentIPs, saveAllSettings } =
    useSettingsContext();

  const { state: uiState, dispatch: uiDispatch } = useUIContext();

  const { diffCommitId, setDiffCommitId } = useConfigDiffEvents();

  const handleCloseConfigDiff = useCallback(() => {
    uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: false });
    if (diffCommitId) {
      ipc.submitChoice(diffCommitId, "cancel").catch((err) => {
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
  const hostLabel = useMemo(() => {
    const current = recentIPs[0];
    if (!current) return undefined;
    const host = availableHosts.find(
      (candidate) => candidate.ip === current || candidate.hostname === current
    );
    return host?.hostname && host.ip && host.hostname !== host.ip
      ? `${host.hostname} (${host.ip})`
      : current;
  }, [recentIPs, availableHosts]);

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

  const {
    isGenerating,
    setIsGenerating,
    sendMessage,
    stop: handleStop,
  } = useMessageExecution({
    input: chatState.input,
    setInput,
    activeSessionId: chatState.activeSessionId,
    createNewSession,
    setMessages,
    updateRecentHosts,
    handleMcpResponse,
    stoppedLabel: t("chat.stopped"),
  });
  const isCurrentlyGenerating =
    isGenerating ||
    chatState.messages.some((message) => message.status === "Running" || message.isToolLoading);

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

  const layoutActions = useAppLayoutActions({
    uiDispatch,
    activeSession,
    activeSessionId: chatState.activeSessionId,
    headerTitle: uiState.headerTitle,
    renameSession,
    isCurrentlyGenerating,
    setMessages,
    setIsGenerating,
    resumeAgent: (taskId) => ipc.command(COMMANDS.resumeAgentTask, { taskId }),
  });
  const { handleStartRenameHeader, handleSaveRenameHeader, resumeTask } = layoutActions;

  const mediate = useCallback(
    (event: GuiEvent): boolean => {
      switch (event.type) {
        case "navigate":
          uiDispatch({ type: "NAVIGATE", panel: event.panel });
          return true;
        case "sidebar.toggle":
          uiDispatch({ type: "SET_SIDEBAR_OPEN", payload: !uiState.isSidebarOpen });
          return true;
        case "diff.toggle":
          uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: !uiState.isConfigDiffOpen });
          return true;
        case "header.edit":
          handleStartRenameHeader();
          return true;
        case "header.change":
          uiDispatch({ type: "SET_HEADER_TITLE", payload: event.title });
          return true;
        case "header.save":
          handleSaveRenameHeader();
          return true;
        case "header.cancel":
          uiDispatch({ type: "STOP_EDITING_HEADER" });
          return true;
        case "session.create":
          createNewSession();
          return true;
        case "session.select":
          switchSession(event.id);
          return true;
        case "session.folder.toggle":
          toggleFolder(event.id);
          return true;
        case "session.rename":
          renameSession(event.id, event.title);
          return true;
        case "session.delete":
          deleteSession(event.id);
          return true;
        case "timeline.scroll":
          scrollToMessage(event.taskId);
          return true;
        case "question.answer":
          if (event.kind === "choice") handleSelectChoice(event.id, event.value);
          else if (event.kind === "interface") handleSelectInterface(event.id, event.value);
          else handleSelectIpAddress(event.id, event.value);
          return true;
        case "question.cancel":
          if (event.kind === "choice") handleCancelChoice(event.id);
          else if (event.kind === "interface") handleCancelInterface(event.id);
          else handleCancelIpAddress(event.id);
          return true;
      }
    },
    [
      uiDispatch,
      uiState.isSidebarOpen,
      uiState.isConfigDiffOpen,
      handleStartRenameHeader,
      handleSaveRenameHeader,
      createNewSession,
      switchSession,
      toggleFolder,
      renameSession,
      deleteSession,
      scrollToMessage,
      handleSelectChoice,
      handleSelectInterface,
      handleSelectIpAddress,
      handleCancelChoice,
      handleCancelInterface,
      handleCancelIpAddress,
    ]
  );

  return {
    mediate,
    uiState,
    chatState,
    sidebarStyle,
    isResizingLeft,
    handleLeftMouseDown,
    fetchHosts,
    resumeTask,
    activeSession,
    hostLabel,
    messagesEndRef,
    handleSend,
    isResizingRight,
    questionQueue,
    totalQuestionsCount,
    textareaRef,
    modelState,
    modelPath,
    setInput,
    showSuggestions,
    setShowSuggestions,
    filteredSuggestions,
    suggestionIndex,
    setSuggestionIndex,
    handleSelectSuggestion,
    handleStop,
    isCurrentlyGenerating,
    handleLoadModel,
    cursorPos,
    setCursorPos,
    availableHosts,
    recentIPs,
    setFilteredSuggestions,
    diffCommitId,
    diffPanelStyle,
    handleRightMouseDown,
    handleCloseConfigDiff,
  };
}

function AppLayoutView({
    mediate,
    uiState,
    chatState,
    sidebarStyle,
    isResizingLeft,
    handleLeftMouseDown,
    fetchHosts,
    resumeTask,
    activeSession,
    hostLabel,
    messagesEndRef,
    handleSend,
    isResizingRight,
    questionQueue,
    totalQuestionsCount,
    textareaRef,
    modelState,
    modelPath,
    setInput,
    showSuggestions,
    setShowSuggestions,
    filteredSuggestions,
    suggestionIndex,
    setSuggestionIndex,
    handleSelectSuggestion,
    handleStop,
    isCurrentlyGenerating,
    handleLoadModel,
    cursorPos,
    setCursorPos,
    availableHosts,
    recentIPs,
    setFilteredSuggestions,
    diffCommitId,
    diffPanelStyle,
    handleRightMouseDown,
    handleCloseConfigDiff,
}: ReturnType<typeof useRootMediator>) {
  return (
    <GuiEventScope handle={mediate}>
      <div className="app-container">
        <div className="main-layout">
          <ActivityBar activePanel={uiState.activePanel} />

          <Sidebar
            isSidebarOpen={uiState.isSidebarOpen}
            history={chatState.history}
            activeSessionId={chatState.activeSessionId}
            messages={chatState.messages}
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
                  onClose={() => mediate({ type: "navigate", panel: "chat" })}
                />
              ) : uiState.isConnectionOpen ? (
                <ConnectionSettingsPanel
                  onClose={() => mediate({ type: "navigate", panel: "chat" })}
                  onConnectionsChanged={fetchHosts}
                />
              ) : uiState.isScheduledTasksOpen ? (
                <ScheduledTasksPanel onClose={() => mediate({ type: "navigate", panel: "chat" })} />
              ) : uiState.isTaskAuditOpen ? (
                <TaskAuditPanel
                  onClose={() => mediate({ type: "navigate", panel: "chat" })}
                  onResume={resumeTask}
                />
              ) : (
                <div className="chat-workspace-container">
                  <main className="main-chat">
                    <ChatHeader
                      isSidebarOpen={uiState.isSidebarOpen}
                      isConfigDiffOpen={uiState.isConfigDiffOpen}
                      isEditing={uiState.isEditingHeader}
                      draftTitle={uiState.headerTitle}
                      sessionTitle={activeSession?.title || "mikomai"}
                      hostLabel={hostLabel}
                    />

                    <ChatPanel
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
                        onOpenSettings={() => mediate({ type: "navigate", panel: "settings" })}
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
        <StatusBar
          modelStatus={modelState.modelStatus}
          modelPath={modelPath}
          loadedModelPath={modelState.loadedModelPath}
        />
        {chatState.modalConfig && <CustomModal {...chatState.modalConfig} />}
        <WatchNotificationToast />
        <KeyringAccessModal />
      </div>
    </GuiEventScope>
  );
}

export function RootMediator() {
  const viewModel = useRootMediator();
  return <AppLayoutView {...viewModel} />;
}
