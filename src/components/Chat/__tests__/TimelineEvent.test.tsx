import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TimelineEvent } from "../TimelineEvent";
import { Message } from "../../../types";
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

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

  it("renders loading spinner circle when content is thinking", () => {
    const msg: Message = {
      role: "ai",
      content: "考え中...",
      timestamp: new Date().toISOString(),
      event_type: "AgentResponse",
      isToolLoading: true,
    };

    const { container } = render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("考え中...")).toBeInTheDocument();
    expect(container.querySelector(".status-spinner-small")).toBeInTheDocument();
  });

  it("renders loading spinner circle when content is reading image", () => {
    const msg: Message = {
      role: "ai",
      content: "画像の読み取り中…",
      timestamp: new Date().toISOString(),
      event_type: "AgentResponse",
      isToolLoading: true,
    };

    const { container } = render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText("画像の読み取り中…")).toBeInTheDocument();
    expect(container.querySelector(".status-spinner-small")).toBeInTheDocument();
  });

  it("opens image modal when attached image is clicked", () => {
    const msg: Message = {
      role: "user",
      event_type: "UserInput",
      content: "この画像について解説して",
      timestamp: new Date().toISOString(),
      attachments: [
        {
          name: "osprz2a.png",
          type: "image",
          content: "data:image/png;base64,testdata",
        },
      ],
    };

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    const attachedImg = screen.getByAltText("osprz2a.png");
    expect(attachedImg).toBeInTheDocument();

    // Click on image
    fireEvent.click(attachedImg);

    // Modal overlay should be rendered
    expect(screen.getByTestId("image-modal-overlay")).toBeInTheDocument();

    // Close modal
    const closeBtn = screen.getByTestId("image-modal-close-btn");
    fireEvent.click(closeBtn);
    expect(screen.queryByTestId("image-modal-overlay")).not.toBeInTheDocument();
  });

  it("renders open in finder/explorer and fetch file buttons, and calls save dialog on fetch file click", async () => {
    const msg: Message = {
      role: "ai",
      content: "Fetch finished",
      timestamp: new Date().toISOString(),
      event_type: "ToolExecution",
      tool_id: "fetch_config",
      action_name: "Fetch Config",
      summary_text: "Config fetched",
      status: "Success",
      raw_data: "hostname Router1",
      saved_path: "/Users/test/storage/current/Router1_config.txt",
    };

    vi.mocked(save).mockResolvedValue("/Users/test/Downloads/Router1_config.txt" as any);
    vi.mocked(invoke).mockResolvedValue(undefined as any);

    render(
      <TimelineEvent
        msg={msg}
        formatMessageTime={formatMessageTime}
      />
    );

    const openBtn = screen.getByRole("button", { name: /(Finder|Explorer)で開く/i });
    expect(openBtn).toBeInTheDocument();

    const fetchBtn = screen.getByRole("button", { name: /ファイルを取得/i });
    expect(fetchBtn).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(fetchBtn);
    });

    expect(save).toHaveBeenCalledWith({
      defaultPath: "Router1_config.txt",
      title: "ファイルを取得",
    });
    expect(invoke).toHaveBeenCalledWith("copy_file_to_destination", {
      srcPath: "/Users/test/storage/current/Router1_config.txt",
      destPath: "/Users/test/Downloads/Router1_config.txt",
    });
  });
});



