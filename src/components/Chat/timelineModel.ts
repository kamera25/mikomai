import { Message } from "../../types";

export function isNetworkDatabaseTool(toolId?: string): boolean {
  return toolId === "query_nw_db" || toolId === "network_query_nw_db";
}

export function isChoiceTool(toolId?: string): boolean {
  return toolId === "ask_user_choice" || toolId === "ask_interface_choice" || toolId === "ask_ipaddress_choice";
}

export function messageContainerClass(msg: Message): string {
  return ["message-container", msg.role, msg.event_type?.toLowerCase(), msg.status?.toLowerCase()]
    .filter(Boolean)
    .join(" ");
}

export function defaultFilename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || "downloaded_file.txt";
}
