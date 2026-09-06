import { ipc } from "../../platform";

export const operationService = {
  createPlan: <T>(args: Record<string, unknown>) => ipc.command<T>("create_network_config_operation_plan", args),
  approve: <T>(id: string, planHash: string) => ipc.command<T>("approve_operation_plan", { id, planHash }),
  execute: <T>(id: string, planHash: string) => ipc.command<T>("execute_approved_operation_plan", { id, planHash }),
};
