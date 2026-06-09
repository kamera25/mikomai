import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Message, SummaryItem } from "../types";
import { parsePingCommand } from "../utils/commandParser";

interface UseMcpProps {
  messages: Message[];
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  summaries: SummaryItem[];
  setSummaries: React.Dispatch<React.SetStateAction<SummaryItem[]>>;
  historyLimit: number;
  mcpTimeout?: number;
}

export function useMcp({ 
  setMessages, 
  summaries, 
  setSummaries, 
  historyLimit,
  mcpTimeout = 30
}: UseMcpProps) {

  const summarizeAndSave = async (content: string, taskId?: string) => {
    try {
      const summaryPrompt = `以下の内容を要約してください。\n\n${content}`;
      const summaryText: string = await invoke("ask_llm_background", { prompt: summaryPrompt });
      const newSummary = { timestamp: new Date().toISOString(), content: summaryText };
      await invoke("save_summary", { summary: newSummary });
      setSummaries(prev => {
        const next = [...prev, newSummary];
        return next.length > 20 ? next.slice(next.length - 20) : next;
      });

      if (taskId) {
        setMessages(prev => prev.map(msg => 
          msg.task_id === taskId ? { ...msg, summary_text: summaryText } : msg
        ));
      }
    } catch (e) {
      console.error("Failed to generate/save summary:", e);
    }
  };

  const getHistoryBlock = (items: SummaryItem[], limit: number) => {
    if (limit <= 0 || items.length === 0) return "";
    const recent = [...items].reverse().slice(0, limit);
    const text = recent.map((s, i) => `${i + 1}. ${s.content}`).join("\n");
    return `\n\n<memory>\n${text}\n</memory>`;
  };

  const executeAndAnalyze = async (
    userMessage: string,
    toolId: string, 
    toolLabel: string, 
    args: any, 
    depth: number = 0, 
    executedTools: Set<string> = new Set()
  ) => {
    const toolSignature = `${toolId}:${JSON.stringify(args)}`;
    if (executedTools.has(toolSignature)) {
      setMessages(prev => [...prev, { 
        role: "ai", 
        content: "以上、報告いたします。", 
        timestamp: new Date().toISOString(),
        event_type: "AgentResponse"
      }]);
      return;
    }
    executedTools.add(toolSignature);

    if (depth > 3) {
      setMessages(prev => [...prev, { role: "ai", content: "エラー: ツール呼び出しのループが深すぎるため中断しました。", timestamp: new Date().toISOString() }]);
      return;
    }

    const taskId = `task_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
    const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
    const statusMsg = isRag ? `NW-DBを検索中...` : `${toolLabel} を実行中...`;
    
    // Add ToolExecution block
    setMessages(prev => [...prev, {
      role: "ai",
      content: "",
      timestamp: new Date().toISOString(),
      isToolLoading: true,
      task_id: taskId,
      event_type: "ToolExecution",
      status: "Running",
      action_name: toolLabel,
      tool_id: toolId,
      summary_text: statusMsg,
      raw_data: null,
      args
    }]);

    try {
      const timeoutPromise = new Promise((_, reject) =>
        setTimeout(() => reject(new Error("MCP execution timed out")), mcpTimeout * 1000)
      );

      const result: any = await Promise.race([
        invoke(toolId, args),
        timeoutPromise
      ]);

      // Update ToolExecution block
      setMessages(prev => prev.map(msg =>
        msg.task_id === taskId ? {
          ...msg,
          isToolLoading: false,
          status: result.success ? "Success" : "Failed",
          summary_text: result.success ? `${toolLabel} 完了` : `${toolLabel} 失敗`,
          raw_data: result.output || "No output provided"
        } : msg
      ));

      const historyBlock = getHistoryBlock(summaries, historyLimit);
      const analysisPrompt = isRag ? 
        `ユーザーの質問: "${userMessage}"\nに対して、技術文書データベース(NW-DB)から以下の情報を取得しました:\n\n${result.output}\n\nこの内容に基づき、ネットワークエンジニアの視点で、ユーザーの質問に対する的確な回答を日本語で生成してください。回答には、参照した資料の内容を具体的に含めてください。${historyBlock}` :
        `ユーザーの入力: "${userMessage}"\nに対する${toolLabel}の実行結果は以下の通りです:\n\n${result.output}\n\nこの結果を分析し、ネットワークエンジニアの視点で状況を日本語で簡潔に報告してください。\n\n # 重要! \n\n既にツールは実行済みです。この回答内で再度同じコマンド、かつ同じ引数でツール呼び出し（JSONフォーマット）を出力することは絶対に避けてください。結果の解説と、次にユーザーが実行すべきアクションの提案のみを行ってください。${historyBlock}`;
      
      const analysisTaskId = `task_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;

      // Hide the intermediate "Analyzing..." or thinking process
      setMessages(prev => [...prev, {
        role: "ai",
        content: "分析中...",
        timestamp: new Date().toISOString(),
        isToolLoading: true,
        isHidden: true, // Hide by default
        task_id: analysisTaskId,
        event_type: "AgentResponse"
      }]);
      
      let analysisContent = "";
      const analysisUnlisten = await listen<string>("llm-chunk", (event) => {
        analysisContent += event.payload;
        setMessages(prev => prev.map(msg =>
          msg.task_id === analysisTaskId ? { ...msg, content: analysisContent, isToolLoading: false, isHidden: false } : msg
        ));
      });

      let agentUnlisten = () => {};
      try {
        agentUnlisten = await listen<string>("agent-selected", (event) => {
          const agentName = event.payload;
          setMessages(prev => prev.map(msg =>
            msg.task_id === analysisTaskId ? { ...msg, summary_text: `${agentName} が分析中...`, isHidden: false } : msg
          ));
        });
      } catch (err) {
        console.error("Failed to listen to agent-selected:", err);
      }

      let responseStr = "";
      try {
        responseStr = await invoke("ask_llm", { prompt: analysisPrompt });
        setMessages(prev => prev.map(msg =>
          msg.task_id === analysisTaskId ? { ...msg, content: responseStr, isToolLoading: false, isHidden: false } : msg
        ));
      } catch (analysisError: any) {
        console.error("Failed to get analysis", analysisError);
      } finally {
        analysisUnlisten();
        agentUnlisten();
      }

      // Support multiple tool calls in parallel
      const nextJsonBlocks = [...responseStr.matchAll(/```(?:json)?\s*(\{[\s\S]*?"tool"[\s\S]*?\})\s*```/g)];
      const nextToolCalls = nextJsonBlocks.map(match => {
        try {
          return JSON.parse(match[1]);
        } catch (e) {
          return null;
        }
      }).filter(tc => tc !== null);

      if (nextToolCalls.length > 0) {
        setMessages(prev => prev.map(msg => 
          msg.task_id === analysisTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
        ));
        summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`, analysisTaskId);
        for (const nextToolCall of nextToolCalls) {
          console.log("Extracted subsequent tool call:", nextToolCall);
          const nextToolActionName = nextToolCall.tool === "network_ping" ? "Ping" : 
                                     nextToolCall.tool === "network_traceroute" ? "Traceroute" : 
                                     nextToolCall.tool === "network_get_hosts" ? "Host List" :
                                     nextToolCall.tool === "network_query_nw_db" || nextToolCall.tool === "query_nw_db" ? "NWDB検索" :
                                     nextToolCall.tool === "network_arp" ? "ARP Table" :
                                     nextToolCall.tool === "network_get_ip_info" ? "IP Info" :
                                     nextToolCall.tool === "network_list_serial_ports" ? "Serial Ports" :
                                     nextToolCall.tool === "network_send_console_message" ? "Console Message" :
                                     nextToolCall.tool === "network_show" ? "Show Command" : nextToolCall.tool;
          
          setTimeout(async () => {
            await executeAndAnalyze(userMessage, nextToolCall.tool, nextToolActionName, nextToolCall.args, depth + 1, executedTools);
          }, 1000);
        }
      } else {
        const nextFallbackMatch = responseStr.match(/\{[\s\S]*?"tool"[\s\S]*\}/);
        if (nextFallbackMatch) {
          try {
            const nextToolCall = JSON.parse(nextFallbackMatch[0]);
            setMessages(prev => prev.map(msg => 
              msg.task_id === analysisTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
            ));
            summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`, analysisTaskId);
            const nextToolActionName = nextToolCall.tool === "network_ping" ? "Ping" : 
                                       nextToolCall.tool === "query_nw_db" || nextToolCall.tool === "network_query_nw_db" ? "NWDB検索" : "Tool";
            setTimeout(async () => {
              await executeAndAnalyze(userMessage, nextToolCall.tool, nextToolActionName, nextToolCall.args, depth + 1, executedTools);
            }, 1000);
          } catch (e) {
            // Final response: make it visible
            setMessages(prev => prev.map(msg => 
              msg.task_id === analysisTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
            ));
            summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`, analysisTaskId);
          }
        } else {
          // Final response: make it visible
          setMessages(prev => prev.map(msg => 
            msg.task_id === analysisTaskId ? { ...msg, isHidden: false, summary_text: "回答要約中..." } : msg
          ));
          summarizeAndSave(`ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`, analysisTaskId);
        }
      }

    } catch (e: any) {
      const errorMsg = e.toString();

      setMessages(prev => prev.map(msg =>
        msg.task_id === taskId ? {
          ...msg,
          isToolLoading: false,
          status: "Failed",
          summary_text: `${toolLabel} エラー`,
          raw_data: errorMsg
        } : msg
      ));
    }
  };

  const handleMcpResponse = async (userMessage: string) => {
    const lowerInput = userMessage.toLowerCase();
    
    const pingArgs = parsePingCommand(userMessage);
    
    const traceMatch = lowerInput.match(/(?:trace(?:route)?|トレース|トレースルート)\s+([a-zA-Z0-9.:-]+)/) ||
                       lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:trace(?:route)?|トレース|トレースルート)/);

    const hostListMatch = lowerInput.match(/(?:host|ホスト|接続先|ターゲット).*(?:list|一覧|教え|見せ|確認)/) || 
                          lowerInput.match(/(?:list|一覧|教え|見せ|確認).*(?:host|ホスト|接続先|ターゲット)/);

    if (pingArgs) {
      await executeAndAnalyze(userMessage, "network_ping", "Ping", pingArgs);
    } else if (traceMatch) {
      const host = traceMatch[1] || traceMatch[2];
      await executeAndAnalyze(userMessage, "network_traceroute", "Traceroute", { host });
    } else if (hostListMatch) {
      await executeAndAnalyze(userMessage, "network_get_hosts", "Host List", {});
    } else if (lowerInput.includes("arp")) {
      await executeAndAnalyze(userMessage, "network_arp", "ARP Table", {});
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
    } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
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
        } catch (err) {
          console.error("Failed to listen to agent-selected:", err);
        }

        const historyBlock = getHistoryBlock(summaries, historyLimit);
        const promptWithContext = `【ユーザー入力】\n${userMessage}${historyBlock}`;

        const response: string = await invoke("ask_llm", { prompt: promptWithContext });
        unlisten(); 
        agentUnlisten();
        
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
            const toolActionName = toolCall.tool === "network_ping" ? "Ping" : 
                                   toolCall.tool === "network_traceroute" ? "Traceroute" : 
                                   toolCall.tool === "network_get_hosts" ? "Host List" :
                                   toolCall.tool === "network_query_nw_db" || toolCall.tool === "query_nw_db" ? "NWDB検索" :
                                   toolCall.tool === "network_arp" ? "ARP Table" :
                                   toolCall.tool === "network_get_ip_info" ? "IP Info" :
                                   toolCall.tool === "network_list_serial_ports" ? "Serial Ports" :
                                   toolCall.tool === "network_send_console_message" ? "Console Message" :
                                   toolCall.tool === "network_show" ? "Show Command" : toolCall.tool;
            
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
              const toolActionName = toolCall.tool === "network_ping" ? "Ping" : 
                                     toolCall.tool === "query_nw_db" || toolCall.tool === "network_query_nw_db" ? "NWDB検索" : "Tool";
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
