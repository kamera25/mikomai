import { useRef, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
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
import { Attachment } from "../../types";

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
  interface ChoiceConfig {
    id: string;
    title: string;
    message: string;
    options: string[];
  }

  interface InterfaceChoiceConfig {
    id: string;
    vendor: string;
    message?: string;
  }

  interface IpAddressChoiceConfig {
    id: string;
    title: string;
    message: string;
    subnet: string;
    defaultIp?: string;
  }

  type QuestionItem = 
    | { type: "choice"; data: ChoiceConfig }
    | { type: "interface"; data: InterfaceChoiceConfig }
    | { type: "ipaddress"; data: IpAddressChoiceConfig };

  const [questionQueue, setQuestionQueue] = useState<QuestionItem[]>([]);
  const [totalQuestionsCount, setTotalQuestionsCount] = useState(0);
  const [diffCommitId, setDiffCommitId] = useState<string | null>(null);

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
    attachments?: Attachment[];
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

  // Update ConfigDiffPanel with dynamic conversion diffs
  useEffect(() => {
    const unlisten = listen<any>("chat-event", (event) => {
      const chatEvent = event.payload;
      if (chatEvent.type === "mcpToolFinished") {
        const { toolId, success, output, args } = chatEvent.payload;
        if (success && toolId === "convert_cisco_config") {
          // Extract the converted config from markdown
          const regex = /```[a-z]*\n([\s\S]*?)```/;
          const match = output.match(regex);
          if (match && match[1]) {
            const converted = match[1].trim();
            const vendor = args?.target_vendor || args?.targetVendor || "juniper";
            const lines = converted.split("\n");
            const diffLines = lines.map((line: string, idx: number) => ({
              type: "insert" as const,
              oldLine: null,
              newLine: idx + 1,
              content: line,
            }));
            
            uiDispatch({
              type: "SET_CONFIG_DIFF_DATA",
              payload: {
                fileName: `${vendor}.conf`,
                additions: lines.length,
                deletions: 0,
                diffLines,
              },
            });
            uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: true });
          }
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [uiDispatch]);

  // Listen to request-diff-commit from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-diff-commit", (event) => {
      const { id, config, fileName, hostname, ip } = event.payload;
      if (id) {
        setDiffCommitId(id);
      }
      if (config) {
        const lines = config.split("\n");
        const diffLines = lines.map((line: string, idx: number) => ({
          type: "insert" as const,
          oldLine: null,
          newLine: idx + 1,
          content: line,
        }));
        
        uiDispatch({
          type: "SET_CONFIG_DIFF_DATA",
          payload: {
            fileName: fileName || "cisco.conf",
            additions: lines.length,
            deletions: 0,
            diffLines,
            hostname,
            ip,
          },
        });
        uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: true });
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [uiDispatch]);

  // Listen to user choice requests from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-user-choice", (event) => {
      const { id, title, message, options } = event.payload;
      const item: QuestionItem = {
        type: "choice",
        data: { id: id || "default", title, message, options }
      };
      setQuestionQueue(prev => {
        const filtered = prev.filter(q => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount(prevTotal => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to interface choice requests from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-interface-choice", (event) => {
      const { id, vendor, message } = event.payload;
      const item: QuestionItem = {
        type: "interface",
        data: { id: id || "default", vendor, message }
      };
      setQuestionQueue(prev => {
        const filtered = prev.filter(q => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount(prevTotal => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to IP address choice requests from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-ipaddress-choice", (event) => {
      const { id, title, message, subnet, defaultIp } = event.payload;
      const item: QuestionItem = {
        type: "ipaddress",
        data: { id: id || "default", title, message, subnet, defaultIp }
      };
      setQuestionQueue(prev => {
        const filtered = prev.filter(q => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount(prevTotal => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to keyboard Escape when choice panels are active
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && questionQueue.length > 0) {
        const current = questionQueue[0];
        if (current.type === "interface") {
          handleCancelInterface(current.data.id);
        } else if (current.type === "ipaddress") {
          handleCancelIpAddress(current.data.id);
        } else {
          handleCancelChoice(current.data.id);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [questionQueue]);

  const handleSelectChoice = async (id: string, option: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_user_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit user choice:", err);
    }
  };

  const handleCancelChoice = async (id: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_user_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel user choice:", err);
    }
  };

  const handleSelectInterface = async (id: string, option: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_interface_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit interface choice:", err);
    }
  };

  const handleCancelInterface = async (id: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_interface_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel interface choice:", err);
    }
  };

  const handleSelectIpAddress = async (id: string, option: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_ipaddress_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit IP address choice:", err);
    }
  };

  const handleCancelIpAddress = async (id: string) => {
    setQuestionQueue(prev => {
      const next = prev.filter(q => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_ipaddress_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel IP address choice:", err);
    }
  };

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

  const [isGenerating, setIsGenerating] = useState(false);
  const isCurrentlyGenerating = isGenerating || chatState.messages.some((m) => m.status === "Running" || m.isToolLoading);

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
          };
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
      if (queueRef.current.length > 0) {
        const next = queueRef.current.shift()!;
        chatDispatch({
          type: "SET_MESSAGE_STATUS",
          payload: { sessionId: next.sessionId, taskId: next.task_id, status: undefined },
        });
        executeMessage(next.content, next.attachments);
      } else {
        isExecutingRef.current = false;
        setIsGenerating(false);
      }
    }
  };

  const sendMessage = async (text?: string, attachments?: Attachment[]) => {
    const messageText = text !== undefined ? text : chatState.input.trim();
    if (!messageText && (!attachments || attachments.length === 0)) return;

    const timestamp = new Date().toISOString();
    const taskId = `task_user_${Date.now()}`;
    const sessionId = chatState.activeSessionId;

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

    const isPending = isExecutingRef.current;

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

  const handleSend = (text?: string, attachments?: Attachment[]) => sendMessage(text, attachments);

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
                  sendMessage={sendMessage}
                />

                <div className="input-area-wrapper" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                  {chatState.messages.some((m) => m.status === "Running") && (
                    <div className="global-loading-indicator"></div>
                  )}
                  {questionQueue.length > 0 && (() => {
                    const currentQuestion = questionQueue[0];
                    const currentIndex = totalQuestionsCount - questionQueue.length + 1;
                    const progressPrefix = `【質問 ${currentIndex}/${totalQuestionsCount}】`;
                    
                    if (currentQuestion.type === "choice") {
                      const choice = currentQuestion.data;
                      return (
                        <div key={choice.id} className="input-choice-panel" style={{
                          background: "var(--bg-secondary)",
                          border: "1px solid var(--border)",
                          borderRadius: "8px",
                          padding: "12px",
                          boxShadow: "0 -2px 10px rgba(0,0,0,0.15)",
                          display: "flex",
                          flexDirection: "column",
                          gap: "10px",
                          animation: "fadeIn 0.2s ease",
                        }}>
                          <div style={{ fontWeight: "600", fontSize: "13px", color: "var(--text-secondary)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                            <span>{progressPrefix} {choice.message}</span>
                            <button
                              onClick={() => handleCancelChoice(choice.id)}
                              style={{
                                background: "transparent",
                                border: "none",
                                color: "var(--text-secondary)",
                                cursor: "pointer",
                                fontSize: "11px",
                                padding: "2px 6px",
                                borderRadius: "4px",
                              }}
                              onMouseEnter={(e) => e.currentTarget.style.background = "var(--bg-tertiary)"}
                              onMouseLeave={(e) => e.currentTarget.style.background = "transparent"}
                            >
                              スキップ (Esc)
                            </button>
                          </div>
                          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                            {choice.options.map((opt, idx) => (
                              <button
                                key={idx}
                                onClick={() => handleSelectChoice(choice.id, opt)}
                                style={{
                                  display: "flex",
                                  alignItems: "center",
                                  width: "100%",
                                  padding: "10px 14px",
                                  background: "var(--bg-tertiary)",
                                  border: "1px solid var(--border)",
                                  borderRadius: "6px",
                                  color: "var(--text-primary)",
                                  textAlign: "left",
                                  cursor: "pointer",
                                  fontSize: "13px",
                                  fontWeight: "500",
                                  transition: "all 0.15s ease",
                                }}
                                onMouseEnter={(e) => {
                                  e.currentTarget.style.borderColor = "var(--primary)";
                                  e.currentTarget.style.background = "var(--bg-hover)";
                                }}
                                onMouseLeave={(e) => {
                                  e.currentTarget.style.borderColor = "var(--border)";
                                  e.currentTarget.style.background = "var(--bg-tertiary)";
                                }}
                              >
                                <span style={{
                                  display: "inline-flex",
                                  alignItems: "center",
                                  justifyContent: "center",
                                  width: "22px",
                                  height: "22px",
                                  borderRadius: "50%",
                                  background: "var(--bg-secondary)",
                                  marginRight: "12px",
                                  fontSize: "11px",
                                  fontWeight: "bold",
                                  color: "var(--text-secondary)",
                                }}>{idx + 1}</span>
                                {opt}
                              </button>
                            ))}
                          </div>
                        </div>
                      );
                    } else if (currentQuestion.type === "ipaddress") {
                      const choice = currentQuestion.data;
                      return (
                        <IpAddressChoicePanel
                          key={choice.id}
                          choice={choice}
                          progressPrefix={progressPrefix}
                          onSelect={handleSelectIpAddress}
                          onCancel={handleCancelIpAddress}
                        />
                      );
                    } else {
                      const choice = currentQuestion.data;
                      return (
                        <InterfaceChoicePanel
                          key={choice.id}
                          choice={choice}
                          progressPrefix={progressPrefix}
                          onSelect={handleSelectInterface}
                          onCancel={handleCancelInterface}
                        />
                      );
                    }
                  })()}
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
                    setIsSettingsOpen={(open) => uiDispatch({ type: "SET_SETTINGS_OPEN", payload: open })}
                    cursorPos={cursorPos}
                    setCursorPos={setCursorPos}
                    availableHosts={availableHosts}
                    recentIPs={recentIPs}
                    setFilteredSuggestions={setFilteredSuggestions}
                  />
                </div>
              </main>
              <ConfigDiffPanel
                id={diffCommitId}
                isOpen={uiState.isConfigDiffOpen}
                onClose={() => {
                  uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: false });
                  if (diffCommitId) {
                    invoke("submit_user_choice", { id: diffCommitId, choice: "cancel" }).catch((err) => {
                      console.error("Failed to cancel user choice on close:", err);
                    });
                    setDiffCommitId(null);
                  }
                }}
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

interface InterfaceChoicePanelProps {
  choice: {
    id: string;
    vendor: string;
    message?: string;
  };
  progressPrefix: string;
  onSelect: (id: string, option: string) => void;
  onCancel: (id: string) => void;
}

function InterfaceChoicePanel({ choice, progressPrefix, onSelect, onCancel }: InterfaceChoicePanelProps) {
  const [ciscoType, setCiscoType] = useState("GigabitEthernet");
  const [ciscoNum, setCiscoNum] = useState("0/1");
  const [customInterface, setCustomInterface] = useState("");

  const vendor = choice.vendor || "Cisco_IOS";
  const isCisco = vendor.toLowerCase().includes("cisco") || vendor.toLowerCase().includes("ios");
  const isYamaha = vendor.toLowerCase().includes("yamaha");
  const isArista = vendor.toLowerCase().includes("arista");

  return (
    <div className="input-choice-panel" style={{
      background: "var(--bg-secondary)",
      border: "1px solid var(--border)",
      borderRadius: "8px",
      padding: "16px",
      boxShadow: "0 -2px 10px rgba(0,0,0,0.15)",
      display: "flex",
      flexDirection: "column",
      gap: "12px",
      animation: "fadeIn 0.2s ease",
    }}>
      <div style={{ fontWeight: "600", fontSize: "14px", color: "var(--text-primary)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span>{progressPrefix} インターフェースの選択 - {vendor}</span>
        <button
          onClick={() => onCancel(choice.id)}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-secondary)",
            cursor: "pointer",
            fontSize: "11px",
            padding: "2px 6px",
            borderRadius: "4px",
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = "var(--bg-tertiary)"}
          onMouseLeave={(e) => e.currentTarget.style.background = "transparent"}
        >
          キャンセル (Esc)
        </button>
      </div>

      {choice.message && (
        <div style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "4px", whiteSpace: "pre-wrap" }}>
          {choice.message}
        </div>
      )}

      {/* Cisco_IOS の UI */}
      {isCisco && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div style={{ display: "flex", gap: "10px" }}>
            <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "4px" }}>
              <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>種別</label>
              <select
                value={ciscoType}
                onChange={(e) => setCiscoType(e.target.value)}
                style={{
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              >
                <option value="GigabitEthernet">GigabitEthernet</option>
                <option value="FastEthernet">FastEthernet</option>
                <option value="TenGigabitEthernet">TenGigabitEthernet</option>
                <option value="Ethernet">Ethernet</option>
                <option value="Vlan">Vlan</option>
                <option value="Loopback">Loopback</option>
              </select>
            </div>
            <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "4px" }}>
              <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>番号</label>
              <input
                type="text"
                value={ciscoNum}
                onChange={(e) => setCiscoNum(e.target.value)}
                placeholder="例: 0/1, 1/0/1"
                style={{
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
            </div>
          </div>
          <button
            className="btn btn-primary"
            onClick={() => onSelect(choice.id, `${ciscoType}${ciscoNum}`)}
            style={{
              width: "100%",
              padding: "10px",
              fontWeight: "500",
            }}
          >
            選択を確定 : {ciscoType}{ciscoNum}
          </button>
        </div>
      )}

      {/* Yamaha の UI */}
      {isYamaha && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            {["lan1", "lan2", "lan3", "lan4", "wan1", "wan2"].map((opt) => (
              <button
                key={opt}
                onClick={() => onSelect(choice.id, opt)}
                style={{
                  padding: "8px 12px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                  cursor: "pointer",
                  transition: "border-color 0.15s ease",
                }}
                onMouseEnter={(e) => e.currentTarget.style.borderColor = "var(--primary)"}
                onMouseLeave={(e) => e.currentTarget.style.borderColor = "var(--border)"}
              >
                {opt}
              </button>
            ))}
          </div>
          <div style={{ borderTop: "1px solid var(--border)", paddingTop: "8px", display: "flex", flexDirection: "column", gap: "4px" }}>
            <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>カスタム入力</label>
            <div style={{ display: "flex", gap: "10px" }}>
              <input
                type="text"
                value={customInterface}
                onChange={(e) => setCustomInterface(e.target.value)}
                placeholder="例: lan1.1, tunnel1"
                style={{
                  flex: 1,
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
              <button
                className="btn btn-primary"
                onClick={() => {
                  if (customInterface.trim()) {
                    onSelect(choice.id, customInterface.trim());
                  }
                }}
                disabled={!customInterface.trim()}
                style={{
                  padding: "8px 16px",
                  fontWeight: "500",
                }}
              >
                確定
              </button>
            </div>
          </div>
        </div>
      )}

      {/* その他のベンダー (Cisco, Yamaha 以外) の UI */}
      {!isCisco && !isYamaha && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {isArista && (
            <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", marginBottom: "4px" }}>
              {["Ethernet1", "Ethernet2", "Ethernet3", "Ethernet4"].map((opt) => (
                <button
                  key={opt}
                  onClick={() => onSelect(choice.id, opt)}
                  style={{
                    padding: "6px 10px",
                    background: "var(--bg-tertiary)",
                    border: "1px solid var(--border)",
                    borderRadius: "6px",
                    color: "var(--text-primary)",
                    cursor: "pointer",
                    fontSize: "12px",
                  }}
                >
                  {opt}
                </button>
              ))}
            </div>
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
            <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>インターフェース名を入力してください</label>
            <div style={{ display: "flex", gap: "10px" }}>
              <input
                type="text"
                value={customInterface}
                onChange={(e) => setCustomInterface(e.target.value)}
                placeholder="例: Ethernet1, ge-0/0/0"
                style={{
                  flex: 1,
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
              <button
                className="btn btn-primary"
                onClick={() => {
                  if (customInterface.trim()) {
                    onSelect(choice.id, customInterface.trim());
                  }
                }}
                disabled={!customInterface.trim()}
                style={{
                  padding: "8px 16px",
                  fontWeight: "500",
                }}
              >
                確定
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface IpAddressChoicePanelProps {
  choice: {
    id: string;
    title: string;
    message: string;
    subnet: string;
    defaultIp?: string;
  };
  progressPrefix: string;
  onSelect: (id: string, option: string) => void;
  onCancel: (id: string) => void;
}

function ip2long(ip: string): number {
  const parts = ip.split('.').map(Number);
  if (parts.length !== 4 || parts.some(isNaN) || parts.some(p => p < 0 || p > 255)) {
    return -1;
  }
  return ((parts[0] << 24) >>> 0) + (parts[1] << 16) + (parts[2] << 8) + parts[3];
}

function isIpInSubnet(ip: string, subnet: string): boolean {
  const ipLong = ip2long(ip);
  if (ipLong === -1) return false;

  const parts = subnet.split('/');
  const subnetIp = parts[0];
  const subnetIpLong = ip2long(subnetIp);
  if (subnetIpLong === -1) return false;

  let maskLength = 32;
  if (parts.length > 1) {
    const maskStr = parts[1];
    if (maskStr.includes('.')) {
      const maskLong = ip2long(maskStr);
      if (maskLong === -1) return false;
      return (ipLong & maskLong) === (subnetIpLong & maskLong);
    } else {
      maskLength = parseInt(maskStr, 10);
      if (isNaN(maskLength) || maskLength < 0 || maskLength > 32) return false;
    }
  }

  if (maskLength === 0) return true;
  const mask = maskLength === 32 ? 0xffffffff : ~((1 << (32 - maskLength)) - 1);
  return (ipLong & mask) === (subnetIpLong & mask);
}

function validateIpAndSubnet(ip: string, subnetInput: string, requiredSubnet?: string): { isValid: boolean; error?: string } {
  const ipLong = ip2long(ip);
  if (ipLong === -1) {
    return { isValid: false, error: "無効なIPアドレスの形式です (例: 192.168.1.1)" };
  }

  let isValidMask = false;
  let maskText = subnetInput.trim();
  if (maskText.startsWith('/')) {
    maskText = maskText.substring(1);
  }
  
  if (/^\d+$/.test(maskText)) {
    const num = parseInt(maskText, 10);
    if (num >= 0 && num <= 32) {
      isValidMask = true;
    }
  } else {
    const maskLong = ip2long(maskText);
    if (maskLong !== -1) {
      const inv = ~maskLong >>> 0;
      if (((inv + 1) & inv) === 0) {
        isValidMask = true;
      }
    }
  }

  if (!isValidMask && maskText !== "") {
    return { isValid: false, error: "無効なサブネットマスクまたはプレフィックス長です (例: 255.255.255.0 または 24)" };
  }

  if (requiredSubnet && requiredSubnet.trim() !== "") {
    if (requiredSubnet.includes('/')) {
      const parts = requiredSubnet.split('/');
      const netIp = parts[0];
      if (ip2long(netIp) !== -1) {
        if (!isIpInSubnet(ip, requiredSubnet)) {
          return { isValid: false, error: `IPアドレスは指定されたサブネット範囲 (${requiredSubnet}) 内である必要があります` };
        }
      }
    }
  }

  return { isValid: true };
}

function IpAddressChoicePanel({ choice, progressPrefix, onSelect, onCancel }: IpAddressChoicePanelProps) {
  const initialIp = choice.defaultIp || "";
  let initialSubnet = "";
  if (choice.subnet) {
    if (choice.subnet.includes('/')) {
      initialSubnet = choice.subnet.split('/')[1];
    } else {
      initialSubnet = choice.subnet;
    }
  } else {
    initialSubnet = "24";
  }

  const [ipAddress, setIpAddress] = useState(initialIp);
  const [subnetMask, setSubnetMask] = useState(initialSubnet);
  const [validationError, setValidationError] = useState<string | undefined>(undefined);

  useEffect(() => {
    if (!ipAddress && !subnetMask) {
      setValidationError(undefined);
      return;
    }
    const result = validateIpAndSubnet(ipAddress, subnetMask, choice.subnet);
    if (!result.isValid) {
      setValidationError(result.error);
    } else {
      setValidationError(undefined);
    }
  }, [ipAddress, subnetMask, choice.subnet]);

  const handleSubmit = () => {
    const result = validateIpAndSubnet(ipAddress, subnetMask, choice.subnet);
    if (result.isValid) {
      let formattedSubnet = subnetMask.trim();
      if (formattedSubnet.startsWith('/')) {
        formattedSubnet = formattedSubnet.substring(1);
      }
      const isPrefix = /^\d+$/.test(formattedSubnet);
      
      let output = "";
      if (isPrefix) {
        output = `${ipAddress}/${formattedSubnet}`;
      } else {
        output = `${ipAddress} ${formattedSubnet}`;
      }
      onSelect(choice.id, output);
    }
  };

  const isSubnetCidr = choice.subnet && choice.subnet.includes('/');

  return (
    <div className="input-choice-panel" style={{
      background: "var(--bg-secondary)",
      border: "1px solid var(--border)",
      borderRadius: "8px",
      padding: "16px",
      boxShadow: "0 -2px 10px rgba(0,0,0,0.15)",
      display: "flex",
      flexDirection: "column",
      gap: "12px",
      animation: "fadeIn 0.2s ease",
    }}>
      <div style={{ fontWeight: "600", fontSize: "14px", color: "var(--text-primary)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span>{progressPrefix} {choice.title || "IPアドレスの設定"}</span>
        <button
          onClick={() => onCancel(choice.id)}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-secondary)",
            cursor: "pointer",
            fontSize: "11px",
            padding: "2px 6px",
            borderRadius: "4px",
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = "var(--bg-tertiary)"}
          onMouseLeave={(e) => e.currentTarget.style.background = "transparent"}
        >
          キャンセル (Esc)
        </button>
      </div>

      {choice.message && (
        <div style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "4px", whiteSpace: "pre-wrap" }}>
          {choice.message}
        </div>
      )}

      {isSubnetCidr && (
        <div style={{
          fontSize: "12px",
          background: "rgba(59, 130, 246, 0.1)",
          border: "1px solid rgba(59, 130, 246, 0.2)",
          color: "var(--primary)",
          padding: "8px 12px",
          borderRadius: "6px",
          fontWeight: "500",
          display: "flex",
          alignItems: "center",
          gap: "6px"
        }}>
          <span style={{ fontSize: "14px" }}>ℹ️</span>
          <span>要求サブネット範囲: <strong>{choice.subnet}</strong></span>
        </div>
      )}

      <div style={{ display: "flex", gap: "10px" }}>
        <div style={{ flex: 2, display: "flex", flexDirection: "column", gap: "4px" }}>
          <label style={{ fontSize: "11px", color: "var(--text-secondary)", fontWeight: "500" }}>IPアドレス</label>
          <input
            type="text"
            value={ipAddress}
            onChange={(e) => setIpAddress(e.target.value)}
            placeholder="例: 192.168.1.1"
            style={{
              padding: "10px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              borderRadius: "6px",
              color: "var(--text-primary)",
              fontSize: "13px",
            }}
          />
        </div>
        <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "4px" }}>
          <label style={{ fontSize: "11px", color: "var(--text-secondary)", fontWeight: "500" }}>サブネットマスク / プレフィックス</label>
          <input
            type="text"
            value={subnetMask}
            onChange={(e) => setSubnetMask(e.target.value)}
            placeholder="例: 24, 255.255.255.0"
            style={{
              padding: "10px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              borderRadius: "6px",
              color: "var(--text-primary)",
              fontSize: "13px",
                }}
              />
            </div>
          </div>

          {validationError && (
            <div style={{
              fontSize: "12px",
              color: "#ef4444",
              background: "rgba(239, 68, 68, 0.08)",
              padding: "8px 12px",
              borderRadius: "6px",
              border: "1px solid rgba(239, 68, 68, 0.15)",
              display: "flex",
              alignItems: "center",
              gap: "6px"
            }}>
              <span>⚠️</span>
              <span>{validationError}</span>
            </div>
          )}

          <button
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={!!validationError || !ipAddress || !subnetMask}
            style={{
              width: "100%",
              padding: "10px",
              fontWeight: "500",
              marginTop: "4px",
              opacity: (validationError || !ipAddress || !subnetMask) ? 0.6 : 1,
              cursor: (validationError || !ipAddress || !subnetMask) ? "not-allowed" : "pointer"
            }}
          >
            設定を確定
          </button>
        </div>
      );
    }

