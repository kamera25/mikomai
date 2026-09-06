import type { ChatEvent } from "./ipc";

/** Normalize the wire event once, before feature reducers consume it. */
export type EventEnvelope = ChatEvent & { sequence?: number; version?: number };

export function acceptEvent(previous: EventEnvelope | undefined, incoming: EventEnvelope): boolean {
  if (previous?.taskId !== incoming.taskId) return true;
  if (previous.sequence === undefined || incoming.sequence === undefined) return true;
  return incoming.sequence > previous.sequence;
}

