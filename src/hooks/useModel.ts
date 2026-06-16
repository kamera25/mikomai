import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModelState } from "../types";

export function useModel(modelPath: string | null) {
  const [modelStatus, setModelStatus] = useState<string>("NotLoaded");

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const status = await invoke<ModelState>("get_model_status");
        if (typeof status === 'string') {
          setModelStatus(status);
        } else if (typeof status === 'object' && status !== null) {
          if ('Error' in status) {
            setModelStatus('Error');
          }
        }
      } catch (e) {
        console.error("Failed to get model status:", e);
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 2000);
    return () => clearInterval(interval);
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
