import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UseMcpProps } from "./useMcp/types";
import { useMcpListeners } from "./useMcp/useMcpListeners";
import { useMcpExecutor } from "./useMcp/useMcpExecutor";
import { getHistoryBlock, extractJsonBlocks, getToolLabel } from "./useMcp/helpers";
import { Message, AskInitialPayload } from "../types";
import { getErrorMessage } from "../utils/error";
import i18n from "../i18n";

export function useMcp({
  messages,
  setMessages,
  summaries,
  setSummaries,
  historyLimit,
  mcpTimeout = 30,
  updateRecentHosts,
  recentIPs,
}: UseMcpProps) {
  // Setup Tauri event listeners using sub-hook
  useMcpListeners({ setMessages, setSummaries, updateRecentHosts });

  // Setup execution and analysis functions using sub-hook
  const { executeAndAnalyze, summarizeAndSave } = useMcpExecutor({
    messages,
    setMessages,
    summaries,
    setSummaries,
    historyLimit,
    mcpTimeout,
    updateRecentHosts,
    recentIPs,
  });

  const handleMcpResponse = async (userMessage: string) => {
    const thinkingTaskId = `task_think_${Date.now()}`;
    // Intermediate thinking message (hidden)
    setMessages((prev) => [
      ...prev,
      {
        role: "ai",
        content: i18n.t("chat.thinking"),
        timestamp: new Date().toISOString(),
        isToolLoading: true,
        isHidden: true,
        task_id: thinkingTaskId,
        event_type: "AgentResponse",
      },
    ]);

    let fullContent = "";
    let unlisten: () => void = () => {};
    let agentUnlisten = () => {};
    let routeUnlisten = () => {};

    try {
      unlisten = await listen<string>("llm-chunk", (event) => {
        fullContent += event.payload;
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === thinkingTaskId
              ? ({ ...msg, content: fullContent, isToolLoading: false, isHidden: false } as Message)
              : msg
          )
        );
      });

      try {
        agentUnlisten = await listen<string>("agent-selected", (event) => {
          const agentName = event.payload;
          setMessages((prev) =>
            prev.map((msg) =>
              msg.task_id === thinkingTaskId
                ? ({ ...msg, summary_text: i18n.t("chat.agent_processing", { agentName }), isHidden: false } as Message)
                : msg
            )
          );
        });
        routeUnlisten = await listen<string>("route-yaml-saved", () => {
          setMessages((prev) =>
            prev.map((msg) =>
              msg.task_id === thinkingTaskId
                ? ({
                    ...msg,
                    summary_text: i18n.t("chat.routing_table_updated"),
                    isHidden: false,
                  } as Message)
                : msg
            )
          );
        });
      } catch (err) {
        console.error("Failed to listen to events:", err);
      }

      const historyBlock = getHistoryBlock(summaries, historyLimit);
      const promptWithContext = `【ユーザー入力】\n${userMessage}${historyBlock}`;

      const payload: AskInitialPayload = { prompt: promptWithContext };
      const response: string = await invoke("ask_llm_initial", { payload });
      unlisten();
      agentUnlisten();
      routeUnlisten();

      setMessages((prev) =>
        prev.map((msg) =>
          msg.task_id === thinkingTaskId
            ? ({ ...msg, content: response, isToolLoading: false, isHidden: false } as Message)
            : msg
        )
      );

      console.log("LLM Response:", response);
      summarizeAndSave(`ユーザー入力: ${userMessage}\n回答: ${response}`, thinkingTaskId);

      // Support multiple tool calls in parallel using robust brace counting
      const jsonBlocks = extractJsonBlocks(response);
      const toolCalls = jsonBlocks
        .map((block) => {
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
        })
        .filter((tc) => tc !== null);

      if (toolCalls.length > 0) {
        // Keep the trigger message visible
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === thinkingTaskId
              ? ({ ...msg, isHidden: false, summary_text: i18n.t("chat.summarizing") } as Message)
              : msg
          )
        );
        for (const toolCall of toolCalls) {
          console.log("Extracted tool call:", toolCall);
          const toolActionName = getToolLabel(toolCall.tool);

          // Execute in parallel (no await here, or wrap in Promise.all)
          executeAndAnalyze(userMessage, toolCall.tool, toolActionName, toolCall.args);
        }
      } else {
        // Final response: make it visible
        setMessages((prev) =>
          prev.map((msg) =>
            msg.task_id === thinkingTaskId
              ? ({ ...msg, isHidden: false, summary_text: i18n.t("chat.answer") } as Message)
              : msg
          )
        );
      }
    } catch (e: unknown) {
      setMessages((prev) =>
        prev.map((msg) =>
          msg.task_id === thinkingTaskId
            ? ({
                ...msg,
                content: `Error: ${getErrorMessage(e)}`,
                isHidden: false,
                isToolLoading: false,
                status: "Failed",
              } as Message)
            : msg
        )
      );
    } finally {
      unlisten();
    }
  };

  return { handleMcpResponse };
}
