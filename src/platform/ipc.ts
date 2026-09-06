import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { COMMANDS } from "./commands";

export type TaskStatus = "pending" | "running" | "awaiting_approval" | "completed" | "failed" | "unknown";
export type TaskSnapshot = { taskId: string; goal: string; status: TaskStatus };
// Wire payload is validated by feature reducers; keep transport generic here.
export type ChatEvent = { type: string; taskId: string; payload?: any };

export const ipc = {
  command: <T>(name: string, args?: Record<string, unknown>) => invoke<T>(name, args),
  sendMcp: (payload: unknown) => invoke<void>(COMMANDS.chat, { payload }),
  startTask: (goal: string) => invoke<TaskSnapshot>("start_task", { goal }),
  resumeTask: (taskId: string) => invoke<TaskSnapshot | null>("resume_task", { taskId }),
  subscribeChat: (handler: (event: ChatEvent) => void): Promise<UnlistenFn> =>
    listen<ChatEvent>("chat-event", ({ payload }) => handler(payload)),
  subscribe: <T>(name: string, handler: (event: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, ({ payload }) => handler(payload)),
};
