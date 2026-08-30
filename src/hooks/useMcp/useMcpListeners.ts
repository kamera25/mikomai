import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Message, SummaryItem, ChatEvent } from "../../types";
import i18n from "../../i18n";

interface UseMcpListenersProps {
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  setSummaries: React.Dispatch<React.SetStateAction<SummaryItem[]>>;
  updateRecentHosts?: (hosts: string[]) => void;
}

export function mergeTaskContent(currentAccumulated: string, newContent: string): string {
  if (!currentAccumulated) return newContent || "";
  if (!newContent) return currentAccumulated;

  // 1. Exact match
  if (currentAccumulated === newContent) {
    return currentAccumulated;
  }

  // 2. newContent is continuation of currentAccumulated (starts with prefix)
  if (newContent.startsWith(currentAccumulated)) {
    return newContent;
  }

  // 3. currentAccumulated already contains newContent
  if (currentAccumulated.includes(newContent)) {
    return currentAccumulated;
  }

  // 4. In AgentLoop, step/decision logs are streamed via chunks, and the final report
  // is passed via mcpInitialFinished content. Append the final report text after the logs.
  return `${currentAccumulated}\n\n${newContent}`;
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

  // Map to track typing queue and state per task ID
  const taskStatesRef = useRef<
    Map<
      string,
      {
        targetContent: string;
        displayedContent: string;
        isTyping: boolean;
        timerId: any;
        isFinished: boolean;
        summaryText?: string;
      }
    >
  >(new Map());

  const TYPING_INTERVAL_MS = 10;

  const commitMessageContent = (
    taskId: string,
    content: string,
    isFinished: boolean,
    customSummaryText?: string
  ) => {
    const isAgent = content.includes("agent-step") || content.includes("agent-decision");
    setMessagesRef.current((prev) =>
      prev.map((msg) => {
        if (msg.task_id === taskId) {
          return {
            ...msg,
            content,
            isHidden: false,
            isToolLoading: isFinished ? false : msg.isToolLoading,
            summary_text: isAgent
              ? "エージェントによる解析を開始"
              : customSummaryText || msg.summary_text,
          } as Message;
        }
        return msg;
      })
    );
  };

  const getOrCreateTaskState = (taskId: string) => {
    let state = taskStatesRef.current.get(taskId);
    if (!state) {
      state = {
        targetContent: "",
        displayedContent: "",
        isTyping: false,
        timerId: null,
        isFinished: false,
      };
      taskStatesRef.current.set(taskId, state);
    }
    return state;
  };

  const startTyping = (taskId: string) => {
    const state = taskStatesRef.current.get(taskId);
    if (!state || state.isTyping) return;

    state.isTyping = true;

    const tick = () => {
      const targetChars = Array.from(state.targetContent);
      const displayedChars = Array.from(state.displayedContent);
      const remaining = targetChars.length - displayedChars.length;

      if (remaining > 0) {
        // Fast, snappy typing with dynamic step scaling for smooth UX
        let step = 1;
        if (remaining > 150) {
          step = Math.ceil(remaining / 30);
        } else if (remaining > 60) {
          step = 3;
        } else if (remaining > 20) {
          step = 2;
        }

        const nextChars = targetChars
          .slice(displayedChars.length, displayedChars.length + step)
          .join("");
        state.displayedContent += nextChars;
        commitMessageContent(taskId, state.displayedContent, false, state.summaryText);
        state.timerId = setTimeout(tick, TYPING_INTERVAL_MS);
      } else {
        // Finished typing all available target content
        state.isTyping = false;
        state.timerId = null;
        if (state.isFinished) {
          commitMessageContent(taskId, state.displayedContent, true, state.summaryText);
        }
      }
    };

    state.timerId = setTimeout(tick, TYPING_INTERVAL_MS);
  };

  const appendChunk = (taskId: string, chunk: string) => {
    const state = getOrCreateTaskState(taskId);
    const chunkChars = Array.from(chunk);
    state.targetContent += chunk;

    if (state.isTyping) {
      // Already typing; the loop will continue consuming targetContent
      return;
    }

    // If 6 or more characters are received at once, start character-by-character display
    if (chunkChars.length >= 6) {
      startTyping(taskId);
    } else {
      // 5 or fewer characters: display immediately
      state.displayedContent = state.targetContent;
      commitMessageContent(taskId, state.displayedContent, false, state.summaryText);
    }
  };

  const finishTaskContent = (taskId: string, finalContent?: string, summaryText?: string) => {
    const state = getOrCreateTaskState(taskId);
    if (summaryText) {
      state.summaryText = summaryText;
    }

    if (finalContent !== undefined && finalContent !== null) {
      state.targetContent = finalContent;
    }

    const targetChars = Array.from(state.targetContent);
    const displayedChars = Array.from(state.displayedContent);
    const remainingDiffCount = targetChars.length - displayedChars.length;

    state.isFinished = true;

    if (!state.isTyping) {
      if (remainingDiffCount >= 6) {
        startTyping(taskId);
      } else {
        state.displayedContent = state.targetContent;
        commitMessageContent(taskId, state.displayedContent, true, state.summaryText);
      }
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
            const targetTaskId = activeInitialTaskIdRef.current || activeAnalysisTaskIdRef.current;
            if (targetTaskId) {
              appendChunk(targetTaskId, chunk);
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
            const state = taskStatesRef.current.get(taskId);
            const currentAccumulated = state ? state.targetContent : "";
            const mergedContent = mergeTaskContent(currentAccumulated, content);

            finishTaskContent(taskId, mergedContent);

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

            if (shouldHide) {
              const state = taskStatesRef.current.get(taskId);
              if (state && state.timerId) {
                clearTimeout(state.timerId);
                state.timerId = null;
                state.isTyping = false;
              }
              setMessagesRef.current((prev) =>
                prev.map((msg) =>
                  msg.task_id === taskId
                    ? ({
                        ...msg,
                        content,
                        isHidden: true,
                        isToolLoading: false,
                        summary_text: summaryText,
                      } as Message)
                    : msg
                )
              );
            } else {
              finishTaskContent(taskId, content, summaryText);
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
      taskStatesRef.current.forEach((state) => {
        if (state.timerId) {
          clearTimeout(state.timerId);
          state.timerId = null;
        }
      });
      taskStatesRef.current.clear();
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
