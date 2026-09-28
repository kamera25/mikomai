import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { SettingsPanel } from "../SettingsPanel";
import * as tauriApi from "@tauri-apps/api/core";

// Mock the SettingsContext
const mockSetHistoryLimit = vi.fn();
const mockSetTemperature = vi.fn();
const mockSetRepetitionPenalty = vi.fn();
const mockSetModelPath = vi.fn();
const mockSetMcpTimeout = vi.fn();
const mockSetCacheExpiryMinutes = vi.fn();
const mockSetIpVersion = vi.fn();
const mockSetConsolePort = vi.fn();
const mockSetConsoleBaudRate = vi.fn();
const mockSetPreloadKnowledge = vi.fn();
const mockSetPreloadAnalysis = vi.fn();
const mockSetPreloadRag = vi.fn();
const mockSetVisionEnabled = vi.fn();
const mockSetMmprojPath = vi.fn();
const mockSaveAllSettings = vi.fn();

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
    ipVersion: "auto",
    setIpVersion: mockSetIpVersion,
    consolePort: null,
    setConsolePort: mockSetConsolePort,
    consoleBaudRate: 9600,
    setConsoleBaudRate: mockSetConsoleBaudRate,
    preloadKnowledge: true,
    setPreloadKnowledge: mockSetPreloadKnowledge,
    preloadAnalysis: true,
    setPreloadAnalysis: mockSetPreloadAnalysis,
    preloadRag: true,
    setPreloadRag: mockSetPreloadRag,
    visionEnabled: false,
    setVisionEnabled: mockSetVisionEnabled,
    mmprojPath: "",
    setMmprojPath: mockSetMmprojPath,
    saveAllSettings: mockSaveAllSettings,
  }),
}));

// Mock Tauri invoke and dialog functions
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
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
      if (cmd === "check_model_exists") {
        return Promise.resolve(false);
      }
      return Promise.reject(new Error("Unknown command"));
    });
  });

  const renderPanel = async (props: Partial<typeof defaultProps> = {}) => {
    const view = render(<SettingsPanel {...defaultProps} {...props} />);
    if (props.isOpen !== false) {
      await waitFor(() => {
        expect(tauriApi.invoke).toHaveBeenCalledWith("network_list_serial_ports");
        expect(tauriApi.invoke).toHaveBeenCalledWith("check_model_exists", expect.anything());
      });
    }
    return view;
  };

  it("renders correctly when isOpen is true", async () => {
    await renderPanel();
    expect(screen.getByText("設定")).toBeInTheDocument();
  });

  it("does not render when isOpen is false", async () => {
    await renderPanel({ isOpen: false });
    expect(screen.queryByText("設定")).not.toBeInTheDocument();
  });

  it("handles history limit change", async () => {
    await renderPanel();
    const slider = screen.getAllByRole("slider")[0]; // History limit
    fireEvent.change(slider, { target: { value: "15" } });
    expect(mockSetHistoryLimit).toHaveBeenCalledWith(15);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ historyLimit: 15 });
  });

  it("handles temperature change", async () => {
    await renderPanel();
    const slider = screen.getAllByRole("slider")[1]; // Temperature
    fireEvent.change(slider, { target: { value: "0.8" } });
    expect(mockSetTemperature).toHaveBeenCalledWith(0.8);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ temperature: 0.8 });
  });

  it("handles repetition penalty change", async () => {
    await renderPanel();
    const slider = screen.getAllByRole("slider")[2]; // Repetition Penalty
    fireEvent.change(slider, { target: { value: "1.2" } });
    expect(mockSetRepetitionPenalty).toHaveBeenCalledWith(1.2);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ repetitionPenalty: 1.2 });
  });

  it("handles mcp timeout change", async () => {
    await renderPanel();
    const slider = screen.getAllByRole("slider")[3]; // MCP Timeout
    fireEvent.change(slider, { target: { value: "60" } });
    expect(mockSetMcpTimeout).toHaveBeenCalledWith(60);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ mcpTimeout: 60 });
  });

  it("handles cache expiry minutes change", async () => {
    await renderPanel();
    const slider = screen.getAllByRole("slider")[4]; // Cache Expiry Minutes
    fireEvent.change(slider, { target: { value: "15" } });
    expect(mockSetCacheExpiryMinutes).toHaveBeenCalledWith(15);
    expect(mockSaveAllSettings).toHaveBeenCalledWith({ cacheExpiryMinutes: 15 });
  });

  it("handles ip version change", async () => {
    await renderPanel();
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
      if (cmd === "check_model_exists") {
        return Promise.resolve(false);
      }
      return Promise.reject(new Error("Unknown command"));
    });

    await renderPanel();
    const button = screen.getByText("モデルをダウンロードして読み込む");
    fireEvent.click(button);

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith("download_model", {
        repo: "unsloth/gemma-4-E4B-it-GGUF",
        filename: "gemma-4-E4B-it-UD-Q4_K_XL.gguf",
      });
      expect(tauriApi.invoke).toHaveBeenCalledWith("download_model", {
        repo: "unsloth/gemma-4-E4B-it-GGUF",
        filename: "mmproj-F16.gguf",
      });
      expect(mockSetModelPath).toHaveBeenCalledWith("/fake/download/path.gguf");
      expect(mockSetMmprojPath).toHaveBeenCalledWith("/fake/download/path.gguf");
      expect(mockSetVisionEnabled).toHaveBeenCalledWith(true);
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("load_model", {
      path: "/fake/download/path.gguf",
    });

    await waitFor(() => {
      expect(
        screen.getByText("成功: Model loaded successfully (Visionプロジェクター設定完了)")
      ).toBeInTheDocument();
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
      if (cmd === "check_model_exists") {
        return Promise.resolve(false);
      }
      return Promise.reject(new Error("Unknown command"));
    });

    await renderPanel();
    const button = screen.getByText("モデルをダウンロードして読み込む");
    fireEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText("エラー: Network error")).toBeInTheDocument();
    });
  });

  it("disables repoPath and modelFilename inputs when preset is selected and enables when custom is selected", async () => {
    await renderPanel();

    const repoInput = screen.getByDisplayValue("unsloth/gemma-4-E4B-it-GGUF");
    const filenameInput = screen.getByDisplayValue("gemma-4-E4B-it-UD-Q4_K_XL.gguf");
    const presetSelect = screen.getByLabelText("モデル選択 (プリセット)");

    // Inputs should be disabled when preset is selected
    expect(repoInput).toBeDisabled();
    expect(filenameInput).toBeDisabled();

    // Select custom
    await act(async () => {
      fireEvent.change(presetSelect, { target: { value: "custom" } });
    });

    // Inputs should now be enabled
    expect(repoInput).not.toBeDisabled();
    expect(filenameInput).not.toBeDisabled();

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith("check_model_exists", expect.anything());
    });
  });

  it("handles input changes for repoPath and modelFilename when custom is selected", async () => {
    await renderPanel();

    const presetSelect = screen.getByLabelText("モデル選択 (プリセット)");
    await act(async () => {
      fireEvent.change(presetSelect, { target: { value: "custom" } });
    });

    // repoPath input
    const repoInput = screen.getByDisplayValue("unsloth/gemma-4-E4B-it-GGUF");
    await act(async () => {
      fireEvent.change(repoInput, { target: { value: "new/repo" } });
    });
    expect(repoInput).toHaveValue("new/repo");

    // modelFilename input
    const filenameInput = screen.getByDisplayValue("gemma-4-E4B-it-UD-Q4_K_XL.gguf");
    await act(async () => {
      fireEvent.change(filenameInput, { target: { value: "new_model.gguf" } });
    });
    expect(filenameInput).toHaveValue("new_model.gguf");

    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith("check_model_exists", expect.anything());
    });
  });

  it("displays model download status correctly", async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd, args) => {
      if (cmd === "check_model_exists") {
        const a = args as { repo: string; filename: string };
        if (a.repo === "unsloth/gemma-4-E4B-it-GGUF") {
          return Promise.resolve(true);
        }
        return Promise.resolve(false);
      }
      if (cmd === "network_list_serial_ports") {
        return Promise.resolve(["COM1", "COM2"]);
      }
      return Promise.reject(new Error("Unknown command"));
    });

    await renderPanel();

    await waitFor(() => {
      expect(screen.getByText("ダウンロード状態:")).toBeInTheDocument();
      expect(screen.getByText("✓ ダウンロード済み")).toBeInTheDocument();
    });
  });

  it("handles sidebar quick access navigation", async () => {
    // Mock scrollIntoView since jsdom does not implement it
    const scrollIntoViewMock = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;

    await renderPanel();

    const llmNavBtn = screen.getAllByText("ローカルLLM (llama.cpp)")[0].closest("button");
    expect(llmNavBtn).not.toHaveClass("active");

    if (llmNavBtn) {
      await act(async () => {
        fireEvent.click(llmNavBtn);
      });
      expect(llmNavBtn).toHaveClass("active");
      expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: "smooth", block: "start" });
    }
  });
});
