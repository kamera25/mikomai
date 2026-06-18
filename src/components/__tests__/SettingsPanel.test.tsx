import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsPanel } from "../SettingsPanel.tsx";
import * as tauriApi from "@tauri-apps/api/core";
import * as tauriDialog from "@tauri-apps/plugin-dialog";

const mockSaveAllSettings = vi.fn();
const mockSetHistoryLimit = vi.fn();
const mockSetTemperature = vi.fn();
const mockSetRepetitionPenalty = vi.fn();
const mockSetModelPath = vi.fn();
const mockSetMcpTimeout = vi.fn();
const mockSetCacheExpiryMinutes = vi.fn();
const mockSetDbPath = vi.fn();
const mockSetIpVersion = vi.fn();
const mockSetConsolePort = vi.fn();
const mockSetConsoleBaudRate = vi.fn();
const mockSetPreloadInvestigate = vi.fn();
const mockSetPreloadKnowledge = vi.fn();
const mockSetPreloadAnalysis = vi.fn();
const mockSetPreloadRag = vi.fn();

vi.mock("../../contexts/SettingsContext", () => ({
  useSettingsContext: () => ({
    historyLimit: 10,
    setHistoryLimit: mockSetHistoryLimit,
    temperature: 0.7,
    setTemperature: mockSetTemperature,
    repetitionPenalty: 1.1,
    setRepetitionPenalty: mockSetRepetitionPenalty,
    modelPath: "",
    setModelPath: mockSetModelPath,
    mcpTimeout: 30,
    setMcpTimeout: mockSetMcpTimeout,
    cacheExpiryMinutes: 10,
    setCacheExpiryMinutes: mockSetCacheExpiryMinutes,
    dbPath: "",
    setDbPath: mockSetDbPath,
    ipVersion: "auto",
    setIpVersion: mockSetIpVersion,
    consolePort: null,
    setConsolePort: mockSetConsolePort,
    consoleBaudRate: 9600,
    setConsoleBaudRate: mockSetConsoleBaudRate,
    preloadInvestigate: true,
    setPreloadInvestigate: mockSetPreloadInvestigate,
    preloadKnowledge: true,
    setPreloadKnowledge: mockSetPreloadKnowledge,
    preloadAnalysis: true,
    setPreloadAnalysis: mockSetPreloadAnalysis,
    preloadRag: true,
    setPreloadRag: mockSetPreloadRag,
    saveAllSettings: mockSaveAllSettings,
  }),
}));

// Mock Tauri invoke and dialog functions
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("SettingsPanel", () => {
  const defaultProps = {
    isOpen: true,
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauriApi.invoke).mockImplementation((cmd) => {
      if (cmd === "network_list_serial_ports") {
        return Promise.resolve(["COM1", "COM2"]);
      }
      return Promise.reject(new Error("Unknown command"));
    });
  });

  it("renders correctly when isOpen is true", () => {
    render(<SettingsPanel {...defaultProps} />);
    expect(screen.getByText("設定")).toBeInTheDocument();
  });

  it("does not render when isOpen is false", () => {
    render(<SettingsPanel {...defaultProps} isOpen={false} />);
    expect(screen.queryByText("設定")).not.toBeInTheDocument();
  });

  it("calls onClose when close button is clicked", () => {
    render(<SettingsPanel {...defaultProps} />);
    const closeButton = screen.getByTitle("設定を閉じる");
    fireEvent.click(closeButton);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });

  it("handles history limit change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole("slider")[0]; // History limit
    fireEvent.change(slider, { target: { value: "15" } });
    expect(mockSetHistoryLimit).toHaveBeenCalledWith(15);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ historyLimit: 15 });
  });

  it("handles temperature change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole("slider")[1]; // Temperature
    fireEvent.change(slider, { target: { value: "0.8" } });
    expect(mockSetTemperature).toHaveBeenCalledWith(0.8);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ temperature: 0.8 });
  });

  it("handles repetition penalty change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole("slider")[2]; // Repetition Penalty
    fireEvent.change(slider, { target: { value: "1.2" } });
    expect(mockSetRepetitionPenalty).toHaveBeenCalledWith(1.2);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ repetitionPenalty: 1.2 });
  });

  it("handles mcp timeout change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole("slider")[3]; // MCP Timeout
    fireEvent.change(slider, { target: { value: "60" } });
    expect(mockSetMcpTimeout).toHaveBeenCalledWith(60);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ mcpTimeout: 60 });
  });

  it("handles cache expiry minutes change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole("slider")[4]; // Cache Expiry Minutes
    fireEvent.change(slider, { target: { value: "15" } });
    expect(mockSetCacheExpiryMinutes).toHaveBeenCalledWith(15);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ cacheExpiryMinutes: 15 });
  });

  it("handles ip version change", () => {
    render(<SettingsPanel {...defaultProps} />);
    const select = screen.getByLabelText("利用するインターネットプロトコルの指定");
    fireEvent.change(select, { target: { value: "ipv6" } });
    expect(mockSetIpVersion).toHaveBeenCalledWith("ipv6");
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ ipVersion: "ipv6" });
  });

  it("handles model download and load success", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd, _args) => {
      if (cmd === "download_model") {
        return Promise.resolve("/fake/download/path.gguf");
      }
      if (cmd === "load_model") {
        return Promise.resolve("Model loaded successfully");
      }
      if (cmd === "network_list_serial_ports") {
        return Promise.resolve(["COM1", "COM2"]);
      }
      return Promise.reject(new Error("Unknown command"));
    });

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText("モデルをダウンロードして読み込む");
    fireEvent.click(button);

    expect(tauriApi.invoke).toHaveBeenCalledWith("download_model", {
      repo: "unsloth/gemma-4-E4B-it-GGUF",
      filename: "gemma-4-E4B-it-UD-Q4_K_XL.gguf",
    });

    await waitFor(() => {
      expect(mockSetModelPath).toHaveBeenCalledWith("/fake/download/path.gguf");
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("load_model", {
      path: "/fake/download/path.gguf",
    });

    await waitFor(() => {
      expect(screen.getByText("Success: Model loaded successfully")).toBeInTheDocument();
    });
  });

  it("handles model download failure", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd, _args) => {
      if (cmd === "download_model") {
        return Promise.reject(new Error("Network error"));
      }
      if (cmd === "network_list_serial_ports") {
        return Promise.resolve(["COM1", "COM2"]);
      }
      return Promise.reject(new Error("Unknown command"));
    });

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText("モデルをダウンロードして読み込む");
    fireEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText("Error: Network error")).toBeInTheDocument();
    });
  });

  it("handles selecting db directory", async () => {
    vi.mocked(tauriDialog.open).mockResolvedValue("/selected/db/path");

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText("参照");
    fireEvent.click(button);

    expect(tauriDialog.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: "データベースディレクトリを選択",
    });

    await waitFor(() => {
      expect(mockSetDbPath).toHaveBeenCalledWith("/selected/db/path");
    });
  });

  it("handles input changes for repoPath, modelFilename, and dbPath", () => {
    render(<SettingsPanel {...defaultProps} />);

    // repoPath input
    const repoInput = screen.getByDisplayValue("unsloth/gemma-4-E4B-it-GGUF");
    fireEvent.change(repoInput, { target: { value: "new/repo" } });
    expect(repoInput).toHaveValue("new/repo");

    // modelFilename input
    const filenameInput = screen.getByDisplayValue("gemma-4-E4B-it-UD-Q4_K_XL.gguf");
    fireEvent.change(filenameInput, { target: { value: "new_model.gguf" } });
    expect(filenameInput).toHaveValue("new_model.gguf");

    // dbPath input
    const dbInput = screen.getByPlaceholderText("/path/to/lancedb");
    fireEvent.change(dbInput, { target: { value: "/new/db/path" } });
    expect(mockSetDbPath).toHaveBeenCalledWith("/new/db/path");
  });

  it("handles db directory selection cancellation", async () => {
    vi.mocked(tauriDialog.open).mockResolvedValue(null);

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText("参照");
    fireEvent.click(button);

    // wait briefly to ensure the promise resolves
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(mockSetDbPath).not.toHaveBeenCalled();
  });

  it("handles db directory selection error", async () => {
    // mock console.error to prevent it from cluttering the test output
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(tauriDialog.open).mockRejectedValue(new Error("Dialog failed"));

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText("参照");
    fireEvent.click(button);

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalledWith("Failed to select directory:", expect.any(Error));
    });
    expect(mockSetDbPath).not.toHaveBeenCalled();

    consoleSpy.mockRestore();
  });

  it("handles save and close button click", () => {
    render(<SettingsPanel {...defaultProps} />);
    const saveButton = screen.getByText("保存して終了");
    fireEvent.click(saveButton);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });
});
