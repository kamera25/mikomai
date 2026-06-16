import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { useSettingsContext } from "../contexts/SettingsContext";
import { getErrorMessage } from "../utils/error";

import "./SettingsPanel.css";

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose }) => {
  const { t } = useTranslation();
  const {
    historyLimit,
    setHistoryLimit,
    temperature,
    setTemperature,
    repetitionPenalty,
    setRepetitionPenalty,
    modelPath: _savedModelPath,
    setModelPath,
    mcpTimeout,
    setMcpTimeout,
    cacheExpiryMinutes,
    setCacheExpiryMinutes,
    dbPath,
    setDbPath,
    ipVersion,
    setIpVersion,
    consolePort,
    setConsolePort,
    consoleBaudRate,
    setConsoleBaudRate,
    preloadInvestigate,
    setPreloadInvestigate,
    preloadKnowledge,
    setPreloadKnowledge,
    preloadAnalysis,
    setPreloadAnalysis,
    preloadRag,
    setPreloadRag,
    saveAllSettings,
  } = useSettingsContext();

  const [repoPath, setRepoPath] = useState("bartowski/google_gemma-4-E4B-it-GGUF");
  const [modelFilename, setModelFilename] = useState("google_gemma-4-E4B-it-Q4_K_M.gguf");
  const [availablePorts, setAvailablePorts] = useState<string[]>([]);

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

  const handleModelPathChange = (val: string) => {
    setModelPath(val);
    saveAllSettings({ modelPath: val });
  };

  const handleMcpTimeoutChange = (val: number) => {
    setMcpTimeout(val);
    saveAllSettings({ mcpTimeout: val });
  };

  const handleCacheExpiryMinutesChange = (val: number) => {
    setCacheExpiryMinutes(val);
    saveAllSettings({ cacheExpiryMinutes: val });
  };

  const handleDbPathChange = (val: string) => {
    setDbPath(val);
    saveAllSettings({ dbPath: val });
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

  const handlePreloadInvestigateChange = (val: boolean) => {
    setPreloadInvestigate(val);
    saveAllSettings({ preloadInvestigate: val });
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
          setAvailablePorts(ports);
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

      handleModelPathChange(downloadedPath);
      setDownloadStatus(`Model downloaded to: ${downloadedPath}. Loading into memory...`);

      const loadResult = await invoke<string>("load_model", {
        path: downloadedPath,
      });

      setDownloadStatus(`Success: ${loadResult}`);
    } catch (e: unknown) {
      setDownloadStatus(`Error: ${getErrorMessage(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenModelDir = async () => {
    try {
      await invoke("open_model_dir", { modelPath: _savedModelPath });
    } catch (e: unknown) {
      setDownloadStatus(`Error: ${getErrorMessage(e)}`);
    }
  };

  const handleSelectDbDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("settings.title_select_db_dir"),
      });
      if (selected) {
        handleDbPathChange(selected as string);
      }
    } catch (e) {
      console.error("Failed to select directory:", e);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="settings-full-overlay">
      <div className="settings-panel">
        <header className="settings-header">
          <div className="settings-header-content">
            <div>
              <h2>{t("settings.header")}</h2>
              <p>{t("settings.desc")}</p>
            </div>
            <button className="close-button" onClick={onClose} title={t("settings.close_title")}>
              &times;
            </button>
          </div>
        </header>

        <div className="settings-body">
          <section className="settings-group">
            <h3>{t("settings.sub_chat")}</h3>
            <div className="form-control">
              <label>{t("settings.label_history")}</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="0"
                  max="20"
                  value={historyLimit}
                  onChange={(e) => handleHistoryLimitChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{historyLimit}</span>
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_history")}
              </p>
            </div>
            <div className="form-control">
              <label>{t("settings.label_temp")}</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="0"
                  max="2.0"
                  step="0.1"
                  value={temperature}
                  onChange={(e) => handleTemperatureChange(parseFloat(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{temperature.toFixed(1)}</span>
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_temp")}
              </p>
            </div>
            <div className="form-control">
              <label>{t("settings.label_rep_penalty")}</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="1.0"
                  max="2.0"
                  step="0.05"
                  value={repetitionPenalty}
                  onChange={(e) => handleRepetitionPenaltyChange(parseFloat(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{repetitionPenalty.toFixed(2)}</span>
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_rep_penalty")}
              </p>
            </div>
            <div className="form-control">
              <label>{t("settings.label_mcp_timeout")}</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="5"
                  max="120"
                  step="5"
                  value={mcpTimeout}
                  onChange={(e) => handleMcpTimeoutChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{mcpTimeout}</span>
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_mcp_timeout")}
              </p>
            </div>
            <div className="form-control">
              <label>{t("settings.label_cache_expiry")}</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="0"
                  max="60"
                  step="1"
                  value={cacheExpiryMinutes}
                  onChange={(e) => handleCacheExpiryMinutesChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{cacheExpiryMinutes}</span>
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_cache_expiry")}
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="ip-version-select">{t("settings.label_ip_version")}</label>
              <select
                id="ip-version-select"
                value={ipVersion}
                onChange={(e) => handleIpVersionChange(e.target.value)}
              >
                <option value="auto">{t("settings.opt_auto")}</option>
                <option value="ipv4">{t("settings.opt_ipv4")}</option>
                <option value="ipv6">{t("settings.opt_ipv6")}</option>
              </select>
              <p className="help-text form-help-text">
                {t("settings.desc_ip_version")}
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="console-port-select">{t("settings.label_console_port")}</label>
              <div className="download-actions">
                <select
                  id="console-port-select"
                  value={
                    availablePorts.includes(consolePort || "")
                      ? consolePort || ""
                      : consolePort
                        ? "custom"
                        : ""
                  }
                  onChange={(e) => {
                    const val = e.target.value;
                    if (val === "custom") {
                      handleConsolePortChange("/dev/ttyUSB0");
                    } else if (val === "") {
                      handleConsolePortChange("");
                    } else {
                      handleConsolePortChange(val);
                    }
                  }}
                  className="download-btn-stretch"
                >
                  <option value="">{t("settings.opt_none")}</option>
                  {availablePorts.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                  <option value="custom">{t("settings.opt_custom")}</option>
                </select>
                {(!availablePorts.includes(consolePort || "") && consolePort) ||
                consolePort === "custom" ? (
                  <input
                    type="text"
                    value={consolePort === "custom" ? "" : consolePort || ""}
                    onChange={(e) => handleConsolePortChange(e.target.value)}
                    placeholder={t("settings.placeholder_console_port")}
                    className="custom-port-input"
                  />
                ) : null}
              </div>
              <p className="help-text form-help-text">
                {t("settings.desc_console_port")}
              </p>
            </div>
            {consolePort && consolePort !== "" && (
              <div className="form-control">
                <label htmlFor="console-baudrate-select">{t("settings.label_console_baudrate")}</label>
                <select
                  id="console-baudrate-select"
                  value={consoleBaudRate}
                  onChange={(e) => handleConsoleBaudRateChange(parseInt(e.target.value))}
                >
                  <option value="9600">9600 bps</option>
                  <option value="19200">19200 bps</option>
                  <option value="38400">38400 bps</option>
                  <option value="57600">57600 bps</option>
                  <option value="115200">115200 bps</option>
                </select>
              </div>
            )}
          </section>

          <section className="settings-group">
            <h3>{t("settings.sub_llm")}</h3>
            <div className="form-control">
              <label>{t("settings.label_hf_repo")}</label>
              <input
                type="text"
                value={repoPath}
                onChange={(e) => setRepoPath(e.target.value)}
                placeholder="bartowski/google_gemma-4-E2B-it-GGUF"
              />
            </div>
            <div className="form-control">
              <label>{t("settings.label_gguf_file")}</label>
              <input
                type="text"
                value={modelFilename}
                onChange={(e) => setModelFilename(e.target.value)}
                placeholder="google_gemma-4-E2B-it-Q4_K_M.gguf"
              />
            </div>
            <div className="form-control">
              <label>{t("settings.label_kv_preload")}</label>
              <p className="help-text form-help-text margin-bottom">
                {t("settings.desc_kv_preload")}
              </p>
              <div className="preload-grid">
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadInvestigate}
                    onChange={(e) => handlePreloadInvestigateChange(e.target.checked)}
                  />
                  {t("settings.worker_investigator")}
                </label>
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadKnowledge}
                    onChange={(e) => handlePreloadKnowledgeChange(e.target.checked)}
                  />
                  {t("settings.worker_knowledge")}
                </label>
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadAnalysis}
                    onChange={(e) => handlePreloadAnalysisChange(e.target.checked)}
                  />
                  {t("settings.worker_analyst")}
                </label>
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadRag}
                    onChange={(e) => handlePreloadRagChange(e.target.checked)}
                  />
                  {t("settings.worker_rag")}
                </label>
              </div>
            </div>
            <div className="form-control">
              <div className="download-actions">
                <button
                  className="btn btn-secondary download-btn-stretch"
                  onClick={handleDownloadAndLoad}
                  disabled={isLoading}
                >
                  {isLoading ? t("settings.btn_downloading") : t("settings.btn_download_load")}
                </button>
                <button
                  className="btn btn-secondary"
                  onClick={handleOpenModelDir}
                  title={t("settings.btn_open_folder_title")}
                >
                  {t("settings.btn_open_folder")}
                </button>
              </div>
              {downloadStatus && (
                <div
                  className="status-text download-status-container"
                  style={{
                    color: downloadStatus.startsWith("Error") ? "var(--danger)" : "var(--success)",
                  }}
                >
                  {downloadStatus}
                </div>
              )}
            </div>
          </section>

          <section className="settings-group">
            <h3>{t("settings.sub_kb")}</h3>
            <div className="form-control">
              <label>{t("settings.label_db_dir")}</label>
              <div className="input-with-button">
                <input
                  type="text"
                  placeholder="/path/to/lancedb"
                  value={dbPath}
                  onChange={(e) => handleDbPathChange(e.target.value)}
                />

                <button className="btn btn-secondary" onClick={handleSelectDbDir}>
                  {t("settings.btn_browse")}
                </button>
              </div>
            </div>
            <div className="form-control">
              <label>{t("settings.label_embed_model")}</label>
              <select>
                <option>{t("settings.opt_multilingual_e5")}</option>
              </select>
            </div>
          </section>
        </div>

        <footer className="settings-footer">
          <div className="settings-footer-content">
            <button className="btn btn-primary" onClick={onClose}>
              {t("settings.btn_save_exit")}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
};
