import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { ConfigDiffPanel } from "../ConfigDiffPanel/ConfigDiffPanel";
import { UIProvider, useUIContext } from "../../contexts/UIContext";
import React, { useEffect } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
}));

const mockDiffData = {
  fileName: "cisco.conf",
  additions: 1,
  deletions: 0,
  diffLines: [{ type: "insert" as const, oldLine: null, newLine: 1, content: "hostname Router" }],
};

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { dispatch } = useUIContext();
  useEffect(() => {
    dispatch({ type: "SET_CONFIG_DIFF_DATA", payload: mockDiffData });
  }, [dispatch]);
  return <>{children}</>;
};

describe("ConfigDiffPanel", () => {
  const defaultProps = {
    id: "test-id",
    isOpen: true,
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
  });

  it("renders diff panel when open with diff data", () => {
    render(
      <UIProvider>
        <TestWrapper>
          <ConfigDiffPanel {...defaultProps} />
        </TestWrapper>
      </UIProvider>
    );

    expect(screen.getByText("変更箇所 (Diff)")).toBeDefined();
    expect(screen.getByText("投入ログ (0)")).toBeDefined();
  });

  it("switches to logs tab when clicked", () => {
    render(
      <UIProvider>
        <TestWrapper>
          <ConfigDiffPanel {...defaultProps} />
        </TestWrapper>
      </UIProvider>
    );

    const logTab = screen.getByText("投入ログ (0)");
    fireEvent.click(logTab);

    expect(screen.getByText("投入ログがここにリアルタイム表示されます...")).toBeDefined();
  });
});
