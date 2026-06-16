import { SummaryItem, Connection, McpHost } from "../../types";
import { invoke } from "@tauri-apps/api/core";

export function getHistoryBlock(items: SummaryItem[], limit: number): string {
  if (limit <= 0 || items.length === 0) return "";
  const recent = [...items].reverse().slice(0, limit);
  const text = recent.map((s, i) => `${i + 1}. ${s.content}`).join("\n");
  return `\n\n<memory>\n${text}\n</memory>`;
}

const TOOL_LABEL_MAP: Record<string, string> = {
  self_network_ping: "Ping",
  self_network_traceroute: "Traceroute",
  network_get_hosts: "Host List",
  network_query_nw_db: "NWDB検索",
  query_nw_db: "NWDB検索",
  self_network_arp: "ARP Table",
  self_network_route: "Route Table",
  network_get_ip_info: "IP Info",
  network_list_serial_ports: "Serial Ports",
  network_send_console_message: "Console Message",
  network_show: "Show Command",
  fetch_config: "Fetch Config",
  fetch_routing: "Fetch Routing",
  fetch_arp: "Fetch ARP",
  require_host_registered: "ホスト登録要求",
};

export function getToolLabel(toolName: string): string {
  return TOOL_LABEL_MAP[toolName] || toolName;
}

export function extractJsonBlocks(text: string): string[] {
  const blocks: string[] = [];
  let i = 0;
  while (i < text.length) {
    if (text[i] === "{") {
      let success = false;
      for (let j = text.length - 1; j > i; j--) {
        if (text[j] === "}") {
          const candidate = text.substring(i, j + 1);
          try {
            JSON.parse(candidate);
            blocks.push(candidate);
            i = j;
            success = true;
            break;
          } catch (e) {
            // Not a valid JSON block, continue searching
          }
        }
      }
      if (success) {
        i++;
        continue;
      }
    }
    i++;
  }
  return blocks;
}

export function keysToCamelCase(obj: Record<string, unknown>): Record<string, unknown> {
  return Object.keys(obj).reduce(
    (acc, key) => {
      const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
      acc[camelKey] = obj[key];
      return acc;
    },
    {} as Record<string, unknown>
  );
}

export async function resolveDeviceNameStep(
  processedArgs: Record<string, any>,
  userMessage?: string
): Promise<string | undefined> {
  let deviceVal =
    processedArgs.deviceName ||
    processedArgs.device_name ||
    processedArgs.device ||
    processedArgs.host;
  if (!deviceVal && userMessage) {
    deviceVal = await resolveDeviceFromConnections(userMessage);
  }
  return deviceVal;
}

export function resolveHostStep(
  processedArgs: Record<string, any>
): string | undefined {
  return (
    processedArgs.host ||
    processedArgs.device ||
    processedArgs.deviceName ||
    processedArgs.device_name ||
    processedArgs.ip
  );
}

export function applyFallbackHostStep(
  val: string | undefined,
  recentIPs?: string[]
): string | undefined {
  if (!val && recentIPs?.[0]) {
    console.log("[useMcp] Omitted host, fallback to session's recent host:", recentIPs[0]);
    return recentIPs[0];
  }
  return val;
}

let cachePromise: Promise<[Connection[], McpHost[]]> | null = null;
let cacheTime = 0;
const CACHE_DURATION = 30000; // 30 seconds

async function fetchConnectionsAndHosts(): Promise<[Connection[], McpHost[]]> {
  const now = Date.now();
  if (cachePromise && now - cacheTime < CACHE_DURATION) {
    return cachePromise;
  }

  cacheTime = now;
  cachePromise = Promise.all([
    invoke<Connection[]>("load_connections"),
    invoke<McpHost[]>("get_mcp_hosts").catch(() => [] as McpHost[]),
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
    const matchCondition = (c: Connection | McpHost) =>
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

export async function normalizeArgs(
  _toolId: string,
  userMessage: string,
  args: Record<string, unknown> | null | undefined,
  _recentIPs?: string[]
): Promise<Record<string, unknown> | null | undefined> {
  if (!args || typeof args !== "object" || Array.isArray(args)) {
    return args;
  }

  const processedArgs = keysToCamelCase(args) as Record<string, any>;
  processedArgs.userMessage = userMessage;
  processedArgs.user_message = userMessage;

  return processedArgs;
}

