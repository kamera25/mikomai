import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { GuiEventScope, useGuiEvent, type GuiEvent } from "./events";

function Emitter() {
  const emit = useGuiEvent();
  return <button onClick={() => emit({ type: "session.create" })}>Create</button>;
}

describe("GUI event chain", () => {
  it("bubbles an unhandled child event to Root", () => {
    const child = vi.fn(() => false);
    const root = vi.fn(() => true);
    render(
      <GuiEventScope handle={root}>
        <GuiEventScope handle={child}>
          <Emitter />
        </GuiEventScope>
      </GuiEventScope>
    );
    fireEvent.click(screen.getByText("Create"));
    expect(child).toHaveBeenCalledWith({ type: "session.create" });
    expect(root).toHaveBeenCalledWith({ type: "session.create" });
  });

  it("stops when a child handles the event", () => {
    const child = vi.fn((_event: GuiEvent) => true);
    const root = vi.fn(() => true);
    render(
      <GuiEventScope handle={root}>
        <GuiEventScope handle={child}>
          <Emitter />
        </GuiEventScope>
      </GuiEventScope>
    );
    fireEvent.click(screen.getByText("Create"));
    expect(root).not.toHaveBeenCalled();
  });
});
