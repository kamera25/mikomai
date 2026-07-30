import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Message, SummaryItem, ChatEvent } from "../../types";
import i18n from "../../i18n";

interface UseMcpListenersProps {
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  setSummaries: React.Dispatch<React.SetStateAction<SummaryItem[]>>;
  updateRecentHosts?: (hosts: string[]) => void;
}

export function useMcpListeners({
  setMessages,
  setSummaries,
  updateRecentHosts,
}: UseMcpListenersProps) {
  const activeAnalysisTaskIdRef = useRef<string | null>(null);
  const activeAnalysisContentRef = useRef<string>("");
  const activeInitialTaskIdRef = useRef<string | null>(null);
  const activeInitialContentRef = useRef<string>("");

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
    let unlistenFn: (() => void) | null = null;
    let unlistenDiffFn: (() => void) | null = null;
    let unlistenStatusFn: (() => void) | null = null;

    const setupListeners = async () => {
      const unlisten = await listen<ChatEvent>(
        "chat-event",
        (event) => {
          if (isCancelled) return;
          const chatEvent = event.payload;

          switch (chatEvent.type) {
            case "arpYamlSaved": {
              const { deviceName, savedPath } = chatEvent.payload;
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
              break;
            }

            case "routeYamlSaved": {
              const { deviceName, savedPath } = chatEvent.payload;
              setMessagesRef.current((prev) => {
                const next = prev.map((msg) => {
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
                });

                const targetTaskId = activeAnalysisTaskIdRef.current || activeInitialTaskIdRef.current;
                if (targetTaskId) {
                  return next.map((msg) =>
                    msg.task_id === targetTaskId
                      ? ({
                          ...msg,
                          summary_text: i18n.t("chat.routing_table_updated"),
                          isHidden: false,
                        } as Message)
                      : msg
                  );
                }
                return next;
              });
              break;
            }

            case "mcpToolStarted": {
              const { taskId, toolId, toolLabel, args, resolvedHost } = chatEvent.payload;
              const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
              const statusMsg = toolId === "validate_cisco_config"
                ? "Configのチェック中"
                : isRag
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
              break;
            }

            case "mcpToolFinished": {
              const { taskId, success, output, savedPath, isCached, cacheTime } = chatEvent.payload;
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
              break;
            }

            case "mcpAnalysisStarted": {
              const { analysisTaskId } = chatEvent.payload;
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
              break;
            }

            case "llmChunk": {
              const chunk = chatEvent.payload;
              if (activeInitialTaskIdRef.current) {
                activeInitialContentRef.current += chunk;
                setMessagesRef.current((prev) =>
                  prev.map((msg) =>
                    msg.task_id === activeInitialTaskIdRef.current
                      ? {
                          ...msg,
                          content: activeInitialContentRef.current,
                          isToolLoading: false,
                          isHidden: false,
                        }
                      : msg
                  )
                );
              } else if (activeAnalysisTaskIdRef.current) {
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
              break;
            }

            case "agentSelected": {
              const agentName = chatEvent.payload;
              const targetTaskId = activeAnalysisTaskIdRef.current || activeInitialTaskIdRef.current;
              if (targetTaskId) {
                const isAnalysis = activeAnalysisTaskIdRef.current === targetTaskId;
                setMessagesRef.current((prev) =>
                  prev.map((msg) =>
                    msg.task_id === targetTaskId
                      ? ({
                          ...msg,
                          summary_text: isAnalysis
                            ? i18n.t("chat.agent_analyzing", { agentName })
                            : i18n.t("chat.agent_processing", { agentName }),
                          isHidden: false,
                        } as Message)
                      : msg
                  )
                );
              }
              break;
            }

            case "mcpInitialStarted": {
              const { taskId, hasImage } = chatEvent.payload;
              activeInitialTaskIdRef.current = taskId;
              activeInitialContentRef.current = "";

              const initialText = hasImage
                ? i18n.t("chat.reading_image")
                : i18n.t("chat.thinking");

              setMessagesRef.current((prev) => [
                ...prev,
                {
                  role: "ai",
                  content: initialText,
                  timestamp: new Date().toISOString(),
                  isToolLoading: true,
                  isHidden: false,
                  task_id: taskId,
                  event_type: "AgentResponse",
                },
              ]);
              break;
            }

            case "mcpInitialFinished": {
              const { taskId, content } = chatEvent.payload;
              setMessagesRef.current((prev) =>
                prev.map((msg) =>
                  msg.task_id === taskId
                    ? ({
                        ...msg,
                        content,
                        isHidden: false,
                        isToolLoading: false,
                      } as Message)
                    : msg
                )
              );

              if (activeInitialTaskIdRef.current === taskId) {
                activeInitialTaskIdRef.current = null;
                activeInitialContentRef.current = "";
              }
              break;
            }

            case "mcpSummarySaved": {
              const { taskId, summaryText, summary, content } = chatEvent.payload;
              const shouldHide = content === "PENDING_DECISION" || content === "他の質問への回答を待っています...";
              setMessagesRef.current((prev) =>
                prev.map((msg) =>
                  msg.task_id === taskId
                    ? ({
                        ...msg,
                        content,
                        isHidden: shouldHide ? true : false,
                        isToolLoading: false,
                        summary_text: summaryText,
                      } as Message)
                    : msg
                )
              );

              if (!shouldHide) {
                setSummariesRef.current((prev) => {
                  const next = [...prev, summary];
                  return next.length > 20 ? next.slice(next.length - 20) : next;
                });
              }

              // Reset active analysis state
              if (activeAnalysisTaskIdRef.current === taskId) {
                activeAnalysisTaskIdRef.current = null;
                activeAnalysisContentRef.current = "";
              }
              break;
            }
          }
        }
      );

      const unlistenDiff = await listen<any>(
        "request-diff-commit",
        (event) => {
          if (isCancelled) return;
          const targetId = event.payload?.id;
          const updateMsg = () => {
            setMessagesRef.current((prev) => {
              let targetIdx = -1;
              if (targetId) {
                targetIdx = prev.findIndex((m) => m.task_id === targetId);
              }
              if (targetIdx === -1) {
                for (let i = prev.length - 1; i >= 0; i--) {
                  if (prev[i].tool_id === "validate_cisco_config" && prev[i].isToolLoading) {
                    targetIdx = i;
                    break;
                  }
                }
              }
              if (targetIdx !== -1) {
                return prev.map((msg, idx) =>
                  idx === targetIdx ? { ...msg, waitingForApproval: true } : msg
                );
              }
              return prev;
            });
          };

          updateMsg();
          setTimeout(updateMsg, 50);
          setTimeout(updateMsg, 200);
        }
      );

      const unlistenStatus = await listen<any>(
        "commit-status",
        (event) => {
          if (isCancelled) return;
          const targetId = event.payload?.id;
          setMessagesRef.current((prev) => {
            let targetIdx = -1;
            if (targetId) {
              targetIdx = prev.findIndex((m) => m.task_id === targetId);
            }
            if (targetIdx === -1) {
              for (let i = prev.length - 1; i >= 0; i--) {
                if (prev[i].tool_id === "validate_cisco_config" && prev[i].isToolLoading) {
                  targetIdx = i;
                  break;
                }
              }
            }
            if (targetIdx !== -1 && prev[targetIdx].waitingForApproval) {
              return prev.map((msg, idx) =>
                idx === targetIdx ? { ...msg, waitingForApproval: false } : msg
              );
            }
            return prev;
          });
        }
      );

      if (isCancelled) {
        unlisten();
        unlistenDiff();
        unlistenStatus();
      } else {
        unlistenFn = unlisten;
        unlistenDiffFn = unlistenDiff;
        unlistenStatusFn = unlistenStatus;
      }
    };

    setupListeners();

    return () => {
      isCancelled = true;
      if (unlistenFn) {
        unlistenFn();
      }
      if (unlistenDiffFn) {
        unlistenDiffFn();
      }
      if (unlistenStatusFn) {
        unlistenStatusFn();
      }
    };
  }, []); // Only register once on mount!
}
