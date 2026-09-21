import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { UIState } from "../contexts/UIContext";

export type Panel = UIState["activePanel"];

export type GuiEvent =
  | { type: "navigate"; panel: Panel }
  | { type: "sidebar.toggle" }
  | { type: "diff.toggle" }
  | { type: "header.edit" }
  | { type: "header.change"; title: string }
  | { type: "header.save" }
  | { type: "header.cancel" }
  | { type: "session.create" }
  | { type: "session.select"; id: string }
  | { type: "session.folder.toggle"; id: string }
  | { type: "session.rename"; id: string; title: string }
  | { type: "session.delete"; id: string }
  | { type: "timeline.scroll"; taskId: string }
  | {
      type: "question.answer";
      kind: "choice" | "interface" | "ipaddress";
      id: string;
      value: string;
    }
  | { type: "question.cancel"; kind: "choice" | "interface" | "ipaddress"; id: string };

export type GuiEventHandler = (event: GuiEvent) => boolean;

// Every scope gets the first opportunity to handle an event. Returning false
// passes it to its parent, ending at the Root mediator.
const GuiEventContext = createContext<GuiEventHandler | null>(null);

export function GuiEventScope({
  children,
  handle,
}: {
  children: ReactNode;
  handle: GuiEventHandler;
}) {
  const parent = useContext(GuiEventContext);
  const bubble = useMemo<GuiEventHandler>(
    () => (event) => handle(event) || parent?.(event) || false,
    [handle, parent]
  );
  return <GuiEventContext.Provider value={bubble}>{children}</GuiEventContext.Provider>;
}

export function useGuiEvent(): GuiEventHandler {
  const emit = useContext(GuiEventContext);
  if (!emit) throw new Error("GUI components must be rendered below Root's event mediator");
  return emit;
}
