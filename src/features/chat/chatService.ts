import { ipc, type ChatEvent, type TaskSnapshot } from "../../platform";

export interface ChatService {
  send(payload: unknown): Promise<void>;
  resume(taskId: string): Promise<TaskSnapshot | null>;
  subscribe(handler: (event: ChatEvent) => void): Promise<() => void>;
}

/** Feature-facing facade; transport details remain in platform/ipc. */
export const chatService: ChatService = {
  send: ipc.sendMcp,
  resume: ipc.resumeTask,
  subscribe: ipc.subscribeChat,
};
