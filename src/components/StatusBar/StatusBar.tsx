import { memo } from "react";
import { useTranslation } from "react-i18next";
import "./StatusBar.css";
import { BoxIcon } from "../Icons";

interface StatusBarProps {
  modelStatus: string;
  modelPath?: string | null;
}

export const StatusBar = memo(function StatusBar({ modelStatus, modelPath }: StatusBarProps) {
  const { t } = useTranslation();

  const getModelDisplayName = () => {
    if (!modelPath) return t("status_bar.local_gemma");
    const parts = modelPath.split(/[/\\]/);
    const filename = parts[parts.length - 1];
    return filename || t("status_bar.local_gemma");
  };

  return (
    <footer className="status-bar">
      <div className="status-left"></div>
      <div className="status-right">
        <div className="status-item">
          <BoxIcon size={12} />
          <span>{getModelDisplayName()}</span>
        </div>
        <div className="status-item">
          <div className={`status-dot-bar ${modelStatus.toLowerCase()}`}></div>
          <span>{modelStatus === "Loaded" ? "Ready" : modelStatus}</span>
        </div>
      </div>
    </footer>
  );
});

StatusBar.displayName = "StatusBar";
