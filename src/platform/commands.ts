/** Backend command names shared by feature facades. */
export const COMMANDS = {
  chat: "handle_mcp_message",
  loadSettings: "load_settings",
  saveSettings: "save_settings",
  loadConnections: "load_connections",
  mcpHosts: "get_mcp_hosts",
  resolveIp: "resolve_ip",
} as const;
