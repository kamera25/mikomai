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
  const [cacheExpiryMinutes, setCacheExpiryMinutes] = useState<number>(DEFAULT_CACHE_EXPIRY_MINUTES);
  const [dbPath, setDbPath] = useState<string>(DEFAULT_DB_PATH);
  const [ipVersion, setIpVersion] = useState<string>(DEFAULT_IP_VERSION);
  const [consolePort, setConsolePort] = useState<string | null>(null);
  const [consoleBaudRate, setConsoleBaudRate] = useState<number>(9600);
  const [preloadInvestigate, setPreloadInvestigate] = useState<boolean>(true);
  const [preloadKnowledge, setPreloadKnowledge] = useState<boolean>(true);
  const [preloadAnalysis, setPreloadAnalysis] = useState<boolean>(true);
  const [preloadRag, setPreloadRag] = useState<boolean>(true);
  const [recentIPs, setRecentIPs] = useState<string[]>([]);

  // Load settings from backend
  useEffect(() => {
    const initSettings = async () => {
      try {
        const settings = await invoke<SystemSettings>("load_settings");
        if (settings) {
          if (settings.historyLimit !== undefined) setHistoryLimit(settings.historyLimit);
          if (settings.temperature !== undefined) setTemperature(settings.temperature);
          if (settings.repetitionPenalty !== undefined) setRepetitionPenalty(settings.repetitionPenalty);
          if (settings.modelPath !== undefined) setModelPath(settings.modelPath);
          if (settings.recentIps !== undefined) setRecentIPs(settings.recentIps);
          if (settings.mcpTimeout !== undefined) setMcpTimeout(settings.mcpTimeout);
          if (settings.cacheExpiryMinutes !== undefined) setCacheExpiryMinutes(settings.cacheExpiryMinutes);
          if (settings.dbPath !== undefined) setDbPath(settings.dbPath);
          if (settings.ipVersion !== undefined) setIpVersion(settings.ipVersion);
          if (settings.consolePort !== undefined) setConsolePort(settings.consolePort);
          if (settings.consoleBaudRate !== undefined) setConsoleBaudRate(settings.consoleBaudRate);
          if (settings.preloadInvestigate !== undefined) setPreloadInvestigate(settings.preloadInvestigate);
          if (settings.preloadKnowledge !== undefined) setPreloadKnowledge(settings.preloadKnowledge);
          if (settings.preloadAnalysis !== undefined) setPreloadAnalysis(settings.preloadAnalysis);
          if (settings.preloadRag !== undefined) setPreloadRag(settings.preloadRag);
        }
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    };
    initSettings();
  }, []);

  const saveAllSettings = async (overrides: Partial<SystemSettings>) => {
    const payload = {
      historyLimit: overrides.historyLimit !== undefined ? overrides.historyLimit : historyLimit,
      temperature: overrides.temperature !== undefined ? overrides.temperature : temperature,
      repetitionPenalty: overrides.repetitionPenalty !== undefined ? overrides.repetitionPenalty : repetitionPenalty,
      modelPath: overrides.modelPath !== undefined ? overrides.modelPath : modelPath,
      recentIps: overrides.recentIps !== undefined ? overrides.recentIps : recentIPs,
      mcpTimeout: overrides.mcpTimeout !== undefined ? overrides.mcpTimeout : mcpTimeout,
      cacheExpiryMinutes: overrides.cacheExpiryMinutes !== undefined ? overrides.cacheExpiryMinutes : cacheExpiryMinutes,
      dbPath: overrides.dbPath !== undefined ? overrides.dbPath : dbPath,
      ipVersion: overrides.ipVersion !== undefined ? overrides.ipVersion : ipVersion,
      consolePort: overrides.consolePort !== undefined ? overrides.consolePort : consolePort,
      consoleBaudRate: overrides.consoleBaudRate !== undefined ? overrides.consoleBaudRate : consoleBaudRate,
      preloadInvestigate: overrides.preloadInvestigate !== undefined ? overrides.preloadInvestigate : preloadInvestigate,
      preloadKnowledge: overrides.preloadKnowledge !== undefined ? overrides.preloadKnowledge : preloadKnowledge,
      preloadAnalysis: overrides.preloadAnalysis !== undefined ? overrides.preloadAnalysis : preloadAnalysis,
      preloadRag: overrides.preloadRag !== undefined ? overrides.preloadRag : preloadRag,
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
    recentIPs,
    setRecentIPs,
    saveAllSettings,
  };
}
