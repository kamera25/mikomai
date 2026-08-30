import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ContextMenu } from "../ContextMenu";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        "context_menu.cut": "切り取り",
        "context_menu.copy": "コピー",
        "context_menu.paste": "貼り付け",
        "context_menu.select_all": "すべて選択",
        "context_menu.reload": "再読み込み",
      };
      return translations[key] || key;
    },
  }),
}));

describe("ContextMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not render when closed", () => {
    render(<ContextMenu />);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("opens on contextmenu event and displays Japanese menu items", () => {
    render(
      <div>
        <div data-testid="target">Test area</div>
        <ContextMenu />
      </div>
    );

    const target = screen.getByTestId("target");
    fireEvent.contextMenu(target, { clientX: 100, clientY: 150 });

    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByText("切り取り")).toBeInTheDocument();
    expect(screen.getByText("コピー")).toBeInTheDocument();
    expect(screen.getByText("貼り付け")).toBeInTheDocument();
    expect(screen.getByText("すべて選択")).toBeInTheDocument();
    expect(screen.getByText("再読み込み")).toBeInTheDocument();
  });

  it("closes when Escape key is pressed", () => {
    render(
      <div>
        <div data-testid="target">Test area</div>
        <ContextMenu />
      </div>
    );

    const target = screen.getByTestId("target");
    fireEvent.contextMenu(target, { clientX: 100, clientY: 150 });
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes when clicking outside", () => {
    render(
      <div>
        <div data-testid="target">Test area</div>
        <div data-testid="outside">Outside area</div>
        <ContextMenu />
      </div>
    );

    const target = screen.getByTestId("target");
    fireEvent.contextMenu(target, { clientX: 100, clientY: 150 });
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("enables cut and paste on editable text fields", () => {
    render(
      <div>
        <input data-testid="test-input" defaultValue="Sample Text" />
        <ContextMenu />
      </div>
    );

    const input = screen.getByTestId("test-input") as HTMLInputElement;
    input.focus();
    input.setSelectionRange(0, 6);

    fireEvent.contextMenu(input, { clientX: 100, clientY: 150 });

    const cutButton = screen.getByText("切り取り").closest("button");
    const copyButton = screen.getByText("コピー").closest("button");
    const pasteButton = screen.getByText("貼り付け").closest("button");

    expect(cutButton).not.toBeDisabled();
    expect(copyButton).not.toBeDisabled();
    expect(pasteButton).not.toBeDisabled();
  });
});
