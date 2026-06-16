import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UseMcpProps } from "./types";
import { getHistoryBlock, normalizeArgs } from "./helpers";
import { TauriCommandResult, Message, AnalyzePayload } from "../../types";
import { getErrorMessage } from "../../utils/error";
import i18n from "../../i18n";

export function useMcpExecutor({
  setMessages,
  summaries,
  setSummaries,
  historyLimit,
  mcpTimeout = 30,
  updateRecentHosts,
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
    const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
    const statusMsg = isRag ? i18n.t("chat.searching_nwdb") : i18n.t("chat.running_tool", { toolLabel });

    // Normalize arguments using helper function
    const processedArgs = await normalizeArgs(toolId, userMessage, args, recentIPs);

    // Extract target host and update recent hosts
    if (updateRecentHosts && processedArgs) {
      const argsObj = processedArgs as Record<string, unknown> & {
        device?: string | { host?: string; hostname?: string };
        deviceName?: string;
        host?: string;
      };
      const device = argsObj.device;
      const host =
        argsObj.deviceName ||
        argsObj.host ||
        (typeof device === "string" ? device : device?.host || device?.hostname);

      if (typeof host === "string" && host.trim()) {
        updateRecentHosts([host.trim()]);
      }
    }

    // Add ToolExecution block
    setMessages((prev) => [
      ...prev,
      {
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
        args: processedArgs,
      },
    ]);

    try {
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error("MCP execution timed out")), mcpTimeout * 1000)
      );

      console.log(
        "[useMcp] invoking Tauri command:",
        toolId,
        "with args:",
        JSON.stringify(processedArgs)
      );
      const result = await Promise.race([
        invoke<TauriCommandResult>(toolId, processedArgs || {}),
        timeoutPromise,
      ]);

      // Update ToolExecution block
      setMessages((prev) =>
        prev.map((msg) =>
          msg.task_id === taskId
            ? ({
                ...msg,
                isToolLoading: false,
                status: result.success ? "Success" : "Failed",
                summary_text: result.success ? i18n.t("chat.tool_success", { toolLabel }) : i18n.t("chat.tool_failed", { toolLabel }),
                raw_data: result.output || "No output provided",
                saved_path: result.saved_path,
                is_cached: result.is_cached,
                cache_time: result.cache_time,
              } as Message)
            : msg
        )
      );

      const historyBlock = getHistoryBlock(summaries, historyLimit);
      const analysisTaskId = `task_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;

      // Hide the intermediate "Analyzing..." or thinking process
      setMessages((prev) => [
        ...prev,
        {
          role: "ai",
          content: i18n.t("chat.analyzing"),
          timestamp: new Date().toISOString(),
          isToolLoading: true,
          isHidden: true, // Hide by default
          task_id: analysisTaskId,
          event_type: "AgentResponse",
        },
      ]);

      let analysisContent = "";
      const analysisUnlisten = await listen<string>("llm-chunk", (event) => {
        analysisContent += event.payload;
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === analysisTaskId
              ? ({
                  ...msg,
                  content: analysisContent,
                  isToolLoading: false,
                  isHidden: false,
                } as Message)
              : msg
          )
        );
      });

      let agentUnlisten = () => {};
      try {
        agentUnlisten = await listen<string>("agent-selected", (event) => {
          const agentName = event.payload;
          setMessages((prev) =>
            prev.map((msg) =>
              msg.task_id === analysisTaskId
                ? ({ ...msg, summary_text: i18n.t("chat.agent_analyzing", { agentName }), isHidden: false } as Message)
                : msg
            )
          );
        });
      } catch (err) {
        console.error("Failed to listen to agent-selected:", err);
      }

      let responseStr = "";
      try {
        const payload: AnalyzePayload = {
          userMessage,
          toolLabel,
          output: result.output || "",
          isRag,
          historyBlock,
        };
        responseStr = await invoke("analyze_tool_output", { payload });
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === analysisTaskId
              ? ({ ...msg, content: responseStr, isToolLoading: false, isHidden: false } as Message)
              : msg
          )
        );
      } catch (analysisError: any) {
        console.error("Failed to get analysis", analysisError);
      } finally {
        analysisUnlisten();
        agentUnlisten();
      }

      // Final response: make it visible
      setMessages((prev) =>
        prev.map((msg) =>
          msg.task_id === analysisTaskId
            ? ({ ...msg, isHidden: false, summary_text: i18n.t("chat.summarizing") } as Message)
            : msg
        )
      );
      summarizeAndSave(
        `ユーザー入力: ${userMessage}\n実行ツール: ${toolLabel}\n分析結果: ${responseStr}`,
        analysisTaskId
      );
    } catch (e: unknown) {
      const errorMsg = getErrorMessage(e);

      setMessages((prev) =>
        prev.map((msg) =>
          msg.task_id === taskId
            ? ({
                ...msg,
                isToolLoading: false,
                status: "Failed",
                summary_text: i18n.t("chat.tool_error", { toolLabel }),
                raw_data: errorMsg,
              } as Message)
            : msg
        )
      );
    }
  };

  return {
    executeAndAnalyze,
    summarizeAndSave,
  };
}
