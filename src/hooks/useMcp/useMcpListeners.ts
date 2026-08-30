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

  const chunkRafIdRef = useRef<number | null>(null);
  const lastChunkCommitRef = useRef(0);
  const chunkCommitDelayRef = useRef<number | null>(null);

  // Rendering a growing Markdown document is substantially more expensive than
  // receiving a token. Keep the latest text in refs and commit it at most a few
  // times per second, while still aligning the work with the next paint.
  const CHUNK_RENDER_INTERVAL_MS = 100;

  const scheduleChunkUpdate = () => {
    if (chunkRafIdRef.current !== null || chunkCommitDelayRef.current !== null) return;
    const elapsed = performance.now() - lastChunkCommitRef.current;
    const commit = () => {
      chunkCommitDelayRef.current = null;
      chunkRafIdRef.current = requestAnimationFrame(() => {
        chunkRafIdRef.current = null;
        lastChunkCommitRef.current = performance.now();
        const targetTaskId = activeInitialTaskIdRef.current || activeAnalysisTaskIdRef.current;
        const targetContent = activeInitialTaskIdRef.current
          ? activeInitialContentRef.current
          : activeAnalysisContentRef.current;

        if (targetTaskId) {
          const isAgent =
            targetContent.includes("agent-step") || targetContent.includes("agent-decision");
          setMessagesRef.current((prev) =>
            prev.map((msg) =>
              msg.task_id === targetTaskId
                ? ({
                    ...msg,
                    content: targetContent,
                    isHidden: false,
                    summary_text: isAgent ? "エージェントによる解析を開始" : msg.summary_text,
                  } as Message)
                : msg
            )
          );
        }
      });
    };

    if (elapsed >= CHUNK_RENDER_INTERVAL_MS) {
      commit();
    } else {
      chunkCommitDelayRef.current = window.setTimeout(commit, CHUNK_RENDER_INTERVAL_MS - elapsed);
    }
  };

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
      const unlisten = await listen<ChatEvent>("chat-event", (event) => {
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

              const targetTaskId =
                activeAnalysisTaskIdRef.current || activeInitialTaskIdRef.current;
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
            const { taskId, toolId, args, resolvedHost } = chatEvent.payload;
            const toolLabel = i18n.t(`tools.${toolId}`, { defaultValue: toolId });
            const isRag = toolId === "query_nw_db" || toolId === "network_query_nw_db";
            const statusMsg =
              toolId === "validate_cisco_config"
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
              scheduleChunkUpdate();
            } else if (activeAnalysisTaskIdRef.current) {
              activeAnalysisContentRef.current += chunk;
              scheduleChunkUpdate();
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
                        summary_text:
                          agentName === "エージェントによる解析を開始"
                            ? agentName
                            : isAnalysis
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
            const { taskId, hasImage } = chatEvent.payload as any;
            activeInitialTaskIdRef.current = taskId;
            activeInitialContentRef.current = "";

            const initialText = hasImage ? i18n.t("chat.reading_image") : i18n.t("chat.thinking");

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
            const currentAccumulated =
              activeInitialTaskIdRef.current === taskId ? activeInitialContentRef.current : "";
            const mergedContent = currentAccumulated
              ? content
                ? `${currentAccumulated}\n\n${content}`
                : currentAccumulated
              : content;
            const isAgent =
              mergedContent?.includes("agent-step") || mergedContent?.includes("agent-decision");

            setMessagesRef.current((prev) =>
              prev.map((msg) =>
                msg.task_id === taskId
                  ? ({
                      ...msg,
                      content: mergedContent,
                      isHidden: false,
                      isToolLoading: false,
                      summary_text: isAgent
                        ? "エージェントによる解析を開始"
                        : msg.summary_text,
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
            const shouldHide =
              content === "PENDING_DECISION" || content === "他の質問への回答を待っています...";
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
      });

      const unlistenDiff = await listen<any>("request-diff-commit", (event) => {
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
      });

      const unlistenStatus = await listen<any>("commit-status", (event) => {
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
          if (targetIdx !== -1 && (prev[targetIdx] as any).waitingForApproval) {
            return prev.map((msg, idx) =>
              idx === targetIdx ? { ...msg, waitingForApproval: false } : msg
            );
          }
          return prev;
        });
      });

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
      if (chunkRafIdRef.current !== null) {
        cancelAnimationFrame(chunkRafIdRef.current);
        chunkRafIdRef.current = null;
      }
      if (chunkCommitDelayRef.current !== null) {
        clearTimeout(chunkCommitDelayRef.current);
        chunkCommitDelayRef.current = null;
      }
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
