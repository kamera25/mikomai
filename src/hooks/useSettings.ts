import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SystemSettings } from "../types";
import {
  DEFAULT_HISTORY_LIMIT,
  DEFAULT_TEMPERATURE,
  DEFAULT_REPETITION_PENALTY,
  DEFAULT_MODEL_PATH,
  DEFAULT_MCP_TIMEOUT,
  DEFAULT_DB_PATH,
  DEFAULT_IP_VERSION,
  DEFAULT_CACHE_EXPIRY_MINUTES,
} from "../constants/defaults";

export function useSettings() {
  const [historyLimit, setHistoryLimit] = useState<number>(DEFAULT_HISTORY_LIMIT);
  const [temperature, setTemperature] = useState<number>(DEFAULT_TEMPERATURE);
  const [repetitionPenalty, setRepetitionPenalty] = useState<number>(DEFAULT_REPETITION_PENALTY);
  const [modelPath, setModelPath] = useState<string | null>(DEFAULT_MODEL_PATH);
  const [mcpTimeout, setMcpTimeout] = useState<number>(DEFAULT_MCP_TIMEOUT);
  const [cacheExpiryMinutes, setCacheExpiryMinutes] = useState<number>(
    DEFAULT_CACHE_EXPIRY_MINUTES
  );
  const [dbPath, setDbPath] = useState<string>(DEFAULT_DB_PATH);
  const [ipVersion, setIpVersion] = useState<string>(DEFAULT_IP_VERSION);
  const [consolePort, setConsolePort] = useState<string | null>(null);
  const [consoleBaudRate, setConsoleBaudRate] = useState<number>(9600);
  const [preloadInvestigate, setPreloadInvestigate] = useState<boolean>(false);
  const [preloadKnowledge, setPreloadKnowledge] = useState<boolean>(false);
  const [preloadAnalysis, setPreloadAnalysis] = useState<boolean>(false);
  const [preloadRag, setPreloadRag] = useState<boolean>(false);
  const [preloadPlotter, setPreloadPlotter] = useState<boolean>(false);
  const [preloadBuilder, setPreloadBuilder] = useState<boolean>(false);
  const [preloadSummarization, setPreloadSummarization] = useState<boolean>(false);
  const [visionEnabled, setVisionEnabled] = useState<boolean>(false);
  const [mmprojPath, setMmprojPath] = useState<string | null>(null);
  const [recentIPs, setRecentIPs] = useState<string[]>([]);

  // Load settings from backend
  useEffect(() => {
    const initSettings = async () => {
      try {
        const settings = await invoke<SystemSettings>("load_settings");
        if (settings) {
          if (settings.historyLimit !== undefined) setHistoryLimit(settings.historyLimit);
          if (settings.temperature !== undefined) setTemperature(settings.temperature);
          if (settings.repetitionPenalty !== undefined)
            setRepetitionPenalty(settings.repetitionPenalty);
          if (settings.modelPath !== undefined) setModelPath(settings.modelPath);
          if (settings.recentIps !== undefined) setRecentIPs(settings.recentIps);
          if (settings.mcpTimeout !== undefined) setMcpTimeout(settings.mcpTimeout);
          if (settings.cacheExpiryMinutes !== undefined)
            setCacheExpiryMinutes(settings.cacheExpiryMinutes);
          if (settings.dbPath !== undefined) setDbPath(settings.dbPath);
          if (settings.ipVersion !== undefined) setIpVersion(settings.ipVersion);
          if (settings.consolePort !== undefined) setConsolePort(settings.consolePort);
          if (settings.consoleBaudRate !== undefined) setConsoleBaudRate(settings.consoleBaudRate);
          if (settings.preloadInvestigate !== undefined)
            setPreloadInvestigate(settings.preloadInvestigate);
          if (settings.preloadKnowledge !== undefined)
            setPreloadKnowledge(settings.preloadKnowledge);
          if (settings.preloadAnalysis !== undefined) setPreloadAnalysis(settings.preloadAnalysis);
          if (settings.preloadRag !== undefined) setPreloadRag(settings.preloadRag);
          if (settings.preloadPlotter !== undefined) setPreloadPlotter(settings.preloadPlotter);
          if (settings.preloadBuilder !== undefined) setPreloadBuilder(settings.preloadBuilder);
          if (settings.preloadSummarization !== undefined)
            setPreloadSummarization(settings.preloadSummarization);
          if (settings.visionEnabled !== undefined) setVisionEnabled(settings.visionEnabled);
          if (settings.mmprojPath !== undefined) setMmprojPath(settings.mmprojPath);
        }
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    };
    initSettings();
  }, []);

  const saveAllSettings = async (overrides: Partial<SystemSettings>) => {
    const updatedHistoryLimit = overrides.historyLimit !== undefined ? overrides.historyLimit : historyLimit;
    const updatedTemperature = overrides.temperature !== undefined ? overrides.temperature : temperature;
    const updatedRepetitionPenalty = overrides.repetitionPenalty !== undefined ? overrides.repetitionPenalty : repetitionPenalty;
    const updatedModelPath = overrides.modelPath !== undefined ? overrides.modelPath : modelPath;
    const updatedRecentIps = overrides.recentIps !== undefined ? overrides.recentIps : recentIPs;
    const updatedMcpTimeout = overrides.mcpTimeout !== undefined ? overrides.mcpTimeout : mcpTimeout;
    const updatedCacheExpiryMinutes = overrides.cacheExpiryMinutes !== undefined ? overrides.cacheExpiryMinutes : cacheExpiryMinutes;
    const updatedDbPath = overrides.dbPath !== undefined ? overrides.dbPath : dbPath;
    const updatedIpVersion = overrides.ipVersion !== undefined ? overrides.ipVersion : ipVersion;
    const updatedConsolePort = overrides.consolePort !== undefined ? overrides.consolePort : consolePort;
    const updatedConsoleBaudRate = overrides.consoleBaudRate !== undefined ? overrides.consoleBaudRate : consoleBaudRate;
    const updatedPreloadInvestigate = overrides.preloadInvestigate !== undefined ? overrides.preloadInvestigate : preloadInvestigate;
    const updatedPreloadKnowledge = overrides.preloadKnowledge !== undefined ? overrides.preloadKnowledge : preloadKnowledge;
    const updatedPreloadAnalysis = overrides.preloadAnalysis !== undefined ? overrides.preloadAnalysis : preloadAnalysis;
    const updatedPreloadRag = overrides.preloadRag !== undefined ? overrides.preloadRag : preloadRag;
    const updatedPreloadPlotter = overrides.preloadPlotter !== undefined ? overrides.preloadPlotter : preloadPlotter;
    const updatedPreloadBuilder = overrides.preloadBuilder !== undefined ? overrides.preloadBuilder : preloadBuilder;
    const updatedPreloadSummarization = overrides.preloadSummarization !== undefined ? overrides.preloadSummarization : preloadSummarization;
    const updatedVisionEnabled = overrides.visionEnabled !== undefined ? overrides.visionEnabled : visionEnabled;
    const updatedMmprojPath = overrides.mmprojPath !== undefined ? overrides.mmprojPath : mmprojPath;

    if (overrides.historyLimit !== undefined) setHistoryLimit(overrides.historyLimit);
    if (overrides.temperature !== undefined) setTemperature(overrides.temperature);
    if (overrides.repetitionPenalty !== undefined) setRepetitionPenalty(overrides.repetitionPenalty);
    if (overrides.modelPath !== undefined) setModelPath(overrides.modelPath);
    if (overrides.recentIps !== undefined) setRecentIPs(overrides.recentIps);
    if (overrides.mcpTimeout !== undefined) setMcpTimeout(overrides.mcpTimeout);
    if (overrides.cacheExpiryMinutes !== undefined) setCacheExpiryMinutes(overrides.cacheExpiryMinutes);
    if (overrides.dbPath !== undefined) setDbPath(overrides.dbPath);
    if (overrides.ipVersion !== undefined) setIpVersion(overrides.ipVersion);
    if (overrides.consolePort !== undefined) setConsolePort(overrides.consolePort);
    if (overrides.consoleBaudRate !== undefined) setConsoleBaudRate(overrides.consoleBaudRate);
    if (overrides.preloadInvestigate !== undefined) setPreloadInvestigate(overrides.preloadInvestigate);
    if (overrides.preloadKnowledge !== undefined) setPreloadKnowledge(overrides.preloadKnowledge);
    if (overrides.preloadAnalysis !== undefined) setPreloadAnalysis(overrides.preloadAnalysis);
    if (overrides.preloadRag !== undefined) setPreloadRag(overrides.preloadRag);
    if (overrides.preloadPlotter !== undefined) setPreloadPlotter(overrides.preloadPlotter);
    if (overrides.preloadBuilder !== undefined) setPreloadBuilder(overrides.preloadBuilder);
    if (overrides.preloadSummarization !== undefined) setPreloadSummarization(overrides.preloadSummarization);
    if (overrides.visionEnabled !== undefined) setVisionEnabled(overrides.visionEnabled);
    if (overrides.mmprojPath !== undefined) setMmprojPath(overrides.mmprojPath);

    const payload = {
      historyLimit: updatedHistoryLimit,
      temperature: updatedTemperature,
      repetitionPenalty: updatedRepetitionPenalty,
      modelPath: updatedModelPath,
      recentIps: updatedRecentIps,
      mcpTimeout: updatedMcpTimeout,
      cacheExpiryMinutes: updatedCacheExpiryMinutes,
      dbPath: updatedDbPath,
      ipVersion: updatedIpVersion,
      consolePort: updatedConsolePort,
      consoleBaudRate: updatedConsoleBaudRate,
      preloadInvestigate: updatedPreloadInvestigate,
      preloadKnowledge: updatedPreloadKnowledge,
      preloadAnalysis: updatedPreloadAnalysis,
      preloadRag: updatedPreloadRag,
      preloadPlotter: updatedPreloadPlotter,
      preloadBuilder: updatedPreloadBuilder,
      preloadSummarization: updatedPreloadSummarization,
      visionEnabled: updatedVisionEnabled,
      mmprojPath: updatedMmprojPath,
    };
    try {
      await invoke("save_settings", { settings: payload });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  };

  return {
    historyLimit,
    setHistoryLimit,
    temperature,
    setTemperature,
    repetitionPenalty,
    setRepetitionPenalty,
    modelPath,
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
    preloadPlotter,
    setPreloadPlotter,
    preloadBuilder,
    setPreloadBuilder,
    preloadSummarization,
    setPreloadSummarization,
    visionEnabled,
    setVisionEnabled,
    mmprojPath,
    setMmprojPath,
    recentIPs,
    setRecentIPs,
    saveAllSettings,
  };
}
