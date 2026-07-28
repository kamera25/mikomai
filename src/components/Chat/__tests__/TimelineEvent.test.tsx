import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TimelineEvent } from "../TimelineEvent";
import { Message } from "../../../types";


describe("TimelineEvent Component", () => {
  const formatMessageTime = (_isoString?: string) => "12:00";

  it("renders standard user message", () => {
    const msg: Message = {
      role: "user",
      event_type: "UserInput",
      content: "Standard message",
      timestamp: new Date().toISOString(),
    };

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("Standard message")).toBeInTheDocument();
  });

  it("renders tool execution block", () => {
    const msg: Message = {
      role: "ai",
      content: "Tool running...",
      timestamp: new Date().toISOString(),
      event_type: "ToolExecution",
      tool_id: "network_ping",
      action_name: "Ping Test",
      summary_text: "Pinged 8.8.8.8 successfully",
      status: "Success",
      raw_data: "ping ok",
    };

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("Ping Test")).toBeInTheDocument();
    expect(screen.getByText("Pinged 8.8.8.8 successfully")).toBeInTheDocument();
  });

  it("renders suggestion balloon when device retrieval text matches and sends message on click", () => {
    const msg: Message = {
      role: "ai",
      content: "NW-DBには指定されたメーカー・機器（F220）に該当する情報が見つかりません。追加の検索キーワードを指示するか、実機から情報を取得しますか？",
      timestamp: new Date().toISOString(),
      event_type: "AgentResponse",
    };

    const mockSendMessage = vi.fn();

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} sendMessage={mockSendMessage} />);
    
    const balloonButton = screen.getByRole("button", { name: "実機から情報を取得する" });
    expect(balloonButton).toBeInTheDocument();

    fireEvent.click(balloonButton);
    expect(mockSendMessage).toHaveBeenCalledWith("実機から情報を取得してください");
  });
});
