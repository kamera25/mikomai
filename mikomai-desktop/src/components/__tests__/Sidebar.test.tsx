import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar/Sidebar";
import { Message } from "../../types";
import { GuiEventScope } from "../../gui/events";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe("Sidebar", () => {
  it("bubbles session selection to the mediator", () => {
    const handle = vi.fn(() => true);
    render(
      <GuiEventScope handle={handle}>
        <Sidebar
          isSidebarOpen
          history={[{ id: "s1", title: "Session 1", type: "session", messages: [] }]}
          activeSessionId=""
          messages={[]}
        />
      </GuiEventScope>
    );
    fireEvent.click(screen.getByText("Session 1"));
    expect(handle).toHaveBeenCalledWith({ type: "session.select", id: "s1" });
  });
  it("renders 'エージェントによる解析を開始' for agent-step messages when summary_text is not provided", () => {
    const messages: Message[] = [
      {
        role: "ai",
        event_type: "AgentResponse",
        content:
          "```agent-step\nphase: planning\nstep: 1\n```\n```agent-decision\nstep: 1\naction: FINISH\n```",
      },
    ];

    render(
      <GuiEventScope handle={() => true}>
        <Sidebar
          isSidebarOpen={true}
          history={[{ id: "s1", title: "Session 1", type: "session", messages: [] }]}
          activeSessionId="s1"
          messages={messages}
        />
      </GuiEventScope>
    );

    expect(screen.getByText("エージェントによる解析を開始")).toBeInTheDocument();
  });
});
