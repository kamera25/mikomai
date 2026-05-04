import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

  const summarizeAndSave = async (content: string) => {
    try {
      const summaryPrompt = `以下の内容を要約してください。\n\n${content}`;
      const summaryText: string = await invoke("ask_llm_background", { prompt: summaryPrompt });
      const newSummary = { timestamp: new Date().toISOString(), content: summaryText };
      await invoke("save_summary", { summary: newSummary });
      setSummaries(prev => {
        const next = [...prev, newSummary];
        // Keep a reasonable number of summaries in state (e.g. 20), 
        // slicing for prompt will happen later based on historyLimit
        return next.length > 20 ? next.slice(next.length - 20) : next;
      });
    } catch (e) {
      console.error("Failed to generate/save summary:", e);
    }
  };

  const getHistoryBlock = (items: SummaryItem[], limit: number) => {
    if (limit <= 0 || items.length === 0) return "";
    const recent = [...items].reverse().slice(0, limit);
    const text = recent.map((s, i) => `${i + 1}. ${s.content}`).join("\n");
    return `\n\n【過去の実行履歴要約】\n${text}`;
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

    // Improved Tool Calling Logic
    setTimeout(async () => {
      const lowerInput = userMessage.toLowerCase();
      
      // Flexible regex for ping (supports Japanese and varied order)
      let pingArgs: any = null;
      const pingBaseMatch = lowerInput.match(/(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)/) || 
                            lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)/);
      
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
      const traceMatch = lowerInput.match(/(?:trace(?:route)?|トレース|トレースルート)\s+([a-zA-Z0-9.:-]+)/) ||
                         lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:trace(?:route)?|トレース|トレースルート)/);

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

        const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
        const statusMsg = isRag ? `NW-DBを検索中...` : `${toolLabel} を実行中...`;
        
        setMessages(prev => [...prev, { role: "ai", content: statusMsg, timestamp: new Date().toISOString(), isToolLoading: true }]);
        try {
          const result: any = await invoke(toolId, args);
          const statusBadge = result.success ? "✅ 成功" : "❌ 失敗";
          const resultMessage = result.success ? 
            `### ${toolLabel} 実行結果: ${statusBadge}\n\`\`\`terminal\n${result.output}\n\`\`\`` :
            `⚠️ **${toolLabel}の実行に失敗しました: ${statusBadge}**\n\n【エラー内容】\n\`\`\`terminal\n${result.output}\n\`\`\``;
          
          if (isRag) {
            // For RAG, we don't show the terminal UI, just update the status to indicate analysis is starting
            setMessages(prev => {
              const updated = [...prev];
              updated[updated.length - 1] = { 
                role: "ai", 
                content: result.success ? `技術文書を確認しました。内容を整理して回答します...` : `⚠️ NW-DBの検索に失敗しました。`, 
                timestamp: new Date().toISOString(),
                isToolLoading: result.success
              };
              return updated;
            });
          } else {
            setMessages(prev => {
              const updated = [...prev];
              updated[updated.length - 1] = { role: "ai", content: resultMessage, timestamp: new Date().toISOString() };
              return updated;
            });
          }

          const historyBlock = getHistoryBlock(summaries, historyLimit);
          const analysisPrompt = isRag ? 
            `ユーザーの質問: "${userMessage}"\nに対して、技術文書データベース(NW-DB)から以下の情報を取得しました:\n\n${result.output}\n\nこの内容に基づき、ネットワークエンジニアの視点で、ユーザーの質問に対する的確な回答を日本語で生成してください。回答には、参照した資料の内容を具体的に含めてください。${historyBlock}` :
            `ユーザーの入力: "${userMessage}"\nに対する${toolLabel}の実行結果（ステータス: ${statusBadge}）は以下の通りです:\n\n${result.output}\n\nこの結果を分析し、ネットワークエンジニアの視点で状況を日本語で簡潔に報告してください。\n\n # 重要! \n\n既にツールは実行済みです。この回答内で再度同じコマンド、かつ同じ引数でツール呼び出し（JSONフォーマット）を出力することは絶対に避けてください。結果の解説と、次にユーザーが実行すべきアクションの提案のみを行ってください。${historyBlock}`;
          
          setMessages(prev => [...prev, { role: "ai", content: "分析中...", timestamp: new Date().toISOString(), isToolLoading: true }]);
          
          let analysisContent = "";
          const analysisUnlisten = await listen<string>("llm-chunk", (event) => {
            analysisContent += event.payload;
            setMessages(prev => {
              const updated = [...prev];
              const lastMessage = updated[updated.length - 1];
              if (lastMessage && lastMessage.role === "ai") {
                updated[updated.length - 1] = { ...lastMessage, content: analysisContent, isToolLoading: false };
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
                                         nextToolCall.tool === "network_query_nw_db" || nextToolCall.tool === "query_nw_db" ? "NW-DB Search" :
                                         nextToolCall.tool === "network_arp" ? "ARP Table" :
                                         nextToolCall.tool === "network_get_ip_info" ? "IP Info" :
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
      } else if (lowerInput.includes("arp")) {
        await executeAndAnalyze("network_arp", "ARP Table", {});
      } else if (lowerInput.includes("ip") || lowerInput.includes("ネットワーク情報") || lowerInput.includes("アドレス")) {
        await executeAndAnalyze("network_get_ip_info", "IP Info", {});
      } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
        await executeAndAnalyze("network_show", "Show Command", {
          device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
          command: "show ip int brief"
        });
      } else {
        setMessages(prev => [...prev, { role: "ai", content: "思考中...", timestamp: new Date().toISOString(), isToolLoading: true }]);
        
        let fullContent = "";
        let unlisten: () => void = () => {};
        
        try {
          unlisten = await listen<string>("llm-chunk", (event) => {
            fullContent += event.payload;
            setMessages(prev => {
              const updated = [...prev];
              const lastMessage = updated[updated.length - 1];
              if (lastMessage && lastMessage.role === "ai") {
                updated[updated.length - 1] = { ...lastMessage, content: fullContent, isToolLoading: false };
              }
              return updated;
            });
          });

          const historyBlock = getHistoryBlock(summaries, historyLimit);
          const promptWithContext = `【ユーザー入力】\n${userMessage}${historyBlock}`;

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
                                     toolCall.tool === "network_query_nw_db" || toolCall.tool === "query_nw_db" ? "NW-DB Search" :
                                     toolCall.tool === "network_arp" ? "ARP Table" :
                                     toolCall.tool === "network_get_ip_info" ? "IP Info" :
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
