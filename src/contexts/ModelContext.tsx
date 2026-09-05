import React, { createContext, useContext, useReducer, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ModelState as BackendModelState } from "../types";
import { useSettingsContext } from "./SettingsContext";

export interface ModelState {
  modelStatus: string;
  loadedModelPath: string | null;
}

export type ModelAction =
  | { type: "SET_STATUS"; payload: string }
  | { type: "SET_LOADED_MODEL_PATH"; payload: string | null };

const initialState: ModelState = {
  modelStatus: "NotLoaded",
  loadedModelPath: null,
};

function modelReducer(state: ModelState, action: ModelAction): ModelState {
  switch (action.type) {
    case "SET_STATUS":
      return {
        ...state,
        modelStatus: action.payload,
        loadedModelPath: action.payload === "NotLoaded" ? null : state.loadedModelPath,
      };
    case "SET_LOADED_MODEL_PATH":
      return { ...state, loadedModelPath: action.payload };
    default:
      return state;
  }
}

interface ModelContextType {
  state: ModelState;
  handleLoadModel: () => Promise<void>;
}

const ModelContext = createContext<ModelContextType | undefined>(undefined);

export const ModelProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, dispatch] = useReducer(modelReducer, initialState);
  const { modelPath } = useSettingsContext();

  const updateStatus = (status: BackendModelState) => {
    if (typeof status === "string") {
      dispatch({ type: "SET_STATUS", payload: status });
    } else if (typeof status === "object" && status !== null) {
      if ("Error" in status) {
        dispatch({ type: "SET_STATUS", payload: "Error" });
      }
    }
  };

  useEffect(() => {
    let active = true;
    let unlistenStatusFn: (() => void) | null = null;
    let unlistenLoadedFn: (() => void) | null = null;

    const checkStatus = async () => {
      try {
        const [status, loadedPath] = await Promise.all([
          invoke<BackendModelState>("get_model_status"),
          invoke<string | null>("get_loaded_model_path").catch(() => null),
        ]);
        if (active) {
          updateStatus(status);
          if (loadedPath) {
            dispatch({ type: "SET_LOADED_MODEL_PATH", payload: loadedPath });
          }
        }
      } catch (e) {
        console.error("Failed to get model status:", e);
      }
    };

    checkStatus();

    const setupListeners = async () => {
      try {
        const unlistenStatus = await listen<BackendModelState>("model-status-changed", (event) => {
          if (active) {
            updateStatus(event.payload);
          }
        });
        if (!active) {
          unlistenStatus();
        } else {
          unlistenStatusFn = unlistenStatus;
        }

        const unlistenLoaded = await listen<string>("model-loaded", (event) => {
          if (active) {
            dispatch({ type: "SET_LOADED_MODEL_PATH", payload: event.payload });
          }
        });
        if (!active) {
          unlistenLoaded();
        } else {
          unlistenLoadedFn = unlistenLoaded;
        }
      } catch (err) {
        console.error("Failed to set up model listeners:", err);
      }
    };

    setupListeners();

    return () => {
      active = false;
      if (unlistenStatusFn) {
        unlistenStatusFn();
      }
      if (unlistenLoadedFn) {
        unlistenLoadedFn();
      }
    };
  }, []);

  const handleLoadModel = async () => {
    if (!modelPath) return;
    try {
      dispatch({ type: "SET_STATUS", payload: "Loading" });
      await invoke("load_model", { path: modelPath });
      dispatch({ type: "SET_LOADED_MODEL_PATH", payload: modelPath });
      dispatch({ type: "SET_STATUS", payload: "Loaded" });
    } catch (e) {
      console.error("Failed to load model:", e);
      dispatch({ type: "SET_STATUS", payload: "Error" });
    }
  };

  return (
    <ModelContext.Provider value={{ state, handleLoadModel }}>
      {children}
    </ModelContext.Provider>
  );
};

export const useModelContext = () => {
  const context = useContext(ModelContext);
  if (!context) {
    throw new Error("useModelContext must be used within a ModelProvider");
  }
  return context;
};
