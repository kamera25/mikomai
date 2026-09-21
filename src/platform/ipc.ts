import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { COMMANDS, EVENTS, type CommandName, type EventName } from "./commands";
import type { SummaryItem } from "../types";
import type { TaskSnapshot } from "./dto";

export type { TaskSnapshot, TaskStatus } from "./dto";
// Wire payload is validated by feature reducers; keep transport generic here.
export type ChatEvent =
  | { type: "mcpToolStarted"; taskId?: string; payload: { taskId: string; toolId: string; args?: Record<string, unknown>; resolvedHost?: string }; sequence?: number; version?: number }
  | { type: "mcpToolFinished"; taskId?: string; payload: { taskId: string; success: boolean; output?: string; savedPath?: string; isCached?: boolean; cacheTime?: string }; sequence?: number; version?: number }
  | { type: "mcpInitialStarted"; taskId?: string; payload: { taskId: string; hasImage?: boolean }; sequence?: number; version?: number }
  | { type: "mcpAnalysisStarted"; taskId?: string; payload: { taskId: string; analysisTaskId: string }; sequence?: number; version?: number }
  | { type: "mcpInitialFinished"; taskId?: string; payload: { taskId: string; content: string }; sequence?: number; version?: number }
  | { type: "mcpSummarySaved"; taskId?: string; payload: { taskId: string; content: string; summaryText?: string; summary?: SummaryItem }; sequence?: number; version?: number }
  | { type: "arpYamlSaved"; taskId?: string; payload: { deviceName?: string; savedPath?: string }; sequence?: number; version?: number }
  | { type: "routeYamlSaved"; taskId?: string; payload: { deviceName?: string; savedPath?: string }; sequence?: number; version?: number }
  | { type: "llmChunk" | "agentSelected"; taskId?: string; payload: string; sequence?: number; version?: number };

function listenWithSafeCleanup<T>(
  name: string,
  handler: (event: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(name, ({ payload }) => handler(payload)).then((unlisten) => {
    let disposed = false;
    return async () => {
      if (disposed) return;
      disposed = true;
      try {
        await unlisten();
      } catch (error) {
        // React StrictMode can tear down an effect while Tauri is still
        // registering its listener. Cleanup must remain best-effort so a
        // stale listener cannot blank the application during startup.
        console.debug(`Ignoring stale Tauri listener cleanup for ${name}`, error);
      }
    };
  });
}

export const ipc = {
  command: <T>(name: CommandName, args?: Record<string, unknown>) => args === undefined ? invoke<T>(name) : invoke<T>(name, args),
  sendMcp: (payload: unknown) => invoke<void>(COMMANDS.chat, { payload }),
  startTask: (goal: string) => invoke<TaskSnapshot>(COMMANDS.startTask, { goal }),
  resumeTask: (taskId: string) => invoke<TaskSnapshot | null>(COMMANDS.resumeTask, { taskId }),
  submitChoice: (id: string, choice: string) => invoke<void>(COMMANDS.submitUserChoice, { id, choice }),
  submitInterfaceChoice: (id: string, choice: string) => invoke<void>(COMMANDS.submitInterfaceChoice, { id, choice }),
  submitIpAddressChoice: (id: string, choice: string) => invoke<void>(COMMANDS.submitIpAddressChoice, { id, choice }),
  prepareAttachments: <T>(sources: unknown) => invoke<T>(COMMANDS.prepareAttachments, { sources }),
  openModelDir: (modelPath: string | null) => invoke<void>(COMMANDS.openModelDir, { modelPath }),
  openPathInFileManager: (path: string) => invoke<void>(COMMANDS.openPathInFileManager, { path }),
  copyFileToDestination: (srcPath: string, destPath: string) => invoke<void>(COMMANDS.copyFileToDestination, { srcPath, destPath }),
  subscribeChat: (handler: (event: ChatEvent) => void): Promise<UnlistenFn> =>
    listenWithSafeCleanup(EVENTS.chat, handler),
  subscribe: <T>(name: EventName, handler: (event: T) => void): Promise<UnlistenFn> =>
    listenWithSafeCleanup(name, handler),
};
