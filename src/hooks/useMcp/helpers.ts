import { SummaryItem } from "../../types";
import { invoke } from "@tauri-apps/api/core";

export function getHistoryBlock(items: SummaryItem[], limit: number): string {
  if (limit <= 0 || items.length === 0) return "";
  const recent = [...items].reverse().slice(0, limit);
  const text = recent.map((s, i) => `${i + 1}. ${s.content}`).join("\n");
  return `\n\n<memory>\n${text}\n</memory>`;
}

export function extractJsonBlocks(text: string): string[] {
  const blocks: string[] = [];
  let depth = 0;
  let start = -1;
  
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '{') {
      if (depth === 0) {
        start = i;
      }
      depth++;
    } else if (text[i] === '}') {
      if (depth > 0) {
        depth--;
        if (depth === 0 && start !== -1) {
          blocks.push(text.substring(start, i + 1));
          start = -1;
        }
      }
    }
  }
  return blocks;
}

function keysToCamelCase(obj: Record<string, any>): Record<string, any> {
  return Object.keys(obj).reduce((acc, key) => {
    const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
    acc[camelKey] = obj[key];
    return acc;
  }, {} as Record<string, any>);
}

let cachePromise: Promise<[any[], any[]]> | null = null;
let cacheTime = 0;
const CACHE_DURATION = 30000; // 30 seconds

async function fetchConnectionsAndHosts(): Promise<[any[], any[]]> {
  const now = Date.now();
  if (cachePromise && now - cacheTime < CACHE_DURATION) {
    return cachePromise;
  }

  cacheTime = now;
  cachePromise = Promise.all([
    invoke<any[]>("load_connections"),
    invoke<any[]>("get_mcp_hosts").catch(() => [])
  ]).catch((err) => {
    cachePromise = null;
    cacheTime = 0;
    throw err;
  });

  return cachePromise;
}

async function resolveDeviceFromConnections(userMessage: string): Promise<string | undefined> {
  try {
    const [connections, mcpHosts] = await fetchConnectionsAndHosts();
    const lowerMessage = userMessage.toLowerCase();
    const matchCondition = (c: any) =>
      (c.hostname && lowerMessage.includes(c.hostname.toLowerCase())) ||
      (c.ip && lowerMessage.includes(c.ip));

    const matched = connections?.find(matchCondition) || mcpHosts?.find(matchCondition);
    if (matched) {
      console.log("[useMcp] Auto-extracted device name from user message:", matched.hostname);
      return matched.hostname;
    }
  } catch (err) {
    console.error("[useMcp] Failed to resolve connections for auto-extraction:", err);
  }
  return undefined;
}


export async function normalizeArgs(toolId: string, userMessage: string, args: any, recentIPs?: string[]): Promise<any> {
  if (!args || typeof args !== "object" || Array.isArray(args)) {
    return args;
  }

  const processedArgs = keysToCamelCase(args);

  if (["fetch_config", "fetch_routing", "fetch_arp"].includes(toolId)) {
    let deviceVal = processedArgs.deviceName || processedArgs.device_name || processedArgs.device || processedArgs.host;
    if (!deviceVal) {
      deviceVal = await resolveDeviceFromConnections(userMessage);
    }
    
    // Fallback to session recent host if not found in args or message
    if (!deviceVal && recentIPs?.[0]) {
      deviceVal = recentIPs[0];
      console.log("[useMcp] Omitted device name, fallback to session's recent host:", deviceVal);
    }

    if (deviceVal) {
      processedArgs.deviceName = deviceVal;
    }
  } else if (["self_network_ping", "self_network_traceroute"].includes(toolId)) {
    let hostVal = processedArgs.host || processedArgs.device || processedArgs.deviceName || processedArgs.device_name || processedArgs.ip;
    
    // Fallback to session recent host if not found in args or message
    if (!hostVal && recentIPs?.[0]) {
      hostVal = recentIPs[0];
      console.log("[useMcp] Omitted host, fallback to session's recent host:", hostVal);
    }

    if (hostVal) {
      processedArgs.host = hostVal;
    }
  }

  return processedArgs;
}
