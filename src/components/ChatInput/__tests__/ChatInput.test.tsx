import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ChatInput } from "../ChatInput";

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

  it("calls handleSend when send button is clicked", () => {
    const handleSend = vi.fn();
    render(<ChatInput {...defaultProps} input="hello" handleSend={handleSend} />);
    const button = screen.getByRole("button");
    fireEvent.click(button);
    expect(handleSend).toHaveBeenCalled();
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
});
