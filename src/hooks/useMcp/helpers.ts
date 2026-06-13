import { SummaryItem } from "../../types";
import { invoke } from "@tauri-apps/api/core";

export function getHistoryBlock(items: SummaryItem[], limit: number): string {
  if (limit <= 0 || items.length === 0) return "";
  const recent = [...items].reverse().slice(0, limit);
  const text = recent.map((s, i) => `${i + 1}. ${s.content}`).join("\n");
  return `\n\n<memory>\n${text}\n</memory>`;
}

export async function normalizeArgs(toolId: string, userMessage: string, args: any, recentIPs?: string[]): Promise<any> {
  const processedArgs: any = args && typeof args === "object" && !Array.isArray(args)
    ? Object.keys(args).reduce((acc, key) => {
        const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
        acc[camelKey] = args[key];
        return acc;
      }, {} as any)
    : args;

  // Robust normalization for arguments
  if (processedArgs && typeof processedArgs === "object") {
    if (["fetch_config", "fetch_routing", "fetch_arp"].includes(toolId)) {
      let deviceVal = processedArgs.deviceName || processedArgs.device_name || processedArgs.device || processedArgs.host;
      if (!deviceVal) {
        try {
          const [connections, mcpHosts] = await Promise.all([
            invoke<any[]>("load_connections"),
            invoke<any[]>("get_mcp_hosts").catch(() => [])
          ]);
          const lowerMessage = userMessage.toLowerCase();
          let matched = connections?.find(c => 
            (c.hostname && lowerMessage.includes(c.hostname.toLowerCase())) ||
            (c.ip && lowerMessage.includes(c.ip))
          );
          if (!matched && mcpHosts) {
            matched = mcpHosts.find(h => 
              (h.hostname && lowerMessage.includes(h.hostname.toLowerCase())) ||
              (h.ip && lowerMessage.includes(h.ip))
            );
          }
          if (matched) {
            deviceVal = matched.hostname;
            console.log("[useMcp] Auto-extracted device name from user message:", deviceVal);
          }
        } catch (err) {
          console.error("[useMcp] Failed to resolve connections for auto-extraction:", err);
        }
      }
      
      // Fallback to session recent host if not found in args or message
      if (!deviceVal && recentIPs && recentIPs.length > 0) {
        deviceVal = recentIPs[0];
        console.log("[useMcp] Omitted device name, fallback to session's recent host:", deviceVal);
      }

      if (deviceVal) {
        processedArgs.deviceName = deviceVal;
      }
    } else if (["self_network_ping", "self_network_traceroute"].includes(toolId)) {
      let hostVal = processedArgs.host || processedArgs.device || processedArgs.deviceName || processedArgs.device_name || processedArgs.ip;
      
      // Fallback to session recent host if not found in args or message
      if (!hostVal && recentIPs && recentIPs.length > 0) {
        hostVal = recentIPs[0];
        console.log("[useMcp] Omitted host, fallback to session's recent host:", hostVal);
      }

      if (hostVal) {
        processedArgs.host = hostVal;
      }
    }
  }
  return processedArgs;
}
