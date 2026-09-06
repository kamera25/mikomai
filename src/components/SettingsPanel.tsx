import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { useSettingsContext } from "../contexts/SettingsContext";
import { getErrorMessage } from "../utils/error";
import { findPreset, PRESET_MODELS } from "./settingsModelPresets";
import { SettingsCategories } from "./SettingsCategories";

import "./SettingsPanel.css";

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export type { ModelPreset } from "./settingsModelPresets";
export { PRESET_MODELS } from "./settingsModelPresets";

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose: _onClose }) => {
  const { t } = useTranslation();
  const {
    historyLimit,
    setHistoryLimit,
    temperature,
    setTemperature,
    repetitionPenalty,
    setRepetitionPenalty,
    modelPath: savedModelPath,
    setModelPath,
    mcpTimeout,
    setMcpTimeout,
    cacheExpiryMinutes,
    setCacheExpiryMinutes,
    ipVersion,
    setIpVersion,
    consolePort,
    setConsolePort,
    consoleBaudRate,
    setConsoleBaudRate,
    preloadKnowledge,
    setPreloadKnowledge,
    preloadAnalysis,
    setPreloadAnalysis,
    preloadRag,
    setPreloadRag,
    preloadPlotter,
    setPreloadPlotter,
    preloadBuilder,
    setPreloadBuilder,
    preloadSummarization,
    setPreloadSummarization,
    visionEnabled,
    setVisionEnabled,
    autoDryRun,
    setAutoDryRun,
    mmprojPath,
    setMmprojPath,
    saveAllSettings,
  } = useSettingsContext();

  const [repoPath, setRepoPath] = useState("unsloth/gemma-4-E4B-it-GGUF");
  const [modelFilename, setModelFilename] = useState("gemma-4-E4B-it-UD-Q4_K_XL.gguf");
  const [selectedPresetId, setSelectedPresetId] = useState<string>("gemma-4-e4b-ud");
  const [availablePorts, setAvailablePorts] = useState<string[]>([]);
  const [downloadedPresets, setDownloadedPresets] = useState<Record<string, boolean>>({});
  const [isCurrentModelDownloaded, setIsCurrentModelDownloaded] = useState<boolean>(false);

  const checkModelStatuses = useCallback(async () => {
    try {
      const statusMap: Record<string, boolean> = {};
      for (const preset of PRESET_MODELS) {
        const exists = await invoke<boolean>("check_model_exists", {
          repo: preset.repo,
          filename: preset.filename,
        });
        statusMap[preset.id] = exists;
      }
      setDownloadedPresets(statusMap);

      const currentExists = await invoke<boolean>("check_model_exists", {
        repo: repoPath,
        filename: modelFilename,
      });
      setIsCurrentModelDownloaded(currentExists);
    } catch (e) {
      console.error("Failed to check model status:", e);
    }
  }, [repoPath, modelFilename]);

  useEffect(() => {
    if (isOpen) {
      checkModelStatuses();
    }
  }, [isOpen, repoPath, modelFilename, checkModelStatuses]);

  // Sync preset selection and file inputs with savedModelPath when panel opens or savedModelPath changes
  useEffect(() => {
    if (!savedModelPath) return;
    const parts = savedModelPath.split(/[/\\]/);
    const fname = parts[parts.length - 1];
    const match = PRESET_MODELS.find((p) => p.filename === fname);
    if (match) {
      setSelectedPresetId(match.id);
      setRepoPath(match.repo);
      setModelFilename(match.filename);
    } else {
      setSelectedPresetId("custom");
      setModelFilename(fname);
    }
  }, [savedModelPath, isOpen]);

  const handlePresetSelect = (presetId: string) => {
    setSelectedPresetId(presetId);
    if (presetId !== "custom") {
      const preset = PRESET_MODELS.find((p) => p.id === presetId);
      if (preset) {
        setRepoPath(preset.repo);
        setModelFilename(preset.filename);
      }
    }
  };

  const handleRepoPathChange = (val: string) => {
    setRepoPath(val);
    const match = findPreset(val, modelFilename);
    setSelectedPresetId(match ? match.id : "custom");
  };

  const handleModelFilenameChange = (val: string) => {
    setModelFilename(val);
    const match = findPreset(repoPath, val);
    setSelectedPresetId(match ? match.id : "custom");
  };

  // Update handlers
  const handleHistoryLimitChange = (val: number) => {
    setHistoryLimit(val);
    saveAllSettings({ historyLimit: val });
  };

  const handleTemperatureChange = (val: number) => {
    setTemperature(val);
    saveAllSettings({ temperature: val });
  };

  const handleRepetitionPenaltyChange = (val: number) => {
    setRepetitionPenalty(val);
    saveAllSettings({ repetitionPenalty: val });
  };

  const handleMcpTimeoutChange = (val: number) => {
    setMcpTimeout(val);
    saveAllSettings({ mcpTimeout: val });
  };

  const handleCacheExpiryMinutesChange = (val: number) => {
    setCacheExpiryMinutes(val);
    saveAllSettings({ cacheExpiryMinutes: val });
  };

  const handleIpVersionChange = (val: string) => {
    setIpVersion(val);
    saveAllSettings({ ipVersion: val });
  };

  const handleConsolePortChange = (val: string) => {
    setConsolePort(val);
    saveAllSettings({ consolePort: val });
  };

  const handleConsoleBaudRateChange = (val: number) => {
    setConsoleBaudRate(val);
    saveAllSettings({ consoleBaudRate: val });
  };

  const handlePreloadKnowledgeChange = (val: boolean) => {
    setPreloadKnowledge(val);
    saveAllSettings({ preloadKnowledge: val });
  };

  const handlePreloadAnalysisChange = (val: boolean) => {
    setPreloadAnalysis(val);
    saveAllSettings({ preloadAnalysis: val });
  };

  const handlePreloadRagChange = (val: boolean) => {
    setPreloadRag(val);
    saveAllSettings({ preloadRag: val });
  };

  const handlePreloadBuilderChange = (val: boolean) => {
    setPreloadBuilder(val);
    saveAllSettings({ preloadBuilder: val });
  };

  const handlePreloadPlotterChange = (val: boolean) => {
    setPreloadPlotter(val);
    saveAllSettings({ preloadPlotter: val });
  };

  const handlePreloadSummarizationChange = (val: boolean) => {
    setPreloadSummarization(val);
    saveAllSettings({ preloadSummarization: val });
  };

  const handleVisionEnabledChange = (val: boolean) => {
    setVisionEnabled(val);
    saveAllSettings({ visionEnabled: val });
  };

  const handleAutoDryRunChange = (val: boolean) => {
    setAutoDryRun(val);
    saveAllSettings({ autoDryRun: val });
  };

  const handleMmprojPathChange = (val: string) => {
    setMmprojPath(val);
    saveAllSettings({ mmprojPath: val });
  };

  const handleSelectMmprojFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "GGUF Model", extensions: ["gguf"] }],
      });
      if (selected && typeof selected === "string") {
        handleMmprojPathChange(selected);
      }
    } catch (err) {
      console.error("Failed to select mmproj file:", err);
    }
  };

  useEffect(() => {
    const fetchPorts = async () => {
      try {
        interface SerialPortsResponse {
          success: boolean;
          output?: string;
          error?: string;
        }
        const result = await invoke<SerialPortsResponse>("network_list_serial_ports");
        if (result && result.success && result.output) {
          const ports: string[] = [];
          const lines = result.output.split("\n");
          for (const line of lines) {
            if (line.trim().startsWith("- ")) {
              const parts = line.trim().substring(2).split(":");
              if (parts[0]) {
                ports.push(parts[0].trim());
              }
            }
          }
          const filteredPorts = ports.filter(
            (p) => p.toLowerCase().includes("serial") && p.includes("cu.")
          );
          setAvailablePorts(filteredPorts);
        }
      } catch (e) {
        console.error("Failed to fetch serial ports for settings:", e);
      }
    };
    fetchPorts();
  }, []);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const handleDownloadAndLoad = async () => {
    try {
      setIsLoading(true);
      setDownloadStatus(t("settings.status_start_download"));

      const downloadedPath = await invoke<string>("download_model", {
        repo: repoPath,
        filename: modelFilename,
      });

      // Download mmproj (vision projector) file together with Gemma model
      const preset = PRESET_MODELS.find((p) => p.repo === repoPath && p.filename === modelFilename);
      const mmprojFilenameToUse = preset?.mmprojFilename || "mmproj-F16.gguf";

      setDownloadStatus(`Visionプロジェクター (${mmprojFilenameToUse}) をダウンロード中...`);

      let downloadedMmprojPath: string | null = null;
      try {
        downloadedMmprojPath = await invoke<string>("download_model", {
          repo: repoPath,
          filename: mmprojFilenameToUse,
        });
      } catch (firstErr) {
        if (mmprojFilenameToUse !== "mmproj.gguf") {
          try {
            downloadedMmprojPath = await invoke<string>("download_model", {
              repo: repoPath,
              filename: "mmproj.gguf",
            });
          } catch (fallbackErr) {
            console.warn("Failed to download mmproj model (fallback):", fallbackErr);
          }
        } else {
          console.warn("Failed to download mmproj model:", firstErr);
        }
      }

      setModelPath(downloadedPath);
      const settingsOverrides: {
        modelPath: string;
        mmprojPath?: string;
        visionEnabled?: boolean;
      } = {
        modelPath: downloadedPath,
      };

      if (downloadedMmprojPath) {
        setMmprojPath(downloadedMmprojPath);
        setVisionEnabled(true);
        settingsOverrides.mmprojPath = downloadedMmprojPath;
        settingsOverrides.visionEnabled = true;
      }

      await saveAllSettings(settingsOverrides);

      setDownloadStatus(`モデル準備完了: ${downloadedPath}. メモリへロード中...`);

      const loadResult = await invoke<string>("load_model", {
        path: downloadedPath,
      });

      setDownloadStatus(
        `成功: ${loadResult}${downloadedMmprojPath ? " (Visionプロジェクター設定完了)" : ""}`
      );
      await checkModelStatuses();
    } catch (e: unknown) {
      setDownloadStatus(`エラー: ${getErrorMessage(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenModelDir = async () => {
    try {
      await invoke("open_model_dir", { modelPath: savedModelPath });
    } catch (e: unknown) {
      setDownloadStatus(`Error: ${getErrorMessage(e)}`);
    }
  };

  const [activeTab, setActiveTab] = useState<string>("chat");
  const bodyRef = React.useRef<HTMLDivElement>(null);
  const isClickScrollingRef = React.useRef<boolean>(false);

  const handleScroll = useCallback(() => {
    if (isClickScrollingRef.current) return;

    const categories = ["chat", "llm", "vision", "kb"];
    const container = bodyRef.current;
    if (!container) return;

    const containerTop = container.getBoundingClientRect().top;

    let currentActive = categories[0];
    for (const cat of categories) {
      const el = document.getElementById(`settings-section-${cat}`);
      if (el) {
        const rect = el.getBoundingClientRect();
        // If top of element is near or above the top half of the scroll container
        if (rect.top - containerTop <= 120) {
          currentActive = cat;
        }
      }
    }
    setActiveTab(currentActive);
  }, []);

  const scrollToCategory = (categoryId: string) => {
    setActiveTab(categoryId);
    isClickScrollingRef.current = true;
    const element = document.getElementById(`settings-section-${categoryId}`);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "start" });
      setTimeout(() => {
        isClickScrollingRef.current = false;
      }, 600);
    } else {
      isClickScrollingRef.current = false;
    }
  };

  if (!isOpen) return null;

  return (
    <div className="settings-full-overlay">
      <div className="settings-panel">
        <header className="settings-header">
          <div className="settings-header-content">
            <div className="settings-header-left">
              <h2>{t("settings.header")}</h2>
            </div>
          </div>
        </header>

        <div className="settings-container">
          <aside className="settings-sidebar">
            <nav className="settings-nav">
              <button
                className={`settings-nav-item ${activeTab === "chat" ? "active" : ""}`}
                onClick={() => scrollToCategory("chat")}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                </svg>
                <span>{t("settings.sub_chat")}</span>
              </button>
              <button
                className={`settings-nav-item ${activeTab === "llm" ? "active" : ""}`}
                onClick={() => scrollToCategory("llm")}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm0 18a8 8 0 1 1 8-8 8 8 0 0 1-8 8z"></path>
                  <circle cx="12" cy="12" r="3"></circle>
                </svg>
                <span>{t("settings.sub_llm")}</span>
              </button>
              <button
                className={`settings-nav-item ${activeTab === "vision" ? "active" : ""}`}
                onClick={() => scrollToCategory("vision")}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
                  <circle cx="12" cy="12" r="3"></circle>
                </svg>
                <span>{t("settings.label_vision")}</span>
              </button>
              <button
                className={`settings-nav-item ${activeTab === "kb" ? "active" : ""}`}
                onClick={() => scrollToCategory("kb")}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                  <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                  <path d="M21 19c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                </svg>
                <span>{t("settings.sub_kb")}</span>
              </button>
            </nav>
          </aside>

          <div className="settings-body" ref={bodyRef} onScroll={handleScroll}>
            <SettingsCategories
              t={t}
              PRESET_MODELS={PRESET_MODELS}
              historyLimit={historyLimit}
              temperature={temperature}
              repetitionPenalty={repetitionPenalty}
              mcpTimeout={mcpTimeout}
              cacheExpiryMinutes={cacheExpiryMinutes}
              ipVersion={ipVersion}
              consolePort={consolePort}
              consoleBaudRate={consoleBaudRate}
              preloadKnowledge={preloadKnowledge}
              preloadAnalysis={preloadAnalysis}
              preloadRag={preloadRag}
              preloadPlotter={preloadPlotter}
              preloadBuilder={preloadBuilder}
              preloadSummarization={preloadSummarization}
              visionEnabled={visionEnabled}
              autoDryRun={autoDryRun}
              mmprojPath={mmprojPath}
              repoPath={repoPath}
              modelFilename={modelFilename}
              selectedPresetId={selectedPresetId}
              availablePorts={availablePorts}
              downloadedPresets={downloadedPresets}
              isCurrentModelDownloaded={isCurrentModelDownloaded}
              downloadStatus={downloadStatus}
              isLoading={isLoading}
              handleHistoryLimitChange={handleHistoryLimitChange}
              handleTemperatureChange={handleTemperatureChange}
              handleRepetitionPenaltyChange={handleRepetitionPenaltyChange}
              handleMcpTimeoutChange={handleMcpTimeoutChange}
              handleCacheExpiryMinutesChange={handleCacheExpiryMinutesChange}
              handleIpVersionChange={handleIpVersionChange}
              handleConsolePortChange={handleConsolePortChange}
              handleConsoleBaudRateChange={handleConsoleBaudRateChange}
              handlePreloadKnowledgeChange={handlePreloadKnowledgeChange}
              handlePreloadAnalysisChange={handlePreloadAnalysisChange}
              handlePreloadRagChange={handlePreloadRagChange}
              handlePreloadPlotterChange={handlePreloadPlotterChange}
              handlePreloadBuilderChange={handlePreloadBuilderChange}
              handlePreloadSummarizationChange={handlePreloadSummarizationChange}
              handleVisionEnabledChange={handleVisionEnabledChange}
              handleAutoDryRunChange={handleAutoDryRunChange}
              handleMmprojPathChange={handleMmprojPathChange}
              handleSelectMmprojFile={handleSelectMmprojFile}
              handlePresetSelect={handlePresetSelect}
              handleRepoPathChange={handleRepoPathChange}
              handleModelFilenameChange={handleModelFilenameChange}
              handleDownloadAndLoad={handleDownloadAndLoad}
              handleOpenModelDir={handleOpenModelDir}
            />
          </div>
        </div>
      </div>
    </div>
  );
};
