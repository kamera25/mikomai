import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import "./App.css";

interface Message {
  role: "user" | "ai";
  content: string;
}

interface ChatSession {
  id: string;
  type: 'session';
  title: string;
  messages: Message[];
}

interface Folder {
  id: string;
  type: 'folder';
  name: string;
  items: HistoryItem[];
  isOpen: boolean;
}

type HistoryItem = Folder | ChatSession;

function App() {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isConnectionOpen, setIsConnectionOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [input]);

  const handleSend = async () => {
    if (!input.trim()) return;
    
    const userMessage = input.trim();
    setInput("");
    setMessages(prev => [...prev, { role: "user", content: userMessage }]);

    // Simulated LLM Tool Calling Logic
    setTimeout(async () => {
      const lowerInput = userMessage.toLowerCase();
      
      if (lowerInput.includes("ping")) {
        const match = lowerInput.match(/ping\s+([a-zA-Z0-9.-]+)/);
        const host = match ? match[1] : "1.1.1.1";

        setMessages(prev => [...prev, { role: "ai", content: `Pinging ${host}...` }]);

        try {
          const result: any = await invoke("network_ping", { host });
          setMessages(prev => [...prev, { role: "ai", content: result.success ? `\`\`\`\n${result.output}\n\`\`\`` : `Error: ${result.output}` }]);
        } catch (e: any) {
          setMessages(prev => [...prev, { role: "ai", content: `Failed to execute ping: ${e.toString()}` }]);
        }
      } else if (lowerInput.includes("traceroute") || lowerInput.includes("trace")) {
        const match = lowerInput.match(/trace(?:route)?\s+([a-zA-Z0-9.-]+)/);
        const host = match ? match[1] : "1.1.1.1";

        setMessages(prev => [...prev, { role: "ai", content: `Tracing route to ${host}...` }]);

        try {
          const result: any = await invoke("network_traceroute", { host });
          setMessages(prev => [...prev, { role: "ai", content: result.success ? `\`\`\`\n${result.output}\n\`\`\`` : `Error: ${result.output}` }]);
        } catch (e: any) {
          setMessages(prev => [...prev, { role: "ai", content: `Failed to execute traceroute: ${e.toString()}` }]);
        }
      } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
        setMessages(prev => [...prev, { role: "ai", content: "Retrieving device status..." }]);
        
        try {
          const result: any = await invoke("network_show", {
            device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
            command: "show ip int brief"
          });
          setMessages(prev => [...prev, { role: "ai", content: result.success ? `\`\`\`\n${result.output}\n\`\`\`` : `Error: ${result.output}` }]);
        } catch (e: any) {
          setMessages(prev => [...prev, { role: "ai", content: `Failed to execute: ${e.toString()}` }]);
        }
      } else {
        setMessages(prev => [...prev, { role: "ai", content: "Thinking..." }]);
        try {
          const result: string = await invoke("ask_llm", { prompt: userMessage });
          setMessages(prev => {
            const updated = [...prev];
            updated[updated.length - 1] = { role: "ai", content: result };
            return updated;
          });
        } catch (e: any) {
          setMessages(prev => {
            const updated = [...prev];
            updated[updated.length - 1] = { role: "ai", content: `Error: ${e.toString()}` };
            return updated;
          });
        }
      }
    }, 500);
  };

  const toggleFolder = (folderId: string) => {
    const updateItems = (items: HistoryItem[]): HistoryItem[] => {
      return items.map(item => {
        if (item.id === folderId && item.type === 'folder') {
          return { ...item, isOpen: !item.isOpen };
        }
        if (item.type === 'folder') {
          return { ...item, items: updateItems(item.items) };
        }
        return item;
      });
    };
    setHistory(prev => updateItems(prev));
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

  const renderHistoryItems = (items: HistoryItem[], level = 0) => {
    return items.map(item => {
      if (item.type === 'folder') {
        return (
          <div key={item.id} className="folder-container">
            <div 
              className="folder-item" 
              style={{ paddingLeft: `${12 + level * 12}px` }}
              onClick={() => toggleFolder(item.id)}
            >
              <div className="folder-icon">
                <svg className={`chevron ${item.isOpen ? 'open' : ''}`} width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
              </div>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: 4, color: 'var(--accent-color)' }}><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
              <span className="folder-name">{item.name}</span>
            </div>
            {item.isOpen && renderHistoryItems(item.items, level + 1)}
          </div>
        );
      } else {
        return (
          <div 
            key={item.id} 
            className={`session-item ${activeSessionId === item.id ? 'active' : ''}`}
            style={{ paddingLeft: `${28 + level * 12}px` }}
            onClick={() => setActiveSessionId(item.id)}
          >
            <span className="session-title">{item.title}</span>
          </div>
        );
      }
    });
  };

  return (
    <div className="app-container">
      {/* Activity Bar (LM Studio style thin left bar) */}
      <nav className="activity-bar">
         <div 
          className={`activity-item ${isSidebarOpen && !isSettingsOpen && !isConnectionOpen ? 'active' : ''}`} 
          title="Chat History" 
          onClick={() => {
            if (isSettingsOpen || isConnectionOpen) {
              setIsSettingsOpen(false);
              setIsConnectionOpen(false);
              setIsSidebarOpen(true);
            } else {
              setIsSidebarOpen(!isSidebarOpen);
            }
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
        </div>
        <div 
          className={`activity-item ${isConnectionOpen ? 'active' : ''}`} 
          title="Connection Settings" 
          onClick={() => {
            setIsConnectionOpen(true);
            setIsSettingsOpen(false);
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="16" y="16" width="6" height="6" rx="1"></rect><rect x="2" y="16" width="6" height="6" rx="1"></rect><rect x="9" y="2" width="6" height="6" rx="1"></rect><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"></path><line x1="12" y1="12" x2="12" y2="8"></line></svg>
        </div>
        <div className="spacer"></div>
        <div 
          className={`activity-item ${isSettingsOpen ? 'active' : ''}`} 
          title="Settings" 
          onClick={() => {
            setIsSettingsOpen(true);
            setIsConnectionOpen(false);
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
        </div>
      </nav>

      {/* Sidebar (History) */}
      <aside className={`sidebar ${isSidebarOpen ? '' : 'collapsed'}`}>
        <div className="sidebar-header">
          <h2>履歴</h2>
          <div className="header-actions">
            <button className="icon-button" title="新規フォルダ" onClick={() => {
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
            }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
            </button>
            <button className="icon-button" title="新規チャット" onClick={() => {
              const id = `session-${Date.now()}`;
              setHistory(prev => [{
                id,
                type: 'session',
                title: "新しいセッション",
                messages: []
              }, ...prev]);
              setActiveSessionId(id);
              setMessages([]);
            }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
            </button>
          </div>
        </div>
        
        <div className="session-list">
          {renderHistoryItems(history)}
        </div>
      </aside>

      {/* Main Viewport (Grouping Chat and Settings) */}
      <div className="main-viewport">
        {isSettingsOpen ? (
          <SettingsPanel 
            isOpen={isSettingsOpen} 
            onClose={() => setIsSettingsOpen(false)} 
          />
        ) : isConnectionOpen ? (
          <ConnectionSettingsPanel 
            onClose={() => setIsConnectionOpen(false)} 
          />
        ) : (
          <main className="main-chat">
            {/* Top Header */}
            <header className="chat-header">
              <div className="header-left">
    
                <div className="model-selector">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>
                  <span>Qwen 2.5 (ローカル)</span>
                </div>
              </div>
            </header>
    
            {/* Chat History */}
            <div className="chat-history">
              {messages.length === 0 ? (
                <div className="empty-state">
                  <div className="agent-icon" style={{ width: 64, height: 64, marginBottom: 24, borderRadius: 16 }}>
                     <svg width="32" height="32" viewBox="0 0 210 210" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path
                        fill="white"
                        d="m 102.84116,141.04786 c -1.49823,-0.27197 -2.864913,-1.58197 -3.084757,-2.95682 -0.123117,-0.7699 -0.146006,-0.79181 -1.358836,-1.30025 -1.815611,-0.76115 -4.326392,-2.40325 -6.334966,-4.14319 l -0.539984,-0.46777 h 1.291862 c 1.271963,0 1.306701,0.0104 2.255593,0.67678 1.444272,1.01423 2.79704,1.79326 3.873399,2.23063 0.918359,0.37316 0.970958,0.37819 1.116669,0.10679 0.38803,-0.72274 1.52509,-1.60607 2.39167,-1.858 1.82156,-0.52954 3.45087,0.0776 4.64029,1.72915 0.22174,0.30789 0.23785,0.30542 1.24727,-0.19184 2.81516,-1.38677 5.52467,-3.41166 7.73707,-5.78215 1.91147,-2.04805 3.23004,-3.93017 4.43388,-6.3289 l 1.00836,-2.00922 -0.83768,-0.79814 c -1.4511,-1.38259 -1.69392,-3.22126 -0.65436,-4.95472 1.33528,-2.22656 4.36816,-2.47677 6.18113,-0.50992 l 0.62583,0.67894 1.28832,-0.54958 c 1.96143,-0.83672 4.07286,-2.00341 5.92826,-3.27569 l 1.67662,-1.1497 -0.002,0.58215 c -0.004,1.45526 -0.12267,1.68944 -1.22613,2.42574 -1.61775,1.07945 -3.85322,2.29279 -5.55662,3.01595 -1.05838,0.44932 -1.57533,0.75226 -1.60845,0.9426 -0.40217,2.31119 -1.52983,3.51483 -3.61636,3.86005 -0.51381,0.085 -0.5854,0.17568 -1.21865,1.5435 -1.43122,3.09141 -3.67622,6.2278 -6.23528,8.711 -1.31572,1.27672 -3.53387,3.07471 -4.37154,3.5435 -0.82032,0.45907 0.75073,0.17424 3.47721,-0.63043 4.01161,-1.18395 9.76687,-4.2469 13.33961,-7.09937 l 1.17022,-0.9343 -0.0541,-0.80224 c -0.0802,-1.18906 0.32028,-2.22159 1.16286,-2.99827 0.95713,-0.88225 1.62634,-1.13674 2.97608,-1.13179 l 1.12248,0.004 0.48773,-1.34677 c 1.39624,-3.85536 1.92315,-6.77568 1.93143,-10.70465 0.006,-3.0062 -0.11701,-4.00852 -0.86367,-7.0184 -1.33194,-5.36923 -3.36258,-9.6608 -6.53107,-13.80287 -0.89137,-1.16527 -0.89515,-1.17479 -0.98997,-2.49543 l -0.0952,-1.3257 0.79311,0.8578 c 2.01799,2.18261 3.78966,4.82091 5.35393,7.97289 2.79201,5.62583 4.03702,10.6691 4.03812,16.35759 5.9e-4,3.68899 -0.39951,5.92125 -1.7757,9.90375 -0.87895,2.54358 -0.86918,2.46756 -0.39661,3.08712 1.28385,1.68321 1.07733,4.04756 -0.47239,5.40824 -0.82154,0.72132 -1.54155,0.98218 -2.71092,0.98218 -1.16314,0 -1.88939,-0.26087 -2.69468,-0.96793 l -0.56648,-0.49737 -0.99479,0.84391 c -1.30365,1.10592 -4.63962,3.31198 -6.50777,4.30354 -4.10284,2.17769 -9.44762,3.89504 -13.78781,4.43021 l -1.54677,0.19072 -0.29178,0.87536 c -0.68382,2.05147 -2.51848,3.14941 -4.62399,2.7672 z" />
                    </svg>
                </div>
                <h3>mikomai</h3>
                <p>ローカルのベクトルデータベースとMCPサーバーに接続しています。マニュアルの取得、スイッチの状態確認、構成変更の提案などをお申し付けください。</p>
              </div>
            ) : (
              messages.map((msg, idx) => (
                <div key={idx} className={`message ${msg.role}`}>
                  <div className={`avatar ${msg.role}`}>
                    {msg.role === 'ai' ? '🤖' : '👤'}
                  </div>
                  <div className="message-bubble">
                    {msg.content.includes("```") ? (
                      <pre style={{ whiteSpace: 'pre-wrap' }}>{msg.content.replace(/```/g, '')}</pre>
                    ) : (
                      msg.content
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
  
          {/* Input Area */}
          <div className="input-area">
            <div className="input-container">
              <textarea
                ref={textareaRef}
                className="chat-input"
                placeholder="mikomaiに質問する..."
                value={input}
                onChange={(e) => setInput(e.target.value)}
                rows={1}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                  }
                }}
              />
              <button className="send-button" onClick={handleSend}>
                <svg viewBox="0 0 24 24">
                  <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"></path>
                </svg>
              </button>
            </div>
          </div>
        </main>
        )}
      </div>
    </div>
  );
}

export default App;
