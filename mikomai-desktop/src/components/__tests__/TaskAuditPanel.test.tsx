import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TaskAuditPanel } from "../TaskAuditPanel";
import * as tauriApi from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockHandleLoadModel = vi.fn().mockResolvedValue(undefined);
let mockModelStatus = "Loaded";

vi.mock("../../contexts/ModelContext", () => ({
  useModelContext: () => ({
    state: { modelStatus: mockModelStatus, loadedModelPath: "/path/to/model" },
    handleLoadModel: mockHandleLoadModel,
  }),
}));

describe("TaskAuditPanel", () => {
  const mockTasks = [
    {
      taskId: "task-1",
      startedAt: "2026-09-06T10:00:00Z",
      goal: "F220のVLAN設定の調査",
      lastEventAt: "2026-09-06T10:05:00Z",
      eventCount: 3,
      status: "finished" as const,
    },
    {
      taskId: "task-2",
      startedAt: "2026-09-06T11:00:00Z",
      goal: "スイッチポート状態確認",
      lastEventAt: "2026-09-06T11:02:00Z",
      eventCount: 1,
      status: "stopped" as const,
    },
  ];

  const mockAuditDetail = {
    summary: mockTasks[0],
    events: [
      {
        event_type: "task_started",
        timestamp: "2026-09-06T10:00:00Z",
        goal: "F220のVLAN設定の調査",
      },
      {
        event_type: "action",
        timestamp: "2026-09-06T10:01:00Z",
        tool: "terminal_exec",
        observation: { raw: "show vlan brief" },
      },
      {
        event_type: "finished",
        timestamp: "2026-09-06T10:05:00Z",
      },
    ],
  };

  const defaultProps = {
    onClose: vi.fn(),
    onResume: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockModelStatus = "Loaded";
    mockHandleLoadModel.mockClear();
    vi.mocked(tauriApi.invoke).mockImplementation(async (cmd, _args) => {
      if (cmd === "list_agent_tasks") {
        return mockTasks;
      }
      if (cmd === "get_agent_task_audit") {
        return mockAuditDetail;
      }
      return null;
    });
  });

  it("renders tasks list and header correctly", async () => {
    render(<TaskAuditPanel {...defaultProps} />);

    expect(screen.getByText("エージェント実行履歴")).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("F220のVLAN設定の調査")).toBeInTheDocument();
      expect(screen.getByText("スイッチポート状態確認")).toBeInTheDocument();
      expect(screen.getByText("完了")).toBeInTheDocument();
      expect(screen.getByText("中断")).toBeInTheDocument();
    });
  });

  it("selects a task and displays event details", async () => {
    render(<TaskAuditPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("F220のVLAN設定の調査")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("F220のVLAN設定の調査"));

    await waitFor(() => {
      expect(screen.getByText("この調査を再開")).toBeInTheDocument();
      expect(screen.getByText("実行タイムライン (3 件)")).toBeInTheDocument();
      expect(screen.getByText("タスク開始")).toBeInTheDocument();
      expect(screen.getByText("実行: terminal_exec")).toBeInTheDocument();
      expect(screen.getAllByText("完了").length).toBeGreaterThanOrEqual(1);
    });
  });

  it("triggers onResume callback when resume button is clicked", async () => {
    render(<TaskAuditPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("F220のVLAN設定の調査")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("F220のVLAN設定の調査"));

    await waitFor(() => {
      expect(screen.getByText("この調査を再開")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("この調査を再開"));
    await waitFor(() => {
      expect(defaultProps.onResume).toHaveBeenCalledWith(mockTasks[0]);
    });
  });

  it("loads model first if not loaded when resuming", async () => {
    mockModelStatus = "NotLoaded";
    render(<TaskAuditPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("F220のVLAN設定の調査")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("F220のVLAN設定の調査"));

    await waitFor(() => {
      expect(screen.getByText("この調査を再開")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("この調査を再開"));

    await waitFor(() => {
      expect(mockHandleLoadModel).toHaveBeenCalled();
      expect(defaultProps.onResume).toHaveBeenCalledWith(mockTasks[0]);
    });
  });

  it("triggers onClose callback when close button is clicked", async () => {
    render(<TaskAuditPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("F220のVLAN設定の調査")).toBeInTheDocument();
    });

    const closeBtn = screen.getByLabelText("閉じる");
    fireEvent.click(closeBtn);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });
});
