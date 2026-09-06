import { ipc, acceptEvent, type EventEnvelope } from "../../platform";
export const chatFeature = { send: ipc.startTask, resume: ipc.resumeTask, subscribe: ipc.subscribeChat };
export { Chat } from "../../components/Chat/Chat";
export { chatService } from "./chatService";
export type { ChatService } from "./chatService";
export const chatEventPolicy = { accept: acceptEvent };
export type { EventEnvelope };
