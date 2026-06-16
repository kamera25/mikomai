import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Message, SummaryItem } from "../../types";
import i18n from "../../i18n";

interface UseMcpListenersProps {
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  setSummaries: React.Dispatch<React.SetStateAction<SummaryItem[]>>;
  updateRecentHosts?: (hosts: string[]) => void;
}

interface ToolStartedPayload {
  taskId: string;
  toolId: string;
  toolLabel: string;
  args: any;
  resolvedHost?: string;
}

interface ToolFinishedPayload {
  taskId: string;
  success: boolean;
  output: string;
  savedPath?: string;
  isCached?: boolean;
  cacheTime?: string;
}

interface AnalysisStartedPayload {
  taskId: string;
  analysisTaskId: string;
}

interface SummarySavedPayload {
  taskId: string;
  summaryText: string;
  summary: SummaryItem;
}

export function useMcpListeners({
  setMessages,
  setSummaries,
  updateRecentHosts,
}: UseMcpListenersProps) {
  const activeAnalysisTaskIdRef = useRef<string | null>(null);
  const activeAnalysisContentRef = useRef<string>("");

  // Use refs to avoid recreating Tauri listeners when handlers/state dependencies change
  const setMessagesRef = useRef(setMessages);
  const setSummariesRef = useRef(setSummaries);
  const updateRecentHostsRef = useRef(updateRecentHosts);

  // Sync refs on every render
  useEffect(() => {
    setMessagesRef.current = setMessages;
    setSummariesRef.current = setSummaries;
    updateRecentHostsRef.current = updateRecentHosts;
  });

  useEffect(() => {
    let isCancelled = false;
    const unlistenFns: (() => void)[] = [];

    const setupListeners = async () => {
      // 1. ARP yaml saved (from background)
      const uArp = await listen<{ deviceName: string; savedPath: string }>(
        "arp-yaml-saved",
        (event) => {
          if (isCancelled) return;
          const { deviceName, savedPath } = event.payload;
          setMessagesRef.current((prev) =>
            prev.map((msg) => {
              const msgDevice = msg.args?.deviceName || msg.args?.device_name;
              if (
                msg.event_type === "ToolExecution" &&
                msg.tool_id === "fetch_arp" &&
                msgDevice === deviceName &&
                !msg.saved_path
              ) {
                return { ...msg, saved_path: savedPath };
              }
              return msg;
            })
          );
        }
      );
      if (isCancelled) {
        uArp();
      } else {
        unlistenFns.push(uArp);
      }

      // 2. Route yaml saved (from background)
      const uRoute = await listen<{ deviceName: string; savedPath: string }>(
        "route-yaml-saved",
        (event) => {
          if (isCancelled) return;
          const { deviceName, savedPath } = event.payload;
          setMessagesRef.current((prev) =>
            prev.map((msg) => {
              const msgDevice = msg.args?.deviceName || msg.args?.device_name;
              if (
                msg.event_type === "ToolExecution" &&
                msg.tool_id === "fetch_routing" &&
                msgDevice === deviceName &&
                !msg.saved_path
              ) {
                return { ...msg, saved_path: savedPath };
              }
              return msg;
            })
          );
        }
      );
      if (isCancelled) {
        uRoute();
      } else {
        unlistenFns.push(uRoute);
      }

      // 3. MCP Tool Started
      const uToolStarted = await listen<ToolStartedPayload>(
        "mcp-tool-started",
        (event) => {
          if (isCancelled) return;
          const { taskId, toolId, toolLabel, args, resolvedHost } = event.payload;
          const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
          const statusMsg = isRag
            ? i18n.t("chat.searching_nwdb")
            : i18n.t("chat.running_tool", { toolLabel });

          setMessagesRef.current((prev) => [
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
              args,
            },
          ]);

          if (resolvedHost && resolvedHost.trim()) {
            updateRecentHostsRef.current?.([resolvedHost.trim()]);
          }
        }
      );
      if (isCancelled) {
        uToolStarted();
      } else {
        unlistenFns.push(uToolStarted);
      }

      // 4. MCP Tool Finished
      const uToolFinished = await listen<ToolFinishedPayload>(
        "mcp-tool-finished",
        (event) => {
          if (isCancelled) return;
          const { taskId, success, output, savedPath, isCached, cacheTime } = event.payload;
          setMessagesRef.current((prev) =>
            prev.map((msg) =>
              msg.task_id === taskId
                ? ({
                    ...msg,
                    isToolLoading: false,
                    status: success ? "Success" : "Failed",
                    summary_text: success
                      ? i18n.t("chat.tool_success", { toolLabel: msg.action_name })
                      : i18n.t("chat.tool_failed", { toolLabel: msg.action_name }),
                    raw_data: output || "No output provided",
                    saved_path: savedPath,
                    is_cached: isCached,
                    cache_time: cacheTime,
                  } as Message)
                : msg
            )
          );
        }
      );
      if (isCancelled) {
        uToolFinished();
      } else {
        unlistenFns.push(uToolFinished);
      }

      // 5. MCP Analysis Started
      const uAnalysisStarted = await listen<AnalysisStartedPayload>(
        "mcp-analysis-started",
        (event) => {
          if (isCancelled) return;
          const { analysisTaskId } = event.payload;
          activeAnalysisTaskIdRef.current = analysisTaskId;
          activeAnalysisContentRef.current = "";

          setMessagesRef.current((prev) => [
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
        }
      );
      if (isCancelled) {
        uAnalysisStarted();
      } else {
        unlistenFns.push(uAnalysisStarted);
      }

      // 6. LLM Chunk (streamed from analyze_tool_output or ask_llm_initial)
      const uLlmChunk = await listen<string>("llm-chunk", (event) => {
        if (isCancelled) return;
        const chunk = event.payload;
        if (activeAnalysisTaskIdRef.current) {
          activeAnalysisContentRef.current += chunk;
          setMessagesRef.current((prev) =>
            prev.map((msg) =>
              msg.task_id === activeAnalysisTaskIdRef.current
                ? {
                    ...msg,
                    content: activeAnalysisContentRef.current,
                    isToolLoading: false,
                    isHidden: false,
                  }
                : msg
            )
          );
        }
      });
      if (isCancelled) {
        uLlmChunk();
      } else {
        unlistenFns.push(uLlmChunk);
      }

      // 7. Agent Selected
      const uAgentSelected = await listen<string>("agent-selected", (event) => {
        if (isCancelled) return;
        const agentName = event.payload;
        if (activeAnalysisTaskIdRef.current) {
          setMessagesRef.current((prev) =>
            prev.map((msg) =>
              msg.task_id === activeAnalysisTaskIdRef.current
                ? ({
                    ...msg,
                    summary_text: i18n.t("chat.agent_analyzing", { agentName }),
                    isHidden: false,
                  } as Message)
                : msg
            )
          );
        }
      });
      if (isCancelled) {
        uAgentSelected();
      } else {
        unlistenFns.push(uAgentSelected);
      }

      // 8. MCP Summary Saved
      const uSummarySaved = await listen<SummarySavedPayload>(
        "mcp-summary-saved",
        (event) => {
          if (isCancelled) return;
          const { taskId, summaryText, summary } = event.payload;
          setMessagesRef.current((prev) =>
            prev.map((msg) =>
              msg.task_id === taskId
                ? ({
                    ...msg,
                    isHidden: false,
                    summary_text: summaryText,
                  } as Message)
                : msg
            )
          );

          setSummariesRef.current((prev) => {
            const next = [...prev, summary];
            return next.length > 20 ? next.slice(next.length - 20) : next;
          });

          // Reset active analysis state
          if (activeAnalysisTaskIdRef.current === taskId) {
            activeAnalysisTaskIdRef.current = null;
            activeAnalysisContentRef.current = "";
          }
        }
      );
      if (isCancelled) {
        uSummarySaved();
      } else {
        unlistenFns.push(uSummarySaved);
      }
    };

    setupListeners();

    return () => {
      isCancelled = true;
      unlistenFns.forEach((unlisten) => unlisten());
    };
  }, []); // Only register once on mount!
}
