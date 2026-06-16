import { invoke } from "@tauri-apps/api/core";
import { UseMcpProps } from "./types";
import { Message } from "../../types";

export function useMcpExecutor({
  setMessages,
  summaries,
  setSummaries,
  historyLimit,
  mcpTimeout = 30,
  recentIPs,
}: UseMcpProps) {
  const summarizeAndSave = async (content: string, taskId?: string) => {
    try {
      const summaryPrompt = `以下の内容を要約してください。\n\n${content}`;
      const summaryText: string = await invoke("ask_llm_background", { prompt: summaryPrompt });
      const newSummary = { timestamp: new Date().toISOString(), content: summaryText };
      await invoke("save_summary", { summary: newSummary });
      setSummaries((prev) => {
        const next = [...prev, newSummary];
        return next.length > 20 ? next.slice(next.length - 20) : next;
      });

      if (taskId) {
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === taskId ? ({ ...msg, summary_text: summaryText } as Message) : msg
          )
        );
      }
    } catch (e) {
      console.error("Failed to generate/save summary:", e);
    }
  };

  const executeAndAnalyze = async (
    userMessage: string,
    toolId: string,
    toolLabel: string,
    args: any
  ) => {
    const taskId = `task_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;

    try {
      console.log(
        "[useMcp] invoking backend executor:",
        toolId,
        "with args:",
        JSON.stringify(args)
      );

      await invoke("execute_mcp_tool", {
        taskId,
        toolId,
        toolLabel,
        userMessage,
        args: args || {},
        summaries,
        recentIps: recentIPs || [],
        historyLimit,
        mcpTimeout,
      });
    } catch (e) {
      console.error("Failed to execute MCP tool on backend:", e);
    }
  };

  return {
    executeAndAnalyze,
    summarizeAndSave,
  };
}
