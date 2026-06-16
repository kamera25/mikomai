import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ModelState } from "../types";

export function useModel(modelPath: string | null) {
  const [modelStatus, setModelStatus] = useState<string>("NotLoaded");

  useEffect(() => {
    let active = true;
    let unlistenFn: (() => void) | null = null;

    const updateStatus = (status: ModelState) => {
      if (typeof status === "string") {
        setModelStatus(status);
      } else if (typeof status === "object" && status !== null) {
        if ("Error" in status) {
          setModelStatus("Error");
        }
      }
    };

    const checkStatus = async () => {
      try {
        const status = await invoke<ModelState>("get_model_status");
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
        const unlisten = await listen<ModelState>("model-status-changed", (event) => {
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
      setModelStatus("Loading");
      await invoke("load_model", { path: modelPath });
      setModelStatus("Loaded");
    } catch (e) {
      console.error("Failed to load model:", e);
      setModelStatus("Error");
    }
  };

  return {
    modelStatus,
    setModelStatus,
    handleLoadModel,
  };
}
