import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import { SettingsPanel } from "./components/SettingsPanel";
import { ConnectionSettingsPanel } from "./components/ConnectionSettingsPanel";
import { ScheduledTasksPanel } from "./components/ScheduledTasksPanel";
import "./App.css";

interface Message {
  role: "user" | "ai";
  content: string;
  timestamp?: string; // ISO string
}

interface SummaryItem {
  timestamp: string;
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
  const [isScheduledTasksOpen, setIsScheduledTasksOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [modelStatus, setModelStatus] = useState<string>("NotLoaded");
  const [connectedHost] = useState<string>("192.168.1.1 (Core-Switch-01)");
  const [summaries, setSummaries] = useState<SummaryItem[]>([]);
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

  const summarizeAndSave = async (content: string) => {
    try {
      const summaryPrompt = `以下の内容（実行結果やアシスタントの回答）を40文字程度で簡潔に要約してください。\n\n${content}`;
      const summaryText: string = await invoke("ask_llm_background", { prompt: summaryPrompt });
      const newSummary = { timestamp: new Date().toISOString(), content: summaryText };
      await invoke("save_summary", { summary: newSummary });
      setSummaries(prev => {
        const next = [...prev, newSummary];
        return next.length > 5 ? next.slice(next.length - 5) : next;
      });
    } catch (e) {
      console.error("Failed to generate/save summary:", e);
    }
  };

  const handleSend = async () => {
    if (!input.trim()) return;
    
    const userMessage = input.trim();
    const timestamp = new Date().toISOString();
    setInput("");
    setMessages(prev => [...prev, { role: "user", content: userMessage, timestamp }]);

    // Improved Tool Calling Logic
    setTimeout(async () => {
      const lowerInput = userMessage.toLowerCase();
      
      // Flexible regex for ping (supports Japanese and varied order)
      let pingArgs: any = null;
      const pingBaseMatch = lowerInput.match(/(?:ping|ピン|ピング)\s+([a-zA-Z0-9.-]+)/) || 
                            lowerInput.match(/([a-zA-Z0-9.-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)/);
      
      if (pingBaseMatch) {
        const host = pingBaseMatch[1];
        pingArgs = { host };
        
        // Try to find size
        const sizeMatch = lowerInput.match(/(?:size|サイズ)\s*(\d+)/);
        if (sizeMatch) pingArgs.size = parseInt(sizeMatch[1]);
        
        // Try to find count
        const countMatch = lowerInput.match(/(?:count|回数|回|回実行)\s*(\d+)/);
        if (countMatch) pingArgs.count = parseInt(countMatch[1]);

        // Try to find df
        if (lowerInput.includes("df") || lowerInput.includes("フラグメント禁止") || lowerInput.includes("断片化禁止")) {
          pingArgs.df = true;
        }
      }
      
      // Flexible regex for traceroute (supports Japanese and varied order)
      const traceMatch = lowerInput.match(/(?:trace(?:route)?|トレース|トレースルート)\s+([a-zA-Z0-9.-]+)/) ||
                         lowerInput.match(/([a-zA-Z0-9.-]+)\s*(?:に|へ)?\s*(?:trace(?:route)?|トレース|トレースルート)/);

      // Flexible regex for host list
      const hostListMatch = lowerInput.match(/(?:host|ホスト|接続先|ターゲット).*(?:list|一覧|教え|見せ|確認)/) || 
                            lowerInput.match(/(?:list|一覧|教え|見せ|確認).*(?:host|ホスト|接続先|ターゲット)/);

      const executeAndAnalyze = async (toolId: string, toolLabel: string, args: any, depth: number = 0, executedTools: Set<string> = new Set()) => {
        const toolSignature = `${toolId}:${JSON.stringify(args)}`;
        if (executedTools.has(toolSignature)) {
          setMessages(prev => [...prev, { role: "ai", content: "以上、報告いたします。", timestamp: new Date().toISOString() }]);
          return;
        }
        executedTools.add(toolSignature);

        if (depth > 3) {
          setMessages(prev => [...prev, { role: "ai", content: "エラー: ツール呼び出しのループが深すぎるため中断しました。", timestamp: new Date().toISOString() }]);
          return;
        }

        setMessages(prev => [...prev, { role: "ai", content: `⏱️ ${toolLabel} を実行中...`, timestamp: new Date().toISOString() }]);
        try {
          const result: any = await invoke(toolId, args);
          const resultMessage = result.success ? 
            `### ${toolLabel} 実行結果\n\`\`\`terminal\n${result.output}\n\`\`\`` :
            `⚠️ **${toolLabel}の実行に失敗しました。**\n\n【エラー内容】\n\`\`\`terminal\n${result.output}\n\`\`\``;
          
          setMessages(prev => {
            const updated = [...prev];
            updated[updated.length - 1] = { role: "ai", content: resultMessage, timestamp: new Date().toISOString() };
            return updated;
          });

          const recentSummariesText = summaries.slice(-5).map((s, i) => `${i+1}. ${s.content}`).join("\n");
          const contextPrefix = recentSummariesText ? `【過去の実行履歴要約（直近5件）】\n${recentSummariesText}\n※最新の情報（番号が大きいもの）を優先するようにし、最新の情報で解決できない場合は、その前の情報を参照…を繰り返すようにしてください。\n\n` : "";

          const analysisPrompt = `${contextPrefix}ユーザーの入力: "${userMessage}"\nに対する${toolLabel}の実行結果は以下の通りです:\n\n${result.output}\n\nこの結果を分析し、ネットワークエンジニアの視点で状況を日本語で簡潔に報告してください。\n\n【重要】既にツールは実行済みです。この回答内で再度同じコマンド、かつ同じ引数でツール呼び出し（JSONフォーマット）を出力することは絶対に避けてください。結果の解説と、次にユーザーが実行すべきアクションの提案のみを行ってください。`;
          
          setMessages(prev => [...prev, { role: "ai", content: "", timestamp: new Date().toISOString() }]);
          
          let analysisContent = "";
          const analysisUnlisten = await listen<string>("llm-chunk", (event) => {
            analysisContent += event.payload;
            setMessages(prev => {
              const updated = [...prev];
              const lastMessage = updated[updated.length - 1];
              if (lastMessage && lastMessage.role === "ai") {
                updated[updated.length - 1] = { ...lastMessage, content: analysisContent };
              }
              return updated;
            });
          });

          let responseStr = "";
          try {
            responseStr = await invoke("ask_llm", { prompt: analysisPrompt });
          } catch (analysisError: any) {
            console.error("Failed to get analysis", analysisError);
          } finally {
            analysisUnlisten();
          }

          // Check if the AI outputted another tool call JSON
          let nextJsonStr = "";
          const nextJsonBlockMatch = responseStr.match(/```(?:json)?\s*(\{[\s\S]*?"tool"[\s\S]*?\})\s*```/);
          if (nextJsonBlockMatch) {
            nextJsonStr = nextJsonBlockMatch[1];
          } else {
            const nextFallbackMatch = responseStr.match(/\{[\s\S]*?"tool"[\s\S]*\}/);
            if (nextFallbackMatch) {
              nextJsonStr = nextFallbackMatch[0];
            }
          }

          if (nextJsonStr) {
            try {
              console.log("Extracted subsequent JSON tool string:", nextJsonStr);
              const nextToolCall = JSON.parse(nextJsonStr);
              const nextToolActionName = nextToolCall.tool === "network_ping" ? "Ping" : 
                                         nextToolCall.tool === "network_traceroute" ? "Traceroute" : 
                                         nextToolCall.tool === "network_get_hosts" ? "Host List" :
                                         nextToolCall.tool === "network_show" ? "Show Command" : nextToolCall.tool;
              
              // Add a small delay for better UX before chaining
              setTimeout(async () => {
                await executeAndAnalyze(nextToolCall.tool, nextToolActionName, nextToolCall.args, depth + 1, executedTools);
              }, 1000);
            } catch (e) {
              console.error("Failed to parse subsequent tool call JSON", e);
              summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`);
            }
          } else {
             summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`);
          }

        } catch (e: any) {
          const errorMsg = e.toString();
          const displayError = errorMsg.includes("Failed to execute") 
            ? `❌ **${toolLabel}の実行に失敗しました。**\n\n実行環境（サイドカーやネットワーク接続）に問題がある可能性があります。\n\n詳細: \`${errorMsg}\``
            : `❌ **${toolLabel}の実行中にエラーが発生しました。**\n\n詳細: \`${errorMsg}\``;

          setMessages(prev => {
            const updated = [...prev];
            updated[updated.length - 1] = { role: "ai", content: displayError, timestamp: new Date().toISOString() };
            return updated;
          });
        }
      };

      if (pingArgs) {
        await executeAndAnalyze("network_ping", "Ping", pingArgs);
      } else if (traceMatch) {
        const host = traceMatch[1] || traceMatch[2];
        await executeAndAnalyze("network_traceroute", "Traceroute", { host });
      } else if (hostListMatch) {
        await executeAndAnalyze("network_get_hosts", "Host List", {});
      } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
        await executeAndAnalyze("network_show", "Show Command", {
          device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
          command: "show ip int brief"
        });
      } else {
        setMessages(prev => [...prev, { role: "ai", content: "", timestamp: new Date().toISOString() }]);
        
        let fullContent = "";
        let unlisten: () => void = () => {};
        
        try {
          unlisten = await listen<string>("llm-chunk", (event) => {
            fullContent += event.payload;
            setMessages(prev => {
              const updated = [...prev];
              const lastMessage = updated[updated.length - 1];
              if (lastMessage && lastMessage.role === "ai") {
                updated[updated.length - 1] = { ...lastMessage, content: fullContent };
              }
              return updated;
            });
          });

          const recentSummariesText = summaries.slice(-5).map((s, i) => `${i+1}. ${s.content}`).join("\n");
          const contextPrefix = recentSummariesText ? `【過去の実行履歴要約（直近5件）】\n${recentSummariesText}\n※最新の情報（番号が大きいもの）を優先するようにし、最新の情報で解決できない場合は、その前の情報を参照…を繰り返すようにしてください。\n\n` : "";
          const promptWithContext = `${contextPrefix}【ユーザー入力】\n${userMessage}`;

          const response: string = await invoke("ask_llm", { prompt: promptWithContext });
          unlisten(); // Unlisten IMMEDIATELY after the first call completes
          
          console.log("LLM Response:", response);
          summarizeAndSave(`ユーザー入力: ${userMessage}\n回答: ${response}`);
          
          // Better extraction: Try to find JSON block or match balanced-like structure
          let jsonStr = "";
          const jsonBlockMatch = response.match(/```(?:json)?\s*(\{[\s\S]*?"tool"[\s\S]*?\})\s*```/);
          if (jsonBlockMatch) {
            jsonStr = jsonBlockMatch[1];
          } else {
            // Fallback: capture from { "tool" to the last }
            const fallbackMatch = response.match(/\{[\s\S]*?"tool"[\s\S]*\}/);
            if (fallbackMatch) {
              jsonStr = fallbackMatch[0];
            }
          }

          if (jsonStr) {
            try {
              console.log("Extracted JSON tool string:", jsonStr);
              const toolCall = JSON.parse(jsonStr);
              const toolActionName = toolCall.tool === "network_ping" ? "Ping" : 
                                     toolCall.tool === "network_traceroute" ? "Traceroute" : 
                                     toolCall.tool === "network_get_hosts" ? "Host List" :
                                     toolCall.tool === "network_show" ? "Show Command" : toolCall.tool;
              
              await executeAndAnalyze(toolCall.tool, toolActionName, toolCall.args);
            } catch (parseError) {
              console.error("Failed to parse tool call JSON", parseError);
            }
          }
        } catch (e: any) {
          setMessages(prev => {
            const updated = [...prev];
            updated[updated.length - 1] = { role: "ai", content: `Error: ${e.toString()}`, timestamp: new Date().toISOString() };
            return updated;
          });
        } finally {
          unlisten();
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
      <div className="main-layout">
        {/* Activity Bar (LM Studio style thin left bar) */}
        <nav className="activity-bar">
         <div 
          className={`activity-item ${isSidebarOpen && !isSettingsOpen && !isConnectionOpen && !isScheduledTasksOpen ? 'active' : ''}`} 
          title="Chat History" 
          onClick={() => {
            if (isSettingsOpen || isConnectionOpen || isScheduledTasksOpen) {
              setIsSettingsOpen(false);
              setIsConnectionOpen(false);
              setIsScheduledTasksOpen(false);
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
            setIsScheduledTasksOpen(false);
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="16" y="16" width="6" height="6" rx="1"></rect><rect x="2" y="16" width="6" height="6" rx="1"></rect><rect x="9" y="2" width="6" height="6" rx="1"></rect><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"></path><line x1="12" y1="12" x2="12" y2="8"></line></svg>
        </div>
        <div 
          className={`activity-item ${isScheduledTasksOpen ? 'active' : ''}`} 
          title="Scheduled Tasks" 
          onClick={() => {
            setIsScheduledTasksOpen(true);
            setIsConnectionOpen(false);
            setIsSettingsOpen(false);
          }}
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>
        </div>
        <div className="spacer"></div>
        <div 
          className={`activity-item ${isSettingsOpen ? 'active' : ''}`} 
          title="Settings" 
          onClick={() => {
            setIsSettingsOpen(true);
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(false);
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
        ) : isScheduledTasksOpen ? (
          <ScheduledTasksPanel 
            onClose={() => setIsScheduledTasksOpen(false)} 
          />
        ) : (
          <main className="main-chat">
            {/* Top Header */}
            <header className="chat-header">
              <div className="header-left">
                <h1 className="header-title">mikomai</h1>
              </div>
            </header>
    
            {/* Chat History */}
            <div className="chat-history">
              {messages.length === 0 ? (
                <div className="empty-state">
                  <div className="agent-icon" style={{ width: 64, height: 64, marginBottom: 24, borderRadius: 16 }}>
                     <svg width="32" height="32" viewBox="0 0 210 210" fill="none" stroke="var(--accent-color)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path
                        fill="var(--accent-color)"
                        d="m 102.84116,141.04786 c -1.49823,-0.27197 -2.864913,-1.58197 -3.084757,-2.95682 -0.123117,-0.7699 -0.146006,-0.79181 -1.358836,-1.30025 -1.815611,-0.76115 -4.326392,-2.40325 -6.334966,-4.14319 l -0.539984,-0.46777 h 1.291862 c 1.271963,0 1.306701,0.0104 2.255593,0.67678 1.444272,1.01423 2.79704,1.79326 3.873399,2.23063 0.918359,0.37316 0.970958,0.37819 1.116669,0.10679 0.38803,-0.72274 1.52509,-1.60607 2.39167,-1.858 1.82156,-0.52954 3.45087,0.0776 4.64029,1.72915 0.22174,0.30789 0.23785,0.30542 1.24727,-0.19184 2.81516,-1.38677 5.52467,-3.41166 7.73707,-5.78215 1.91147,-2.04805 3.23004,-3.93017 4.43388,-6.3289 l 1.00836,-2.00922 -0.83768,-0.79814 c -1.4511,-1.38259 -1.69392,-3.22126 -0.65436,-4.95472 1.33528,-2.22656 4.36816,-2.47677 6.18113,-0.50992 l 0.62583,0.67894 1.28832,-0.54958 c 1.96143,-0.83672 4.07286,-2.00341 5.92826,-3.27569 l 1.67662,-1.1497 -0.002,0.58215 c -0.004,1.45526 -0.12267,1.68944 -1.22613,2.42574 -1.61775,1.07945 -3.85322,2.29279 -5.55662,3.01595 -1.05838,0.44932 -1.57533,0.75226 -1.60845,0.9426 -0.40217,2.31119 -1.52983,3.51483 -3.61636,3.86005 -0.51381,0.085 -0.5854,0.17568 -1.21865,1.5435 -1.43122,3.09141 -3.67622,6.2278 -6.23528,8.711 -1.31572,1.27672 -3.53387,3.07471 -4.37154,3.5435 -0.82032,0.45907 0.75073,0.17424 3.47721,-0.63043 4.01161,-1.18395 9.76687,-4.2469 13.33961,-7.09937 l 1.17022,-0.9343 -0.0541,-0.80224 c -0.0802,-1.18906 0.32028,-2.22159 1.16286,-2.99827 0.95713,-0.88225 1.62634,-1.13674 2.97608,-1.13179 l 1.12248,0.004 0.48773,-1.34677 c 1.39624,-3.85536 1.92315,-6.77568 1.93143,-10.70465 0.006,-3.0062 -0.11701,-4.00852 -0.86367,-7.0184 -1.33194,-5.36923 -3.36258,-9.6608 -6.53107,-13.80287 -0.89137,-1.16527 -0.89515,-1.17479 -0.98997,-2.49543 l -0.0952,-1.3257 0.79311,0.8578 c 2.01799,2.18261 3.78966,4.82091 5.35393,7.97289 2.79201,5.62583 4.03702,10.6691 4.03812,16.35759 5.9e-4,3.68899 -0.39951,5.92125 -1.7757,9.90375 -0.87895,2.54358 -0.86918,2.46756 -0.39661,3.08712 1.28385,1.68321 1.07733,4.04756 -0.47239,5.40824 -0.82154,0.72132 -1.54155,0.98218 -2.71092,0.98218 -1.16314,0 -1.88939,-0.26087 -2.69468,-0.96793 l -0.56648,-0.49737 -0.99479,0.84391 c -1.30365,1.10592 -4.63962,3.31198 -6.50777,4.30354 -4.10284,2.17769 -9.44762,3.89504 -13.78781,4.43021 l -1.54677,0.19072 -0.29178,0.87536 c -0.68382,2.05147 -2.51848,3.14941 -4.62399,2.7672 z" />
                    </svg>
                </div>
                <h3>mikomai</h3>
                <p>ローカルのベクトルデータベースとMCPサーバーに接続しています。マニュアルの取得、スイッチの状態確認、構成変更の提案などをお申し付けください。</p>
              </div>
            ) : (
              messages.map((msg, idx) => (
                <div key={idx} className={`message-container ${msg.role}`}>
                  {msg.role === 'user' && (
                    <div className="message-header">
                      <div className="header-line"></div>
                      <span className="message-time">{formatMessageTime(msg.timestamp)}</span>
                    </div>
                  )}
                  <div className={`message ${msg.role}`}>
                    <div className="message-bubble markdown-body">
                      {msg.content.split(/(```[\s\S]*?```)/).map((part, i) => {
                        if (part.startsWith("```")) {
                          const isTerminal = part.startsWith("```terminal");
                          const content = part.replace(/```(\w+)?\n?/, "").replace(/```$/, "");

                          if (isTerminal) {
                            return (
                              <div key={i} className="terminal-container">
                                <div className="terminal-header">
                                  <div className="terminal-dots">
                                    <div className="terminal-dot red"></div>
                                    <div className="terminal-dot yellow"></div>
                                    <div className="terminal-dot green"></div>
                                  </div>
                                </div>
                                <pre className="terminal-content"><code>{content}</code></pre>
                              </div>
                            );
                          }

                          return <pre key={i} className="code-block"><code>{content}</code></pre>;
                        }
                        return <ReactMarkdown key={i}>{part}</ReactMarkdown>;
                      })}
                    </div>
                  </div>
                </div>
              ))
            )}
            <div ref={messagesEndRef} />
          </div>
  
          {/* Input Area */}
          <div className="input-area">
            {modelStatus !== "Loaded" && (
              <div className={`model-status-banner ${modelStatus.toLowerCase()}`}>
                <div className="status-spinner"></div>
                <span>
                  {modelStatus === "NotLoaded" && "AIモデルが読み込まれていません。設定からモデルを読み込んでください。"}
                  {modelStatus === "Loading" && "AIモデルを読み込み中です。しばらくお待ちください..."}
                  {modelStatus === "Error" && "AIモデルの読み込みに失敗しました。設定を確認してください。"}
                </span>
                {(modelStatus === "NotLoaded" || modelStatus === "Error") && (
                  <button className="banner-button" onClick={() => setIsSettingsOpen(true)}>
                    設定
                  </button>
                )}
              </div>
            )}
            <div className={`input-container ${modelStatus !== "Loaded" ? 'disabled' : ''}`}>
              <textarea
                ref={textareaRef}
                className="chat-input"
                placeholder={modelStatus === "Loaded" ? "mikomaiに質問する..." : "モデルの準備を待っています..."}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                rows={1}
                disabled={modelStatus !== "Loaded"}
                onCompositionStart={() => { isComposing.current = true; }}
                onCompositionEnd={() => { 
                  setTimeout(() => { isComposing.current = false; }, 100); 
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !isComposing.current && !e.shiftKey && modelStatus === "Loaded") {
                    e.preventDefault();
                    handleSend();
                  }
                }}
              />
                <button 
                  className="send-button" 
                  onClick={handleSend}
                  disabled={modelStatus !== "Loaded" || !input.trim()}
                >
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
            <span>Gemma 4-E2B-it (ローカル)</span>
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
