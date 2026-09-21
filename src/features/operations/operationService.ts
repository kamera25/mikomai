import { ipc, COMMANDS } from "../../platform";

export const operationService = {
  createPlan: <T>(args: Record<string, unknown>) => ipc.command<T>(COMMANDS.createNetworkConfigOperationPlan, args),
  approve: <T>(id: string, planHash: string) => ipc.command<T>(COMMANDS.approveOperationPlan, { id, planHash }),
  execute: <T>(id: string, planHash: string) => ipc.command<T>(COMMANDS.executeOperationPlan, { id, planHash }),
};
