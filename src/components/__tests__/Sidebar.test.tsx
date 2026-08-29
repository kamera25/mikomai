import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Sidebar } from "../Sidebar/Sidebar";
import { Message } from "../../types";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe("Sidebar", () => {
  it("renders 'エージェントによる解析を開始' for agent-step messages when summary_text is not provided", () => {
    const messages: Message[] = [
      {
        role: "ai",
        event_type: "AgentResponse",
        content: "```agent-step\nphase: planning\nstep: 1\n```\n```agent-decision\nstep: 1\naction: FINISH\n```",
      },
    ];

    render(
      <Sidebar
        isSidebarOpen={true}
        history={[{ id: "s1", title: "Session 1", type: "session" }]}
        activeSessionId="s1"
        messages={messages}
        createNewFolder={vi.fn()}
        createNewSession={vi.fn()}
        toggleFolder={vi.fn()}
        switchSession={vi.fn()}
        renameSession={vi.fn()}
        deleteSession={vi.fn()}
      />
    );

    expect(screen.getByText("エージェントによる解析を開始")).toBeInTheDocument();
  });
});
