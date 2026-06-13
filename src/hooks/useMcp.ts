import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UseMcpProps } from "./useMcp/types";
import { useMcpListeners } from "./useMcp/useMcpListeners";
import { useMcpExecutor } from "./useMcp/useMcpExecutor";
import { getHistoryBlock } from "./useMcp/helpers";
import { parsePingCommand } from "../utils/commandParser";

export function useMcp({ 
  messages,
  setMessages, 
  summaries, 
  setSummaries, 
  historyLimit,
  mcpTimeout = 30
}: UseMcpProps) {

  // Setup Tauri event listeners using sub-hook
  useMcpListeners({ setMessages });

  // Setup execution and analysis functions using sub-hook
  const { executeAndAnalyze, summarizeAndSave } = useMcpExecutor({
    messages,
    setMessages,
    summaries,
    setSummaries,
    historyLimit,
    mcpTimeout
  });

  const handleMcpResponse = async (userMessage: string) => {
    const lowerInput = userMessage.toLowerCase();
    
    const pingArgs = parsePingCommand(userMessage);
    
    const traceMatch = lowerInput.match(/(?:trace(?:route)?|トレース|トレースルート)\s+([a-zA-Z0-9.:-]+)/) ||
                       lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:trace(?:route)?|トレース|トレースルート)/);

    const hostListMatch = lowerInput.match(/(?:host|ホスト|接続先|ターゲット).*(?:list|一覧|教え|見せ|確認)/) || 
                          lowerInput.match(/(?:list|一覧|教え|見せ|確認).*(?:host|ホスト|接続先|ターゲット)/);

    if (pingArgs) {
      await executeAndAnalyze(userMessage, "self_network_ping", "Ping", pingArgs);
    } else if (traceMatch) {
      const host = traceMatch[1] || traceMatch[2];
      await executeAndAnalyze(userMessage, "self_network_traceroute", "Traceroute", { host });
    } else if (hostListMatch) {
      await executeAndAnalyze(userMessage, "network_get_hosts", "Host List", {});
    } else if (lowerInput.includes("arp") && (lowerInput.includes("ローカル") || lowerInput.includes("自機") || lowerInput.includes("このpc") || lowerInput.includes("local"))) {
      await executeAndAnalyze(userMessage, "self_network_arp", "ARP Table", {});
    } else if (lowerInput.includes("route") && (lowerInput.includes("ローカル") || lowerInput.includes("自機") || lowerInput.includes("このpc") || lowerInput.includes("local") || lowerInput.includes("ルーティング"))) {
      await executeAndAnalyze(userMessage, "self_network_route", "Route Table", {});
    } else if (lowerInput.includes("ip") || lowerInput.includes("ネットワーク情報") || lowerInput.includes("アドレス")) {
      await executeAndAnalyze(userMessage, "network_get_ip_info", "IP Info", {});
    } else if (lowerInput.includes("console") || lowerInput.includes("コンソール") || lowerInput.includes("シリアル")) {
      if (lowerInput.includes("list") || lowerInput.includes("一覧") || lowerInput.includes("ポート") || lowerInput.includes("リスト")) {
        await executeAndAnalyze(userMessage, "network_list_serial_ports", "Serial Ports", {});
      } else {
        // Fallback to LLM for parsing port and message if not clearly a list request
        setMessages(prev => [...prev, { 
          role: "ai", 
          content: "考え中...", 
          timestamp: new Date().toISOString(), 
          isToolLoading: true,
          isHidden: true,
          event_type: "AgentResponse"
        }]);
      }
    } else if (
      (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) &&
      !lowerInput.includes("config") &&
      !lowerInput.includes("設定") &&
      !lowerInput.includes("構成")
    ) {
      await executeAndAnalyze(userMessage, "network_show", "Show Command", {
        device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
        command: "show ip int brief"
      });
    } else {
      const thinkingTaskId = `task_think_${Date.now()}`;
      // Intermediate thinking message (hidden)
      setMessages(prev => [...prev, { 
        role: "ai", 
        content: "考え中...", 
        timestamp: new Date().toISOString(), 
        isToolLoading: true,
        isHidden: true,
        task_id: thinkingTaskId,
        event_type: "AgentResponse"
      }]);
      
      let fullContent = "";
      let unlisten: () => void = () => {};
      let agentUnlisten = () => {};
      let routeUnlisten = () => {};
      
      try {
        unlisten = await listen<string>("llm-chunk", (event) => {
          fullContent += event.payload;
          setMessages(prev => prev.map(msg => 
            msg.task_id === thinkingTaskId ? { ...msg, content: fullContent, isToolLoading: false, isHidden: false } : msg
          ));
        });

        try {
          agentUnlisten = await listen<string>("agent-selected", (event) => {
            const agentName = event.payload;
            setMessages(prev => prev.map(msg =>
              msg.task_id === thinkingTaskId ? { ...msg, summary_text: `${agentName} が処理中...`, isHidden: false } : msg
            ));
          });
          routeUnlisten = await listen<string>("route-yaml-saved", () => {
            setMessages(prev => prev.map(msg =>
              msg.task_id === thinkingTaskId ? { ...msg, summary_text: "ルーティングテーブルを更新しました", isHidden: false } : msg
            ));
          });
        } catch (err) {
          console.error("Failed to listen to events:", err);
        }

        const historyBlock = getHistoryBlock(summaries, historyLimit);
        const promptWithContext = `【ユーザー入力】\n${userMessage}${historyBlock}`;

        const response: string = await invoke("ask_llm", { prompt: promptWithContext });
        unlisten(); 
        agentUnlisten();
        routeUnlisten();
        
        setMessages(prev => prev.map(msg => 
          msg.task_id === thinkingTaskId ? { ...msg, content: response, isToolLoading: false, isHidden: false } : msg
        ));
        
        console.log("LLM Response:", response);
        summarizeAndSave(`ユーザー入力: ${userMessage}\n回答: ${response}`, thinkingTaskId);
        
        // Support multiple tool calls in parallel
        const jsonBlocks = [...response.matchAll(/```(?:json)?\s*(\{[\s\S]*?"tool"[\s\S]*?\})\s*```/g)];
        const toolCalls = jsonBlocks.map(match => {
          try {
            return JSON.parse(match[1]);
          } catch (e) {
            return null;
          }
        }).filter(tc => tc !== null);

        if (toolCalls.length > 0) {
          // Keep the trigger message visible
          setMessages(prev => prev.map(msg => 
            msg.task_id === thinkingTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
          ));
          for (const toolCall of toolCalls) {
            console.log("Extracted tool call:", toolCall);
             const toolActionName = toolCall.tool === "self_network_ping" ? "Ping" : 
                                    toolCall.tool === "self_network_traceroute" ? "Traceroute" : 
                                    toolCall.tool === "network_get_hosts" ? "Host List" :
                                    toolCall.tool === "network_query_nw_db" || toolCall.tool === "query_nw_db" ? "NWDB検索" :
                                    toolCall.tool === "self_network_arp" ? "ARP Table" :
                                    toolCall.tool === "network_get_ip_info" ? "IP Info" :
                                    toolCall.tool === "network_list_serial_ports" ? "Serial Ports" :
                                    toolCall.tool === "network_send_console_message" ? "Console Message" :
                                    toolCall.tool === "network_show" ? "Show Command" :
                                    toolCall.tool === "fetch_config" ? "Fetch Config" :
                                    toolCall.tool === "fetch_routing" ? "Fetch Routing" :
                                    toolCall.tool === "fetch_arp" ? "Fetch ARP" :
                                    toolCall.tool === "require_host_regsterd" ? "ホスト登録要求" : toolCall.tool;
            
            // Execute in parallel (no await here, or wrap in Promise.all)
            executeAndAnalyze(userMessage, toolCall.tool, toolActionName, toolCall.args);
          }
        } else {
          const fallbackMatch = response.match(/\{[\s\S]*?"tool"[\s\S]*\}/);
          if (fallbackMatch) {
            try {
              const toolCall = JSON.parse(fallbackMatch[0]);
              setMessages(prev => prev.map(msg => 
                msg.task_id === thinkingTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
              ));
               const toolActionName = toolCall.tool === "self_network_ping" ? "Ping" : 
                                      toolCall.tool === "query_nw_db" || toolCall.tool === "network_query_nw_db" ? "NWDB検索" :
                                      toolCall.tool === "self_network_route" ? "Route Table" :
                                      toolCall.tool === "fetch_config" ? "Fetch Config" :
                                      toolCall.tool === "fetch_routing" ? "Fetch Routing" :
                                      toolCall.tool === "fetch_arp" ? "Fetch ARP" :
                                      toolCall.tool === "require_host_regsterd" ? "ホスト登録要求" : "Tool";
              executeAndAnalyze(userMessage, toolCall.tool, toolActionName, toolCall.args);
            } catch (e) {
              // Final response: make it visible
              setMessages(prev => prev.map(msg => 
                msg.task_id === thinkingTaskId ? { ...msg, isHidden: false, summary_text: "回答" } : msg
              ));
            }
          } else {
            // Final response: make it visible
            setMessages(prev => prev.map(msg => 
              msg.task_id === thinkingTaskId ? { ...msg, isHidden: false, summary_text: "回答" } : msg
            ));
          }
        }
      } catch (e: any) {
        setMessages(prev => prev.map(msg => 
          msg.task_id === thinkingTaskId ? { 
            ...msg, 
            content: `Error: ${e.toString()}`, 
            isHidden: false, 
            isToolLoading: false,
            status: "Failed"
          } : msg
        ));
      } finally {
        unlisten();
      }
    }
  };

  return { handleMcpResponse };
}
