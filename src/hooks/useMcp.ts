import { invoke } from "@tauri-apps/api/core";
import { UseMcpProps } from "./useMcp/types";
import { useMcpListeners } from "./useMcp/useMcpListeners";

export function useMcp({
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

  const handleMcpResponse = async (userMessage: string) => {
    try {
      await invoke("handle_mcp_message", {
        payload: {
          userMessage,
          summaries,
          recentIps: recentIPs || [],
          historyLimit,
          mcpTimeout,
        },
      });
    } catch (e: unknown) {
      console.error("Failed to execute MCP message:", e);
    }
  };

  return { handleMcpResponse };
}

