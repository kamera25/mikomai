import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Chat } from "../Chat";
import { Message } from "../../../types";

describe("Chat Component", () => {
  const formatMessageTime = (_isoString?: string) => "12:00";

  it("renders empty state when there are no messages", () => {
    render(<Chat messages={[]} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("mikomai")).toBeInTheDocument();
  });

  it("renders messages correctly", () => {
    const messages: Message[] = [
      {
        role: "user",
        event_type: "UserInput",
        content: "Hello, robot",
        timestamp: new Date().toISOString(),
      },
      {
        role: "ai",
        event_type: "AgentResponse",
        content: "Hello, user",
        timestamp: new Date().toISOString(),
      },
    ];

    render(<Chat messages={messages} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("Hello, robot")).toBeInTheDocument();
    expect(screen.getByText("Hello, user")).toBeInTheDocument();
  });
});
