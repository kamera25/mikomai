import { useEffect } from "react";
import { ipc, type EventEnvelope } from "../../platform";
import { chatReducer, initialChatReducerState, type ChatReducerState } from "./chatReducer";

export type ChatEventConsumer = (event: EventEnvelope) => void;

export function useChatEvents(consume: ChatEventConsumer, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void ipc.subscribeChat((event) => { if (!cancelled) consume(event as EventEnvelope); }).then((cleanup) => {
      if (cancelled) cleanup(); else unlisten = cleanup;
    }).catch((error: unknown) => {
      if (!cancelled) console.error("Failed to subscribe to chat events:", error);
    });
    return () => { cancelled = true; unlisten?.(); };
  }, [consume, enabled]);
}

export { chatReducer, initialChatReducerState };
export type { ChatReducerState };
