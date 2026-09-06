import { chatService } from "../features/chat/chatService";
import { UseMcpProps } from "./useMcp/types";
import { useMcpListeners } from "./useMcp/useMcpListeners";
import { Attachment } from "../types";

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

  const handleMcpResponse = async (userMessage: string, attachments?: Attachment[]) => {
    try {
      await chatService.send({
        userMessage,
        summaries,
        recentIps: recentIPs || [],
        historyLimit,
        mcpTimeout,
        attachments,
      });
    } catch (e: unknown) {
      console.error("Failed to execute MCP message:", e);
    }
  };

  return { handleMcpResponse };
}
