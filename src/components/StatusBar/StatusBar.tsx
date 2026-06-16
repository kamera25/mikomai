import { useTranslation } from "react-i18next";
import "./StatusBar.css";

interface StatusBarProps {
  modelStatus: string;
  modelPath?: string | null;
}

export function StatusBar({ modelStatus, modelPath }: StatusBarProps) {
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
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
            <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
            <line x1="12" y1="22.08" x2="12" y2="12"></line>
          </svg>
          <span>{getModelDisplayName()}</span>
        </div>
        <div className="status-item">
          <div className={`status-dot-bar ${modelStatus.toLowerCase()}`}></div>
          <span>{modelStatus === "Loaded" ? "Ready" : modelStatus}</span>
        </div>
      </div>
    </footer>
  );
}
