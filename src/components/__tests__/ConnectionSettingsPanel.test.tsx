import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ConnectionSettingsPanel } from "../ConnectionSettingsPanel/ConnectionSettingsPanel.tsx";
import * as tauriApi from "@tauri-apps/api/core";
import * as tauriDialog from "@tauri-apps/plugin-dialog";

// Mock Tauri invoke and dialog functions
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: vi.fn(),
}));

describe("ConnectionSettingsPanel", () => {
  const defaultProps = {
    onClose: vi.fn(),
    onConnectionsChanged: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    // Default mocks
    vi.mocked(tauriApi.invoke).mockImplementation(async (cmd, _args) => {
      if (cmd === "get_device_types") {
        return {
          deviceTypes: ["cisco_ios"],
          deviceTypeAliases: { cisco_ios: "Cisco IOS" },
        };
      }
      if (cmd === "get_mcp_hosts") {
        return [
          {
            hostname: "Mcp-Host-01",
            ip: "192.168.10.10",
            deviceType: "cisco_ios",
            username: "admin",
          },
        ];
      }
      if (cmd === "load_connections") {
        return [];
      }
      if (cmd === "save_connections") {
        return null;
      }
      return null;
    });
  });

  it("renders correctly", async () => {
    render(<ConnectionSettingsPanel {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("接続設定")).toBeInTheDocument();
      expect(screen.getByText("接続が登録されていません。")).toBeInTheDocument();
    });
  });

  it("starts a bulk Node DB refresh in the background", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation(async (cmd, _args) => {
      if (cmd === "get_device_types") return { deviceTypes: [], deviceTypeAliases: {} };
      if (cmd === "get_mcp_hosts") return [];
      if (cmd === "load_connections") {
        return [{ id: "1", hostname: "Router-01", ip: "192.0.2.1", type: "SSH", status: "offline" }];
      }
      if (cmd === "start_node_db_bulk_refresh") return { nodeCount: 1 };
      return null;
    });
    render(<ConnectionSettingsPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("ノードDBへ一括取得")).toBeEnabled();
    });
    fireEvent.click(screen.getByText("ノードDBへ一括取得"));

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith("start_node_db_bulk_refresh");
      expect(tauriDialog.message).toHaveBeenCalledWith(
        expect.stringContaining("1台のノード情報の一括取得")
      );
    });
  });

  it("renders username, password, and enable password input fields when Console type is selected", async () => {
    render(<ConnectionSettingsPanel {...defaultProps} />);

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith("get_mcp_hosts");
    });

    const addBtn = screen.getByText("ホスト追加");
    fireEvent.click(addBtn);

    // Change type to Console
    const typeSelect = screen.getByDisplayValue("SSH");
    fireEvent.change(typeSelect, { target: { value: "Console" } });

    expect(screen.getByText(/ユーザ名|ユーザーID|Username/i)).toBeInTheDocument();
    expect(screen.getAllByText(/パスワード|Password/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/特権パスワード|Enable Password/i)).toBeInTheDocument();
  });
});
