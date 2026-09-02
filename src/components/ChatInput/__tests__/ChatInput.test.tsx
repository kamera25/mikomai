import { describe, it, expect, vi } from "vitest";
import { render as rtlRender, screen, fireEvent } from "@testing-library/react";
import { ChatInput } from "../ChatInput";
import { SettingsProvider } from "../../../contexts/SettingsContext";
import { invoke } from "@tauri-apps/api/core";

vi.mock("../../../hooks/useSettings", () => ({
  useSettings: () => ({
    visionEnabled: false,
    mmprojPath: null,
  }),
}));

const render = (ui: React.ReactElement) => {
  const result = rtlRender(<SettingsProvider>{ui}</SettingsProvider>);
  return {
    ...result,
    rerender: (newUi: React.ReactElement) =>
      result.rerender(<SettingsProvider>{newUi}</SettingsProvider>),
  };
};

describe("ChatInput Component", () => {
  const defaultProps = {
    modelStatus: "Loaded",
    modelPath: "/path/to/model.gguf",
    input: "",
    setInput: vi.fn(),
    showSuggestions: false,
    setShowSuggestions: vi.fn(),
    filteredSuggestions: [],
    suggestionIndex: 0,
    setSuggestionIndex: vi.fn(),
    handleSelectSuggestion: vi.fn(),
    handleSend: vi.fn(),
    handleStop: vi.fn(),
    isGenerating: false,
    handleLoadModel: vi.fn(),
    setIsSettingsOpen: vi.fn(),
    cursorPos: 0,
    setCursorPos: vi.fn(),
    availableHosts: [],
    recentIPs: [],
    setFilteredSuggestions: vi.fn(),
  };

  it("renders input field when model is loaded", () => {
    render(<ChatInput {...defaultProps} />);
    expect(screen.getByPlaceholderText("mikomaiに質問する...")).toBeInTheDocument();
  });

  it("renders banner when model is not loaded", () => {
    render(<ChatInput {...defaultProps} modelStatus="NotLoaded" />);
    expect(
      screen.getByText("AIモデルが読み込まれていません。モデルを読み込んでください。")
    ).toBeInTheDocument();
  });

  it("calls handleSend and clears input when send button is clicked", () => {
    const handleSend = vi.fn();
    const setInput = vi.fn();
    render(<ChatInput {...defaultProps} input="hello" handleSend={handleSend} setInput={setInput} />);
    const button = screen.getByTitle("送信");
    fireEvent.click(button);
    expect(handleSend).toHaveBeenCalledWith("hello", []);
    expect(setInput).toHaveBeenCalledWith("");
  });

  it("allows queueing a message when the model is not loaded", () => {
    const handleSend = vi.fn();
    render(<ChatInput {...defaultProps} modelStatus="NotLoaded" input="hello" handleSend={handleSend} />);
    const button = screen.getByTitle("送信予約");
    expect(button).not.toBeDisabled();
    fireEvent.click(button);
    expect(handleSend).toHaveBeenCalledWith("hello", []);
  });

  it("renders stop button when isGenerating is true and calls handleStop on click", () => {
    const handleStop = vi.fn();
    render(<ChatInput {...defaultProps} isGenerating={true} handleStop={handleStop} />);
    const stopButton = screen.getByTitle("応答を停止");
    expect(stopButton).toBeInTheDocument();
    fireEvent.click(stopButton);
    expect(handleStop).toHaveBeenCalled();
  });

  it("updates filtered suggestions when availableHosts changes while suggestions are shown", () => {
    const setFilteredSuggestions = vi.fn();
    const { rerender } = render(
      <ChatInput
        {...defaultProps}
        input="@"
        cursorPos={1}
        showSuggestions={true}
        availableHosts={[]}
        setFilteredSuggestions={setFilteredSuggestions}
      />
    );

    expect(setFilteredSuggestions).toHaveBeenCalledWith([
      { hostname: "localhost", ip: "このコンピュータ" }
    ]);

    setFilteredSuggestions.mockClear();

    rerender(
      <ChatInput
        {...defaultProps}
        input="@"
        cursorPos={1}
        showSuggestions={true}
        availableHosts={[{ hostname: "router-new", ip: "10.0.0.5" }]}
        setFilteredSuggestions={setFilteredSuggestions}
      />
    );

    expect(setFilteredSuggestions).toHaveBeenCalledWith([
      { hostname: "localhost", ip: "このコンピュータ" },
      { hostname: "router-new", ip: "10.0.0.5" }
    ]);
  });

  it("closes suggestions when the suggestions list becomes empty", () => {
    const setFilteredSuggestions = vi.fn();
    const setShowSuggestions = vi.fn();
    const { rerender } = render(
      <ChatInput
        {...defaultProps}
        input="@router"
        cursorPos={7}
        showSuggestions={true}
        availableHosts={[{ hostname: "router-1", ip: "10.0.0.1" }]}
        setFilteredSuggestions={setFilteredSuggestions}
        setShowSuggestions={setShowSuggestions}
      />
    );

    expect(setFilteredSuggestions).toHaveBeenCalledWith([
      { hostname: "router-1", ip: "10.0.0.1" }
    ]);
    expect(setShowSuggestions).not.toHaveBeenCalled();

    setFilteredSuggestions.mockClear();
    setShowSuggestions.mockClear();

    rerender(
      <ChatInput
        {...defaultProps}
        input="@router"
        cursorPos={7}
        showSuggestions={true}
        availableHosts={[]}
        setFilteredSuggestions={setFilteredSuggestions}
        setShowSuggestions={setShowSuggestions}
      />
    );

    expect(setFilteredSuggestions).toHaveBeenCalledWith([]);
    expect(setShowSuggestions).toHaveBeenCalledWith(false);
  });

  it("shows a backend attachment rejection to the user", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      attachments: [],
      rejected: [{ name: "diagram.png", reason: "画像添付には Vision モデルの設定が必要です" }],
    });
    const { container } = render(<ChatInput {...defaultProps} />);
    const fileInput = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(["image"], "diagram.png", { type: "image/png" });

    fireEvent.change(fileInput, { target: { files: [file] } });

    expect(await screen.findByRole("alert")).toHaveTextContent("Vision モデルの設定が必要です");
    expect(invoke).toHaveBeenCalledWith("prepare_attachments", expect.any(Object));
  });

  it("closes host suggestions with Escape without sending the message", () => {
    const setShowSuggestions = vi.fn();
    const handleSend = vi.fn();
    render(
      <ChatInput
        {...defaultProps}
        input="@router"
        cursorPos={7}
        showSuggestions={true}
        filteredSuggestions={[{ hostname: "router-1", ip: "10.0.0.1" }]}
        setShowSuggestions={setShowSuggestions}
        handleSend={handleSend}
      />
    );
    fireEvent.keyDown(screen.getByPlaceholderText("mikomaiに質問する..."), { key: "Escape" });
    expect(setShowSuggestions).toHaveBeenCalledWith(false);
    expect(handleSend).not.toHaveBeenCalled();
  });
});
