import { useState, useEffect, useCallback } from "react";
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

export interface SettingsState {
  historyLimit: number;
  temperature: number;
  repetitionPenalty: number;
  modelPath: string | null;
  mcpTimeout: number;
  cacheExpiryMinutes: number;
  dbPath: string;
  ipVersion: string;
  consolePort: string | null;
  consoleBaudRate: number;
  preloadInvestigate: boolean;
  preloadKnowledge: boolean;
  preloadAnalysis: boolean;
  preloadRag: boolean;
  preloadPlotter: boolean;
  preloadBuilder: boolean;
  preloadSummarization: boolean;
  visionEnabled: boolean;
  autoDryRun: boolean;
  mmprojPath: string | null;
  recentIps: string[];
}

const INITIAL_SETTINGS_STATE: SettingsState = {
  historyLimit: DEFAULT_HISTORY_LIMIT,
  temperature: DEFAULT_TEMPERATURE,
  repetitionPenalty: DEFAULT_REPETITION_PENALTY,
  modelPath: DEFAULT_MODEL_PATH,
  mcpTimeout: DEFAULT_MCP_TIMEOUT,
  cacheExpiryMinutes: DEFAULT_CACHE_EXPIRY_MINUTES,
  dbPath: DEFAULT_DB_PATH,
  ipVersion: DEFAULT_IP_VERSION,
  consolePort: null,
  consoleBaudRate: 9600,
  preloadInvestigate: false,
  preloadKnowledge: false,
  preloadAnalysis: false,
  preloadRag: false,
  preloadPlotter: false,
  preloadBuilder: false,
  preloadSummarization: false,
  visionEnabled: false,
  autoDryRun: false,
  mmprojPath: null,
  recentIps: [],
};

export function useSettings() {
  const [settings, setSettings] = useState<SettingsState>(INITIAL_SETTINGS_STATE);

  // Load settings from backend
  useEffect(() => {
    const initSettings = async () => {
      try {
        const loaded = await invoke<SystemSettings>("load_settings");
        if (loaded) {
          setSettings((prev) => ({
            ...prev,
            ...(loaded.historyLimit !== undefined && { historyLimit: loaded.historyLimit }),
            ...(loaded.temperature !== undefined && { temperature: loaded.temperature }),
            ...(loaded.repetitionPenalty !== undefined && { repetitionPenalty: loaded.repetitionPenalty }),
            ...(loaded.modelPath !== undefined && { modelPath: loaded.modelPath }),
            ...(loaded.recentIps !== undefined && { recentIps: loaded.recentIps }),
            ...(loaded.mcpTimeout !== undefined && { mcpTimeout: loaded.mcpTimeout }),
            ...(loaded.cacheExpiryMinutes !== undefined && { cacheExpiryMinutes: loaded.cacheExpiryMinutes }),
            ...(loaded.dbPath !== undefined && { dbPath: loaded.dbPath }),
            ...(loaded.ipVersion !== undefined && { ipVersion: loaded.ipVersion }),
            ...(loaded.consolePort !== undefined && { consolePort: loaded.consolePort }),
            ...(loaded.consoleBaudRate !== undefined && { consoleBaudRate: loaded.consoleBaudRate }),
            ...(loaded.preloadInvestigate !== undefined && { preloadInvestigate: loaded.preloadInvestigate }),
            ...(loaded.preloadKnowledge !== undefined && { preloadKnowledge: loaded.preloadKnowledge }),
            ...(loaded.preloadAnalysis !== undefined && { preloadAnalysis: loaded.preloadAnalysis }),
            ...(loaded.preloadRag !== undefined && { preloadRag: loaded.preloadRag }),
            ...(loaded.preloadPlotter !== undefined && { preloadPlotter: loaded.preloadPlotter }),
            ...(loaded.preloadBuilder !== undefined && { preloadBuilder: loaded.preloadBuilder }),
            ...(loaded.preloadSummarization !== undefined && { preloadSummarization: loaded.preloadSummarization }),
            ...(loaded.visionEnabled !== undefined && { visionEnabled: loaded.visionEnabled }),
            ...(loaded.autoDryRun !== undefined && { autoDryRun: loaded.autoDryRun }),
            ...(loaded.mmprojPath !== undefined && { mmprojPath: loaded.mmprojPath }),
          }));
        }
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    };
    initSettings();
  }, []);

  const updateSetting = useCallback(<K extends keyof SettingsState>(key: K, value: SettingsState[K] | ((prev: SettingsState[K]) => SettingsState[K])) => {
    setSettings((prev) => ({
      ...prev,
      [key]: typeof value === "function" ? (value as (prev: SettingsState[K]) => SettingsState[K])(prev[key]) : value,
    }));
  }, []);

  const saveAllSettings = async (overrides: Partial<SystemSettings>) => {
    const updated: SettingsState = {
      ...settings,
      ...overrides,
      ...(overrides.recentIps !== undefined && { recentIps: overrides.recentIps }),
    };

    setSettings(updated);

    const payload = {
      historyLimit: updated.historyLimit,
      temperature: updated.temperature,
      repetitionPenalty: updated.repetitionPenalty,
      modelPath: updated.modelPath,
      recentIps: updated.recentIps,
      mcpTimeout: updated.mcpTimeout,
      cacheExpiryMinutes: updated.cacheExpiryMinutes,
      dbPath: updated.dbPath,
      ipVersion: updated.ipVersion,
      consolePort: updated.consolePort,
      consoleBaudRate: updated.consoleBaudRate,
      preloadInvestigate: updated.preloadInvestigate,
      preloadKnowledge: updated.preloadKnowledge,
      preloadAnalysis: updated.preloadAnalysis,
      preloadRag: updated.preloadRag,
      preloadPlotter: updated.preloadPlotter,
      preloadBuilder: updated.preloadBuilder,
      preloadSummarization: updated.preloadSummarization,
      visionEnabled: updated.visionEnabled,
      autoDryRun: updated.autoDryRun,
      mmprojPath: updated.mmprojPath,
    };

    try {
      await invoke("save_settings", { settings: payload });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  };

  return {
    historyLimit: settings.historyLimit,
    setHistoryLimit: (val: number | ((prev: number) => number)) => updateSetting("historyLimit", val),
    temperature: settings.temperature,
    setTemperature: (val: number | ((prev: number) => number)) => updateSetting("temperature", val),
    repetitionPenalty: settings.repetitionPenalty,
    setRepetitionPenalty: (val: number | ((prev: number) => number)) => updateSetting("repetitionPenalty", val),
    modelPath: settings.modelPath,
    setModelPath: (val: string | null | ((prev: string | null) => string | null)) => updateSetting("modelPath", val),
    mcpTimeout: settings.mcpTimeout,
    setMcpTimeout: (val: number | ((prev: number) => number)) => updateSetting("mcpTimeout", val),
    cacheExpiryMinutes: settings.cacheExpiryMinutes,
    setCacheExpiryMinutes: (val: number | ((prev: number) => number)) => updateSetting("cacheExpiryMinutes", val),
    dbPath: settings.dbPath,
    setDbPath: (val: string | ((prev: string) => string)) => updateSetting("dbPath", val),
    ipVersion: settings.ipVersion,
    setIpVersion: (val: string | ((prev: string) => string)) => updateSetting("ipVersion", val),
    consolePort: settings.consolePort,
    setConsolePort: (val: string | null | ((prev: string | null) => string | null)) => updateSetting("consolePort", val),
    consoleBaudRate: settings.consoleBaudRate,
    setConsoleBaudRate: (val: number | ((prev: number) => number)) => updateSetting("consoleBaudRate", val),
    preloadInvestigate: settings.preloadInvestigate,
    setPreloadInvestigate: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadInvestigate", val),
    preloadKnowledge: settings.preloadKnowledge,
    setPreloadKnowledge: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadKnowledge", val),
    preloadAnalysis: settings.preloadAnalysis,
    setPreloadAnalysis: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadAnalysis", val),
    preloadRag: settings.preloadRag,
    setPreloadRag: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadRag", val),
    preloadPlotter: settings.preloadPlotter,
    setPreloadPlotter: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadPlotter", val),
    preloadBuilder: settings.preloadBuilder,
    setPreloadBuilder: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadBuilder", val),
    preloadSummarization: settings.preloadSummarization,
    setPreloadSummarization: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("preloadSummarization", val),
    visionEnabled: settings.visionEnabled,
    setVisionEnabled: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("visionEnabled", val),
    autoDryRun: settings.autoDryRun,
    setAutoDryRun: (val: boolean | ((prev: boolean) => boolean)) => updateSetting("autoDryRun", val),
    mmprojPath: settings.mmprojPath,
    setMmprojPath: (val: string | null | ((prev: string | null) => string | null)) => updateSetting("mmprojPath", val),
    recentIPs: settings.recentIps,
    setRecentIPs: (val: string[] | ((prev: string[]) => string[])) => updateSetting("recentIps", val),
    saveAllSettings,
  };
}
