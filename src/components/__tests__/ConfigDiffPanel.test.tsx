import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ConfigDiffPanel } from "../ConfigDiffPanel/ConfigDiffPanel";
import { UIProvider, useUIContext } from "../../contexts/UIContext";
import React, { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

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
  hostname: "router-1",
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

  it("creates, approves, and executes a hash-bound operation plan", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ id: "plan-1", planHash: "hash-1", approvalStatus: "pending" })
      .mockResolvedValueOnce({ id: "plan-1", planHash: "hash-1", approvalStatus: "approved" })
      .mockResolvedValueOnce({ success: true, output: "applied" })
      .mockResolvedValueOnce(undefined);
    render(
      <UIProvider>
        <TestWrapper>
          <ConfigDiffPanel {...defaultProps} />
        </TestWrapper>
      </UIProvider>
    );

    fireEvent.click(screen.getByText("承認して実行"));

    await waitFor(() => {
      expect(invoke).toHaveBeenNthCalledWith(1, "create_network_config_operation_plan", {
        deviceName: "router-1",
        commands: ["hostname Router"],
        rationale: "画面に表示した 1 行の設定差分を router-1 に適用する",
      });
      expect(invoke).toHaveBeenNthCalledWith(2, "approve_operation_plan", {
        id: "plan-1",
        planHash: "hash-1",
      });
      expect(invoke).toHaveBeenNthCalledWith(3, "execute_approved_operation_plan", {
        id: "plan-1",
        planHash: "hash-1",
      });
    });
  });
});
