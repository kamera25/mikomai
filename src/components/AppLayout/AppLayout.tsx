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
  const [selectModalConfig, setSelectModalConfig] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    options: string[];
    onSelect: (option: string) => void;
    onCancel: () => void;
  } | null>(null);
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

  const [interfaceModalConfig, setInterfaceModalConfig] = useState<{
    isOpen: boolean;
    vendor: string;
    onSelect: (option: string) => void;
    onCancel: () => void;
  } | null>(null);

  const [ciscoType, setCiscoType] = useState("GigabitEthernet");
  const [ciscoNum, setCiscoNum] = useState("0/1");
  const [customInterface, setCustomInterface] = useState("");

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

  // Listen to user choice requests from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-user-choice", (event) => {
      const { title, message, options } = event.payload;
      setSelectModalConfig({
        isOpen: true,
        title,
        message,
        options,
        onSelect: async (option: string) => {
          setSelectModalConfig(null);
          try {
            await invoke("submit_user_choice", { choice: option });
          } catch (err) {
            console.error("Failed to submit user choice:", err);
          }
        },
        onCancel: async () => {
          setSelectModalConfig(null);
          try {
            await invoke("submit_user_choice", { choice: "cancelled" });
          } catch (err) {
            console.error("Failed to cancel user choice:", err);
          }
        }
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to interface choice requests from Rust
  useEffect(() => {
    const unlisten = listen<any>("request-interface-choice", (event) => {
      const { vendor } = event.payload;
      setInterfaceModalConfig({
        isOpen: true,
        vendor: vendor || "Cisco_IOS",
        onSelect: async (option: string) => {
          setInterfaceModalConfig(null);
          try {
            await invoke("submit_interface_choice", { choice: option });
          } catch (err) {
            console.error("Failed to submit interface choice:", err);
          }
        },
        onCancel: async () => {
          setInterfaceModalConfig(null);
          try {
            await invoke("submit_interface_choice", { choice: "cancelled" });
          } catch (err) {
            console.error("Failed to cancel interface choice:", err);
          }
        }
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to keyboard Escape when interface choice panel is active
  useEffect(() => {
    if (!interfaceModalConfig || !interfaceModalConfig.isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        interfaceModalConfig.onCancel();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [interfaceModalConfig]);

  // Listen to keyboard numeric selections when choice panel is active
  useEffect(() => {
    if (!selectModalConfig || !selectModalConfig.isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const num = parseInt(e.key, 10);
      if (!isNaN(num) && num >= 1 && num <= selectModalConfig.options.length) {
        e.preventDefault();
        selectModalConfig.onSelect(selectModalConfig.options[num - 1]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        selectModalConfig.onCancel();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [selectModalConfig]);

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

                <div className="input-area-wrapper" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                  {chatState.messages.some((m) => m.status === "Running") && (
                    <div className="global-loading-indicator"></div>
                  )}
                  {selectModalConfig && selectModalConfig.isOpen && (
                    <div className="input-choice-panel" style={{
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
                        <span>{selectModalConfig.message}</span>
                        <button
                          onClick={selectModalConfig.onCancel}
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
                        {selectModalConfig.options.map((opt, idx) => (
                          <button
                            key={idx}
                            onClick={() => selectModalConfig.onSelect(opt)}
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
                  )}
                  {interfaceModalConfig && interfaceModalConfig.isOpen && (
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
                        <span>インターフェースの選択 - {interfaceModalConfig.vendor}</span>
                        <button
                          onClick={interfaceModalConfig.onCancel}
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

                      {/* Cisco_IOS の UI */}
                      {(interfaceModalConfig.vendor.toLowerCase().includes("cisco") || interfaceModalConfig.vendor.toLowerCase().includes("ios")) && (
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
                            onClick={() => interfaceModalConfig.onSelect(`${ciscoType}${ciscoNum}`)}
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
                      {interfaceModalConfig.vendor.toLowerCase().includes("yamaha") && (
                        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
                            {["lan1", "lan2", "lan3", "lan4", "wan1", "wan2"].map((opt) => (
                              <button
                                key={opt}
                                onClick={() => interfaceModalConfig.onSelect(opt)}
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
                                    interfaceModalConfig.onSelect(customInterface.trim());
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
                      {!interfaceModalConfig.vendor.toLowerCase().includes("cisco") && 
                       !interfaceModalConfig.vendor.toLowerCase().includes("ios") && 
                       !interfaceModalConfig.vendor.toLowerCase().includes("yamaha") && (
                        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                          {/* Arista用に Ethernet1 などのクイック選択を提供 */}
                          {interfaceModalConfig.vendor.toLowerCase().includes("arista") && (
                            <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", marginBottom: "4px" }}>
                              {["Ethernet1", "Ethernet2", "Ethernet3", "Ethernet4"].map((opt) => (
                                <button
                                  key={opt}
                                  onClick={() => interfaceModalConfig.onSelect(opt)}
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
                                    interfaceModalConfig.onSelect(customInterface.trim());
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
