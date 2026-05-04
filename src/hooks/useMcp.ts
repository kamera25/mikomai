import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Message, SummaryItem } from "../types";

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

  const summarizeAndSave = async (content: string) => {
    try {
      const summaryPrompt = `以下の内容を要約してください。\n\n${content}`;
      const summaryText: string = await invoke("ask_llm_background", { prompt: summaryPrompt });
      const newSummary = { timestamp: new Date().toISOString(), content: summaryText };
      await invoke("save_summary", { summary: newSummary });
      setSummaries(prev => {
        const next = [...prev, newSummary];
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
      const timeoutPromise = new Promise((_, reject) =>
        setTimeout(() => reject(new Error("MCP execution timed out")), mcpTimeout * 1000)
      );

      const result: any = await Promise.race([
        invoke(toolId, args),
        timeoutPromise
      ]);
      const statusBadge = result.success ? "✅ 成功" : "❌ 失敗";
      const resultMessage = result.success ? 
        `### ${toolLabel} 実行結果: ${statusBadge}\n\`\`\`terminal\n${result.output}\n\`\`\`` :
        `⚠️ **${toolLabel}の実行に失敗しました: ${statusBadge}**\n\n【エラー内容】\n\`\`\`terminal\n${result.output}\n\`\`\``;
      
      if (isRag) {
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
                                     nextToolCall.tool === "network_list_serial_ports" ? "Serial Ports" :
                                     nextToolCall.tool === "network_send_console_message" ? "Console Message" :
                                     nextToolCall.tool === "network_show" ? "Show Command" : nextToolCall.tool;
          
          setTimeout(async () => {
            await executeAndAnalyze(userMessage, nextToolCall.tool, nextToolActionName, nextToolCall.args, depth + 1, executedTools);
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

  const handleMcpResponse = async (userMessage: string) => {
    const lowerInput = userMessage.toLowerCase();
    
    let pingArgs: any = null;
    const pingBaseMatch = lowerInput.match(/(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)/) || 
                          lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)/);
    
    if (pingBaseMatch) {
      const host = pingBaseMatch[1];
      pingArgs = { host };
      
      const sizeMatch = lowerInput.match(/(?:size|サイズ)\s*(\d+)/);
      if (sizeMatch) pingArgs.size = parseInt(sizeMatch[1]);
      
      const countMatch = lowerInput.match(/(?:count|回数|回|回実行)\s*(\d+)/);
      if (countMatch) pingArgs.count = parseInt(countMatch[1]);

      if (lowerInput.includes("df") || lowerInput.includes("フラグメント禁止") || lowerInput.includes("断片化禁止")) {
        pingArgs.df = true;
      }
    }
    
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
        setMessages(prev => [...prev, { role: "ai", content: "思考中...", timestamp: new Date().toISOString(), isToolLoading: true }]);
      }
    } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
      await executeAndAnalyze(userMessage, "network_show", "Show Command", {
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
        unlisten(); 
        
        console.log("LLM Response:", response);
        summarizeAndSave(`ユーザー入力: ${userMessage}\n回答: ${response}`);
        
        let jsonStr = "";
        const jsonBlockMatch = response.match(/```(?:json)?\s*(\{[\s\S]*?"tool"[\s\S]*?\})\s*```/);
        if (jsonBlockMatch) {
          jsonStr = jsonBlockMatch[1];
        } else {
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
                                   toolCall.tool === "network_list_serial_ports" ? "Serial Ports" :
                                   toolCall.tool === "network_send_console_message" ? "Console Message" :
                                   toolCall.tool === "network_show" ? "Show Command" : toolCall.tool;
            
            await executeAndAnalyze(userMessage, toolCall.tool, toolActionName, toolCall.args);
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
  };

  return { handleMcpResponse };
}
