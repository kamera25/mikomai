import type { ChatEvent } from "./ipc";

/** Normalize the wire event once, before feature reducers consume it. */
export type EventEnvelope = ChatEvent;

type SequencedEvent = { taskId?: string; type: string; sequence?: number };

export function acceptEvent(previous: SequencedEvent | undefined, incoming: SequencedEvent): boolean {
  if (!previous) return true;
  if (previous.taskId !== incoming.taskId) return true;
  if (previous.sequence === undefined || incoming.sequence === undefined) return true;
  return incoming.sequence > previous.sequence;
}
