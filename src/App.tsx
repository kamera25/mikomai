import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "katex/dist/katex.min.css";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "./components/ScheduledTasksPanel";
import "./App.css";

import { SummaryItem } from './types';
import { Chat } from "./components/Chat/Chat";
import { ChatInput } from "./components/ChatInput/ChatInput";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { ActivityBar } from "./components/ActivityBar/ActivityBar";
import { useMcp } from "./hooks/useMcp";
import { useHistory } from "./hooks/useHistory";
import { StatusBar } from "./components/StatusBar/StatusBar";
import { useSettings } from "./hooks/useSettings";
import { useModel } from "./hooks/useModel";
import { useHostSuggestions } from "./hooks/useHostSuggestions";

function App() {
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

  const {
    historyLimit,
    setHistoryLimit,
    temperature,
    setTemperature,
    repetitionPenalty,
    setRepetitionPenalty,
    modelPath,
    setModelPath,
    mcpTimeout,
    setMcpTimeout,
    cacheExpiryMinutes,
    setCacheExpiryMinutes,
    dbPath,
    setDbPath,
    ipVersion,
    setIpVersion,
    consolePort,
    setConsolePort,
    consoleBaudRate,
    setConsoleBaudRate,
    preloadInvestigate,
    setPreloadInvestigate,
    preloadKnowledge,
    setPreloadKnowledge,
    preloadAnalysis,
    setPreloadAnalysis,
    preloadRag,
    setPreloadRag,
    recentIPs,
    setRecentIPs,
    saveAllSettings,
  } = useSettings();

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
    cursorPos,
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
    const isDifferent = sessionIps.length !== recentIPs.length || sessionIps.some((val, idx) => val !== recentIPs[idx]);
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
    recentIPs
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
    
    const isToday = date.getFullYear() === now.getFullYear() &&
                    date.getMonth() === now.getMonth() &&
                    date.getDate() === now.getDate();
    
    const timeStr = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    
    if (isToday) {
      return timeStr;
    } else {
      const dateStr = date.toLocaleDateString([], { year: 'numeric', month: '2-digit', day: '2-digit' }).replace(/\//g, '/');
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
    const foundMentions = Array.from(userMessage.matchAll(mentionRegex)).map(m => m[1]);
    
    const allFound = [...new Set([...foundMentions, ...foundIPs])];
    
    if (allFound.length > 0) {
      updateRecentHosts(allFound);
    }

    setInput("");
    setMessages(prev => [...prev, {
      role: "user",
      content: userMessage,
      timestamp,
      event_type: "UserInput",
      task_id: `task_user_${Date.now()}`
    }]);

    // Force scroll to bottom on new user input
    setTimeout(() => {
      scrollToBottom();
    }, 100);

    // Use MCP hook to handle the response
    setTimeout(async () => {
      await handleMcpResponse(userMessage);
    }, 500);
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
          <SettingsPanel 
            isOpen={isSettingsOpen} 
            onClose={() => setIsSettingsOpen(false)} 
            historyLimit={historyLimit}
            onHistoryLimitChange={(newLimit) => {
              setHistoryLimit(newLimit);
              saveAllSettings({ historyLimit: newLimit });
            }}
            temperature={temperature}
            onTemperatureChange={(newTemp) => {
              setTemperature(newTemp);
              saveAllSettings({ temperature: newTemp });
            }}
            repetitionPenalty={repetitionPenalty}
            onRepetitionPenaltyChange={(newPenalty) => {
              setRepetitionPenalty(newPenalty);
              saveAllSettings({ repetitionPenalty: newPenalty });
            }}
            modelPath={modelPath}
            onModelPathChange={(newPath) => {
              setModelPath(newPath);
              saveAllSettings({ modelPath: newPath });
            }}
            mcpTimeout={mcpTimeout}
            onMcpTimeoutChange={(newTimeout: number) => {
              setMcpTimeout(newTimeout);
              saveAllSettings({ mcpTimeout: newTimeout });
            }}
            cacheExpiryMinutes={cacheExpiryMinutes}
            onCacheExpiryMinutesChange={(newExpiry: number) => {
              setCacheExpiryMinutes(newExpiry);
              saveAllSettings({ cacheExpiryMinutes: newExpiry });
            }}
            dbPath={dbPath}
            onDbPathChange={(newDbPath) => {
              setDbPath(newDbPath);
              saveAllSettings({ dbPath: newDbPath });
            }}
            ipVersion={ipVersion}
            onIpVersionChange={(newIpVersion) => {
              setIpVersion(newIpVersion);
              saveAllSettings({ ipVersion: newIpVersion });
            }}
            consolePort={consolePort}
            onConsolePortChange={(newPort) => {
              setConsolePort(newPort);
              saveAllSettings({ consolePort: newPort });
            }}
            consoleBaudRate={consoleBaudRate}
            onConsoleBaudRateChange={(newRate) => {
              setConsoleBaudRate(newRate);
              saveAllSettings({ consoleBaudRate: newRate });
            }}
            preloadInvestigate={preloadInvestigate}
            onPreloadInvestigateChange={(val) => {
              setPreloadInvestigate(val);
              saveAllSettings({ preloadInvestigate: val });
            }}
            preloadKnowledge={preloadKnowledge}
            onPreloadKnowledgeChange={(val) => {
              setPreloadKnowledge(val);
              saveAllSettings({ preloadKnowledge: val });
            }}
            preloadAnalysis={preloadAnalysis}
            onPreloadAnalysisChange={(val) => {
              setPreloadAnalysis(val);
              saveAllSettings({ preloadAnalysis: val });
            }}
            preloadRag={preloadRag}
            onPreloadRagChange={(val) => {
              setPreloadRag(val);
              saveAllSettings({ preloadRag: val });
            }}
          />
        ) : isConnectionOpen ? (
          <ConnectionSettingsPanel 
            onClose={() => setIsConnectionOpen(false)} 
            onConnectionsChanged={fetchHosts}
          />
        ) : isScheduledTasksOpen ? (
          <ScheduledTasksPanel 
            onClose={() => setIsScheduledTasksOpen(false)} 
          />
        ) : (
          <main className="main-chat">
            {/* Top Header */}
            <header className="chat-header">
              <div className="header-left">
                <button 
                  className="sidebar-toggle-button" 
                  onClick={() => setIsSidebarOpen(!isSidebarOpen)}
                  title={isSidebarOpen ? "サイドバーを閉じる" : "サイドバーを開く"}
                >
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                     <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                     <line x1="9" y1="3" x2="9" y2="21"></line>
                  </svg>
                </button>
                {isEditingHeader ? (
                  <input
                    className="header-title-input"
                    value={headerTitle}
                    onChange={(e) => setHeaderTitle(e.target.value)}
                    onBlur={handleSaveRenameHeader}
                    onCompositionStart={() => { isComposingHeader.current = true; }}
                    onCompositionEnd={() => {
                      setTimeout(() => { isComposingHeader.current = false; }, 150);
                    }}
                    onKeyDown={(e) => {
                      if (isComposingHeader.current || e.nativeEvent.isComposing || e.keyCode === 229) {
                        return;
                      }
                      if (e.key === 'Enter') {
                        handleSaveRenameHeader();
                      } else if (e.key === 'Escape') {
                        setIsEditingHeader(false);
                      }
                    }}
                    autoFocus
                    style={{
                      fontSize: '1.25rem',
                      fontWeight: '600',
                      background: 'transparent',
                      border: '1px solid var(--border-color)',
                      color: 'var(--text-color)',
                      borderRadius: '4px',
                      padding: '2px 8px',
                      outline: 'none',
                    }}
                  />
                ) : (
                  <h1 
                    className="header-title" 
                    onDoubleClick={handleStartRenameHeader}
                    style={{ cursor: 'pointer' }}
                    title="ダブルクリックしてリネーム"
                  >
                    {activeSession?.title || "mikomai"}
                  </h1>
                )}
                {recentIPs.length > 0 && (
                  <div>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
                    <span className="header-hostname">
                      {(() => {
                        const current = recentIPs[0];
                        const host = availableHosts.find(h => h.ip === current || h.hostname === current);
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
            <Chat ref={messagesEndRef} messages={messages} formatMessageTime={formatMessageTime} />
  
          {/* Input Area */}
          <div className="input-area-wrapper">
            {messages.some(m => m.status === 'Running') && (
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
    </div>
  );
}

export default App;
