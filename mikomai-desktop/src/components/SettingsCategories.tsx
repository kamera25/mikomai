import React from "react";
import type { ModelPreset } from "./settingsModelPresets";

export interface SettingsCategoriesProps {
  [key: string]: any;
}

export const SettingsCategories: React.FC<SettingsCategoriesProps> = (props) => {
  const { t, historyLimit, temperature, repetitionPenalty, mcpTimeout, cacheExpiryMinutes, ipVersion, consolePort, consoleBaudRate, preloadKnowledge, preloadAnalysis, preloadRag, preloadPlotter, preloadBuilder, preloadSummarization, visionEnabled, autoDryRun, mmprojPath, repoPath, modelFilename, selectedPresetId, availablePorts, downloadedPresets, isCurrentModelDownloaded, downloadStatus, isLoading, handleHistoryLimitChange, handleTemperatureChange, handleRepetitionPenaltyChange, handleMcpTimeoutChange, handleCacheExpiryMinutesChange, handleIpVersionChange, handleConsolePortChange, handleConsoleBaudRateChange, handlePreloadKnowledgeChange, handlePreloadAnalysisChange, handlePreloadRagChange, handlePreloadPlotterChange, handlePreloadBuilderChange, handlePreloadSummarizationChange, handleVisionEnabledChange, handleAutoDryRunChange, handleMmprojPathChange, handleSelectMmprojFile, handlePresetSelect, handleRepoPathChange, handleModelFilenameChange, handleDownloadAndLoad, handleOpenModelDir, PRESET_MODELS } = props;
  return (
    <>
            <section id="settings-section-chat" className="settings-group">
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
              <label htmlFor="auto-dry-run-checkbox" style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                <input
                  id="auto-dry-run-checkbox"
                  type="checkbox"
                  checked={autoDryRun}
                  onChange={(e) => handleAutoDryRunChange(e.target.checked)}
                />
                {t("settings.label_auto_dry_run")}
              </label>
              <p className="help-text form-help-text">
                {t("settings.desc_auto_dry_run")}
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
                  {availablePorts.map((p: string) => (
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

          <section id="settings-section-llm" className="settings-group">
            <h3>{t("settings.sub_llm")}</h3>
            <div className="form-control">
              <label htmlFor="model-preset-select">{t("settings.label_model_preset")}</label>
              <select
                id="model-preset-select"
                value={selectedPresetId}
                onChange={(e) => handlePresetSelect(e.target.value)}
              >
                {PRESET_MODELS.map((preset: ModelPreset) => {
                  const isDownloaded = downloadedPresets[preset.id];
                  const statusTag = isDownloaded
                    ? ` (${t("settings.status_downloaded")})`
                    : ` (${t("settings.status_not_downloaded")})`;
                  return (
                    <option key={preset.id} value={preset.id}>
                      {t(preset.labelKey) + statusTag}
                    </option>
                  );
                })}
                <option value="custom">{t("settings.opt_custom")}</option>
              </select>
            </div>
            <div className="form-control">
              <label htmlFor="hf-repo-input">{t("settings.label_hf_repo")}</label>
              <input
                id="hf-repo-input"
                type="text"
                value={repoPath}
                onChange={(e) => handleRepoPathChange(e.target.value)}
                placeholder="unsloth/gemma-4-E4B-it-GGUF"
                disabled={selectedPresetId !== "custom"}
              />
            </div>
            <div className="form-control">
              <label htmlFor="gguf-file-input">{t("settings.label_gguf_file")}</label>
              <input
                id="gguf-file-input"
                type="text"
                value={modelFilename}
                onChange={(e) => handleModelFilenameChange(e.target.value)}
                placeholder="gemma-4-E4B-it-UD-Q4_K_XL.gguf"
                disabled={selectedPresetId !== "custom"}
              />
            </div>
            <div className="form-control">
              <div className="model-status-indicator">
                <span className="model-status-label">{t("settings.status_model_presence")}: </span>
                <span
                  className="model-status-value"
                  style={{
                    fontWeight: "600",
                    color: isCurrentModelDownloaded ? "#16a34a" : "#64748b",
                  }}
                >
                  {isCurrentModelDownloaded
                    ? `✓ ${t("settings.status_downloaded")}`
                    : t("settings.status_not_downloaded")}
                </span>
              </div>
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
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadBuilder}
                    onChange={(e) => handlePreloadBuilderChange(e.target.checked)}
                  />
                  {t("settings.worker_builder")}
                </label>
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadPlotter}
                    onChange={(e) => handlePreloadPlotterChange(e.target.checked)}
                  />
                  {t("settings.worker_plotter")}
                </label>
                <label className="preload-label">
                  <input
                    type="checkbox"
                    checked={preloadSummarization}
                    onChange={(e) => handlePreloadSummarizationChange(e.target.checked)}
                  />
                  {t("settings.worker_summarization")}
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

          <section id="settings-section-vision" className="settings-group">
            <h3>{t("settings.label_vision")}</h3>
            <div className="form-control">
              <label className="preload-label">
                <input
                  type="checkbox"
                  checked={visionEnabled}
                  onChange={(e) => handleVisionEnabledChange(e.target.checked)}
                />
                {t("settings.label_vision_enabled")}
              </label>
              <p className="help-text form-help-text">
                {t("settings.desc_vision_enabled")}
              </p>
            </div>
            <div className="form-control">
              <label>{t("settings.label_mmproj_path")}</label>
              <p className="help-text form-help-text margin-bottom">
                {t("settings.desc_mmproj_path")}
              </p>
              <div className="input-with-button">
                <input
                  type="text"
                  placeholder={t("settings.placeholder_mmproj_path")}
                  value={mmprojPath || ""}
                  onChange={(e) => handleMmprojPathChange(e.target.value)}
                />
                <button className="btn btn-secondary" onClick={handleSelectMmprojFile}>
                  {t("settings.btn_browse")}
                </button>
              </div>
            </div>
          </section>

          <section id="settings-section-kb" className="settings-group">
            <h3>{t("settings.sub_kb")}</h3>
            <div className="form-control">
              <label>{t("settings.label_embed_model")}</label>
              <select>
                <option>{t("settings.opt_multilingual_e5")}</option>
              </select>
            </div>
          </section>
    </>
  );
};
