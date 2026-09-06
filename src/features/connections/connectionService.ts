import { ipc, COMMANDS } from "../../platform";
import type { Connection, McpHost } from "../../types";
export const connectionService = {
  load: () => Promise.all([ipc.command<Connection[]>(COMMANDS.loadConnections), ipc.command<McpHost[]>(COMMANDS.mcpHosts)]),
  resolveIp: (ip: string) => ipc.command<string>(COMMANDS.resolveIp, { ip }),
};
