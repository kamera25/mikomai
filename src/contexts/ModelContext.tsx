import React, { createContext, useContext, useReducer, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ModelState as BackendModelState } from "../types";
import { useSettingsContext } from "./SettingsContext";

export interface ModelState {
  modelStatus: string;
}

export type ModelAction =
  | { type: "SET_STATUS"; payload: string };

const initialState: ModelState = {
  modelStatus: "NotLoaded",
};

function modelReducer(state: ModelState, action: ModelAction): ModelState {
  switch (action.type) {
    case "SET_STATUS":
      return { ...state, modelStatus: action.payload };
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
    let unlistenFn: (() => void) | null = null;

    const checkStatus = async () => {
      try {
        const status = await invoke<BackendModelState>("get_model_status");
        if (active) {
          updateStatus(status);
        }
      } catch (e) {
        console.error("Failed to get model status:", e);
      }
    };

    checkStatus();

    const setupListener = async () => {
      try {
        const unlisten = await listen<BackendModelState>("model-status-changed", (event) => {
          if (active) {
            updateStatus(event.payload);
          }
        });
        if (!active) {
          unlisten();
        } else {
          unlistenFn = unlisten;
        }
      } catch (err) {
        console.error("Failed to set up model status listener:", err);
      }
    };

    setupListener();

    return () => {
      active = false;
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  const handleLoadModel = async () => {
    if (!modelPath) return;
    try {
      dispatch({ type: "SET_STATUS", payload: "Loading" });
      await invoke("load_model", { path: modelPath });
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
