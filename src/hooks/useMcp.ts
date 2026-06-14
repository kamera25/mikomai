import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UseMcpProps } from "./useMcp/types";
import { useMcpListeners } from "./useMcp/useMcpListeners";
import { useMcpExecutor } from "./useMcp/useMcpExecutor";
import { getHistoryBlock, extractJsonBlocks, getToolLabel } from "./useMcp/helpers";
import { parsePingCommand } from "../utils/commandParser";

export function useMcp({ 
  messages,
  setMessages, 
  summaries, 
  setSummaries, 
  historyLimit,
  mcpTimeout = 30,
  updateRecentHosts,
  recentIPs
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
    mcpTimeout,
    updateRecentHosts,
    recentIPs
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
    } else if (lowerInput.includes("ip") || lowerInput.includes("ネットワーク情報") ) {
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
        
        // Support multiple tool calls in parallel using robust brace counting
        const jsonBlocks = extractJsonBlocks(response);
        const toolCalls = jsonBlocks.map(block => {
          try {
            const parsed = JSON.parse(block);
            const tool = parsed.tool_name;
            const args = parsed.params || {};
            if (tool) {
              return { tool, args };
            }
            return null;
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
             const toolActionName = getToolLabel(toolCall.tool);
            
            // Execute in parallel (no await here, or wrap in Promise.all)
            executeAndAnalyze(userMessage, toolCall.tool, toolActionName, toolCall.args);
          }
        } else {
          // Final response: make it visible
          setMessages(prev => prev.map(msg => 
            msg.task_id === thinkingTaskId ? { ...msg, isHidden: false, summary_text: "回答" } : msg
          ));
        }
      } catch (e: unknown) {
        setMessages(prev => prev.map(msg => 
          msg.task_id === thinkingTaskId ? { 
            ...msg, 
            content: `Error: ${e instanceof Error ? e.message : String(e)}`, 
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
