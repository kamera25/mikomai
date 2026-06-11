import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "katex/dist/katex.min.css";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "./components/ScheduledTasksPanel";
import "./App.css";

import { SummaryItem, Connection, McpHost } from './types';
import { Chat } from "./components/Chat/Chat";
import { ChatInput } from "./components/ChatInput/ChatInput";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { ActivityBar } from "./components/ActivityBar/ActivityBar";
import { useMcp } from "./hooks/useMcp";
import { useHistory } from "./hooks/useHistory";
import { StatusBar } from "./components/StatusBar/StatusBar";
import {
  DEFAULT_HISTORY_LIMIT,
  DEFAULT_TEMPERATURE,
  DEFAULT_REPETITION_PENALTY,
  DEFAULT_MODEL_PATH,
  DEFAULT_MCP_TIMEOUT,
  DEFAULT_DB_PATH,
  DEFAULT_IP_VERSION,
} from "./constants/defaults";

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
  } = useHistory();

  const [input, setInput] = useState("");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isConnectionOpen, setIsConnectionOpen] = useState(false);
  const [isScheduledTasksOpen, setIsScheduledTasksOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [modelStatus, setModelStatus] = useState<string>("NotLoaded");
  const [summaries, setSummaries] = useState<SummaryItem[]>([]);
  const [historyLimit, setHistoryLimit] = useState<number>(DEFAULT_HISTORY_LIMIT);
  const [temperature, setTemperature] = useState<number>(DEFAULT_TEMPERATURE);
  const [repetitionPenalty, setRepetitionPenalty] = useState<number>(DEFAULT_REPETITION_PENALTY);
  const [modelPath, setModelPath] = useState<string | null>(DEFAULT_MODEL_PATH);
  const [mcpTimeout, setMcpTimeout] = useState<number>(DEFAULT_MCP_TIMEOUT);
  const [dbPath, setDbPath] = useState<string>(DEFAULT_DB_PATH);
  const [ipVersion, setIpVersion] = useState<string>(DEFAULT_IP_VERSION);
  
  // Host Suggestion states
  const [availableHosts, setAvailableHosts] = useState<{hostname: string, ip: string}[]>([]);
  const [recentIPs, setRecentIPs] = useState<string[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [filteredSuggestions, setFilteredSuggestions] = useState<{hostname: string, ip: string}[]>([]);
  const [suggestionIndex, setSuggestionIndex] = useState(0);
  const [cursorPos, setCursorPos] = useState(0);

  const { handleMcpResponse } = useMcp({
    messages,
    setMessages,
    summaries,
    setSummaries,
    historyLimit,
    mcpTimeout
  });

  const isComposing = useRef(false);
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  // Load summaries and settings from backend
  useEffect(() => {
    const initSettingsAndSummaries = async () => {
      try {
        const savedSummaries = await invoke<SummaryItem[]>("load_summaries");
        setSummaries(savedSummaries || []);
      } catch (e) {
        console.error("Failed to load summaries:", e);
      }

      try {
        const settings: any = await invoke("load_settings");
        if (settings && settings.historyLimit !== undefined) {
          setHistoryLimit(settings.historyLimit);
        }
        if (settings && settings.temperature !== undefined) {
          setTemperature(settings.temperature);
        }
        if (settings && settings.repetitionPenalty !== undefined) {
          setRepetitionPenalty(settings.repetitionPenalty);
        }
        if (settings && settings.modelPath !== undefined) {
          setModelPath(settings.modelPath);
        }
        if (settings && settings.recentIps !== undefined) {
          setRecentIPs(settings.recentIps);
        }
        if (settings && settings.mcpTimeout !== undefined) {
          setMcpTimeout(settings.mcpTimeout);
        }
        if (settings && settings.dbPath) {
          setDbPath(settings.dbPath);
        }
        if (settings && settings.ipVersion !== undefined) {
          setIpVersion(settings.ipVersion);
        }
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    };
    initSettingsAndSummaries();
  }, []);

  const fetchHosts = useCallback(async (hostToResolve?: string) => {
    try {
      const [connections, mcpHosts] = await Promise.all([
        invoke<Connection[]>("load_connections"),
        invoke<McpHost[]>("get_mcp_hosts")
      ]);
      
      const hostMap = new Map<string, string>();
      if (connections) {
        connections.forEach(c => {
          if (c.hostname && c.ip) hostMap.set(c.hostname, c.ip);
        });
      }
      if (mcpHosts) {
        mcpHosts.forEach(h => {
          if (h.hostname && h.ip) hostMap.set(h.hostname, h.ip);
        });
      }

      // Active resolution for new IP addresses
      if (hostToResolve && /^(?:\d{1,3}\.){3}\d{1,3}$/.test(hostToResolve)) {
        const isKnown = Array.from(hostMap.values()).includes(hostToResolve);
        if (!isKnown) {
          try {
            const resolvedName = await invoke<string>("resolve_ip", { ip: hostToResolve });
            if (resolvedName) {
              hostMap.set(resolvedName, hostToResolve);
            }
          } catch (e) {
            // Silently fail if resolution fails
          }
        }
      }
      
      const hostsArray = Array.from(hostMap.entries()).map(([hostname, ip]) => ({
        hostname,
        ip
      }));
      
      setAvailableHosts(hostsArray);
    } catch (e) {
      console.error("Failed to fetch hosts for suggestions:", e);
    }
  }, []);

  // Initial fetch for hosts
  useEffect(() => {
    fetchHosts();
  }, [fetchHosts]);

  // Trigger name resolution/host fetch when active IP/host changes
  useEffect(() => {
    if (recentIPs.length > 0) {
      fetchHosts(recentIPs[0]);
    }
  }, [recentIPs[0], fetchHosts]);



  // Poll model status
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const status = await invoke<any>("get_model_status");
        if (typeof status === 'string') {
          setModelStatus(status);
        } else if (typeof status === 'object' && status !== null) {
          // Handle Error(string) case
          if ('Error' in status) {
            setModelStatus('Error');
          }
        }
      } catch (e) {
        console.error("Failed to get model status:", e);
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 2000);
    return () => clearInterval(interval);
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


  const handleSelectSuggestion = (hostObj: {hostname: string, ip: string}) => {
    const host = hostObj.hostname;
    const textBeforeCursor = input.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf('@');
    const newValue = input.slice(0, atIndex) + host + ' ' + input.slice(cursorPos);
    setInput(newValue);
    setShowSuggestions(false);
    
    // Focus back to textarea and set cursor position
    setTimeout(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        const newPos = atIndex + host.length + 1;
        textareaRef.current.setSelectionRange(newPos, newPos);
      }
    }, 0);
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
      const newRecent = [
        ...new Set([...allFound, ...recentIPs])
      ].slice(0, 10);
      setRecentIPs(newRecent);
      
      // Save updated hosts to backend settings
      try {
        await invoke("save_settings", { 
          settings: { 
            historyLimit, 
            temperature, 
            repetitionPenalty, 
            modelPath,
            recentIps: newRecent,
            mcpTimeout,
            dbPath,
            ipVersion
          } 
        });
      } catch (e) {
        console.error("Failed to save recent hosts to settings:", e);
      }
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
  
  const handleLoadModel = async () => {
    if (!modelPath) return;
    try {
      setModelStatus("Loading");
      await invoke("load_model", { path: modelPath });
      setModelStatus("Loaded");
    } catch (e) {
      console.error("Failed to load model:", e);
      setModelStatus("Error");
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
            onHistoryLimitChange={async (newLimit) => {
              setHistoryLimit(newLimit);
              try {
                await invoke("save_settings", { settings: { historyLimit: newLimit, temperature, repetitionPenalty, modelPath, recentIps: recentIPs, mcpTimeout, dbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            temperature={temperature}
            onTemperatureChange={async (newTemp) => {
              setTemperature(newTemp);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature: newTemp, repetitionPenalty, modelPath, recentIps: recentIPs, mcpTimeout, dbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            repetitionPenalty={repetitionPenalty}
            onRepetitionPenaltyChange={async (newPenalty) => {
              setRepetitionPenalty(newPenalty);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty: newPenalty, modelPath, recentIps: recentIPs, mcpTimeout, dbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            modelPath={modelPath}
            onModelPathChange={async (newPath) => {
              setModelPath(newPath);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty, modelPath: newPath, recentIps: recentIPs, mcpTimeout, dbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            mcpTimeout={mcpTimeout}
            onMcpTimeoutChange={async (newTimeout: number) => {
              setMcpTimeout(newTimeout);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty, modelPath, recentIps: recentIPs, mcpTimeout: newTimeout, dbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            dbPath={dbPath}
            onDbPathChange={async (newDbPath) => {
              setDbPath(newDbPath);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty, modelPath, recentIps: recentIPs, mcpTimeout, dbPath: newDbPath, ipVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            ipVersion={ipVersion}
            onIpVersionChange={async (newIpVersion) => {
              setIpVersion(newIpVersion);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty, modelPath, recentIps: recentIPs, mcpTimeout, dbPath, ipVersion: newIpVersion } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
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
                <h1 className="header-title">{activeSession?.title || "mikomai"}</h1>
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
              isComposing={isComposing}
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
