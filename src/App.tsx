import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "katex/dist/katex.min.css";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "./components/ScheduledTasksPanel";
import "./App.css";

import { Message, SummaryItem, Connection, McpHost, ChatSession, HistoryItem } from './types';
import { Chat } from "./components/Chat/Chat";
import { ChatInput } from "./components/ChatInput/ChatInput";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { ActivityBar } from "./components/ActivityBar/ActivityBar";
import { useMcp } from "./hooks/useMcp";

function App() {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isConnectionOpen, setIsConnectionOpen] = useState(false);
  const [isScheduledTasksOpen, setIsScheduledTasksOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [modelStatus, setModelStatus] = useState<string>("NotLoaded");
  const [connectedHost] = useState<string>("192.168.1.1 (Core-Switch-01)");
  const [summaries, setSummaries] = useState<SummaryItem[]>([]);
  const [historyLimit, setHistoryLimit] = useState<number>(5);
  const [temperature, setTemperature] = useState<number>(0.0);
  const [repetitionPenalty, setRepetitionPenalty] = useState<number>(1.1);
  const [modelPath, setModelPath] = useState<string | null>(null);
  
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
    historyLimit
  });

  const isComposing = useRef(false);
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  // Load history from backend
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
          const defaultHistory: HistoryItem[] = [{
            id: defaultId,
            type: 'session',
            title: "New Session",
            messages: []
          }];
          setHistory(defaultHistory);
          setActiveSessionId(defaultId);
        }
      } catch (e) {
        console.error("Failed to load history:", e);
      } finally {
        setIsLoaded(true);
      }

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
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    };
    initHistory();

    const fetchHosts = async () => {
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
        
        const hostsArray = Array.from(hostMap.entries()).map(([hostname, ip]) => ({
          hostname,
          ip
        }));
        
        setAvailableHosts(hostsArray);
      } catch (e) {
        console.error("Failed to fetch hosts for suggestions:", e);
      }
    };
    fetchHosts();
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

  const findFirstSession = (items: HistoryItem[]): ChatSession | undefined => {
    for (const item of items) {
      if (item.type === 'session') return item;
      if (item.type === 'folder') {
        const found = findFirstSession(item.items);
        if (found) return found;
      }
    }
    return undefined;
  };

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
    
    // Extract IP addresses to remember
    const ipRegex = /\b(?:\d{1,3}\.){3}\d{1,3}\b/g;
    const foundIPs = userMessage.match(ipRegex);
    if (foundIPs) {
      const newRecent = [
        ...new Set([...foundIPs, ...recentIPs])
      ].slice(0, 10);
      setRecentIPs(newRecent);
      
      // Save updated IPs to backend settings
      try {
        await invoke("save_settings", { 
          settings: { 
            historyLimit, 
            temperature, 
            repetitionPenalty, 
            modelPath,
            recentIps: newRecent 
          } 
        });
      } catch (e) {
        console.error("Failed to save recent IPs to settings:", e);
      }
    }

    setInput("");
    setMessages(prev => [...prev, { role: "user", content: userMessage, timestamp }]);

    // Use MCP hook to handle the response
    setTimeout(async () => {
      await handleMcpResponse(userMessage);
    }, 500);
  };

  // Helper to find a session in the tree
  const findSession = (items: HistoryItem[], id: string): ChatSession | undefined => {
    for (const item of items) {
      if (item.type === 'session' && item.id === id) return item;
      if (item.type === 'folder') {
        const found = findSession(item.items, id);
        if (found) return found;
      }
    }
    return undefined;
  };

  // Sync messages when active session changes
  useEffect(() => {
    const session = findSession(history, activeSessionId);
    if (session) {
      setMessages(session.messages);
    }
  }, [activeSessionId]);
  
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

  // Update history when messages change
  useEffect(() => {
    if (messages.length === 0) return;
    setHistory(prev => {
      const updateSessionMessages = (items: HistoryItem[]): HistoryItem[] => {
        return items.map(item => {
          if (item.id === activeSessionId && item.type === 'session') {
            return { ...item, messages };
          }
          if (item.type === 'folder') {
            return { ...item, items: updateSessionMessages(item.items) };
          }
          return item;
        });
      };
      return updateSessionMessages(prev);
    });
  }, [messages, activeSessionId]);

    return (
    <div className="app-container">
      <div className="main-layout">
        {/* Activity Bar (LM Studio style thin left bar) */}
        <ActivityBar
          isSidebarOpen={isSidebarOpen}
          setIsSidebarOpen={setIsSidebarOpen}
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
        createNewFolder={() => {
          const folderName = prompt("フォルダ名を入力してください");
          if (folderName) {
            setHistory(prev => [{
              id: `folder-${Date.now()}`,
              type: 'folder',
              name: folderName,
              isOpen: true,
              items: []
            }, ...prev]);
          }
        }}
        createNewSession={() => {
          const id = `session-${Date.now()}`;
          setHistory(prev => [{
            id,
            type: 'session',
            title: "新しいセッション",
            messages: []
          }, ...prev]);
          setActiveSessionId(id);
          setMessages([]);
        }}
        toggleFolder={(folderId: string) => {
          setHistory(prev => {
            const toggleNode = (items: HistoryItem[]): HistoryItem[] => {
              return items.map(item => {
                if (item.type === 'folder') {
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
        }}
        switchSession={async (sessionId: string) => {
          setActiveSessionId(sessionId);
          const findSession = (items: HistoryItem[]): ChatSession | null => {
            for (const item of items) {
              if (item.type === 'session' && item.id === sessionId) {
                return item;
              }
              if (item.type === 'folder') {
                const found = findSession(item.items);
                if (found) return found;
              }
            }
            return null;
          };
          const session = findSession(history);
          if (session) {
            setMessages(session.messages);
          }
        }}
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
                await invoke("save_settings", { settings: { historyLimit: newLimit, temperature, repetitionPenalty, modelPath, recentIps: recentIPs } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            temperature={temperature}
            onTemperatureChange={async (newTemp) => {
              setTemperature(newTemp);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature: newTemp, repetitionPenalty, modelPath, recentIps: recentIPs } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            repetitionPenalty={repetitionPenalty}
            onRepetitionPenaltyChange={async (newPenalty) => {
              setRepetitionPenalty(newPenalty);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty: newPenalty, modelPath, recentIps: recentIPs } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
            modelPath={modelPath}
            onModelPathChange={async (newPath) => {
              setModelPath(newPath);
              try {
                await invoke("save_settings", { settings: { historyLimit, temperature, repetitionPenalty, modelPath: newPath, recentIps: recentIPs } });
              } catch (e) {
                console.error("Failed to save settings:", e);
              }
            }}
          />
        ) : isConnectionOpen ? (
          <ConnectionSettingsPanel 
            onClose={() => setIsConnectionOpen(false)} 
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
                <h1 className="header-title">mikomai</h1>
                {recentIPs.length > 0 && (
                  <span className="header-hostname">{recentIPs[0]}</span>
                )}
              </div>
            </header>
    
            {/* Chat History */}
            <Chat ref={messagesEndRef} messages={messages} formatMessageTime={formatMessageTime} />
  
          {/* Input Area */}
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
        </main>
      )}
    </div>
  </div>
      
      {/* Status Bar */}
      <footer className="status-bar">
        <div className="status-left">
          <div className="status-item">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
            <span>{connectedHost}</span>
          </div>
        </div>
        <div className="status-right">
          <div className="status-item">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>
            <span>Gemma 4-E4B-it (ローカル)</span>
          </div>
          <div className="status-item">
            <div className={`status-dot ${modelStatus.toLowerCase()}`}></div>
            <span>{modelStatus === "Loaded" ? "Ready" : modelStatus}</span>
          </div>
        </div>
      </footer>
    </div>
  );
}

export default App;
