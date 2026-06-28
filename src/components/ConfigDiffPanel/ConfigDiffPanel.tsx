import React from "react";
import "./ConfigDiffPanel.css";
import { ConfigFileIcon } from "../Icons";
import { useUIContext } from "../../contexts/UIContext";
import { invoke } from "@tauri-apps/api/core";

interface ConfigDiffPanelProps {
  id: string | null;
  isOpen: boolean;
  onClose: () => void;
}

export const ConfigDiffPanel: React.FC<ConfigDiffPanelProps> = ({ id, isOpen, onClose }) => {
  const { state: uiState } = useUIContext();
  const diffData = uiState.configDiffData;

  if (!diffData) {
    return (
      <div className={`config-diff-panel ${isOpen ? "open" : "collapsed"}`}>
        <div className="diff-header">
          <div className="diff-header-left">
            <span className="diff-title">変更箇所</span>
          </div>
          <div className="diff-header-right">
            <button className="close-btn" onClick={onClose} aria-label="Close diff panel">
              &times;
            </button>
          </div>
        </div>
        <div className="diff-content" style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "var(--text-secondary)", padding: "20px", textAlign: "center" }}>
          <div>表示できるConfigの変更点はありません。</div>
        </div>
      </div>
    );
  }

  return (
    <div className={`config-diff-panel ${isOpen ? "open" : "collapsed"}`}>
      <div className="diff-header">
        <div className="diff-header-left">
          <span className="diff-title">変更箇所</span>
          <span className="diff-count-badge">+{diffData.additions} -{diffData.deletions}</span>
        </div>
        <div className="diff-header-right">
          <button className="btn btn-secondary btn-sm review-btn">レビュー</button>
          <button className="close-btn" onClick={onClose} aria-label="Close diff panel">
            &times;
          </button>
        </div>
      </div>

      <div className="diff-content">
        <div className="diff-file-header">
          <div className="file-info">
            <span className="file-icon" style={{ display: "flex", alignItems: "center", color: "var(--text-secondary)" }}>
              <ConfigFileIcon size={16} />
            </span>
            <span className="file-name">{diffData.fileName}</span>
          </div>
          <div className="file-actions">
            <span className="diff-stat-addition">+{diffData.additions}</span>
            <span className="diff-stat-deletion">-{diffData.deletions}</span>
          </div>
        </div>

        <div className="diff-viewer-wrapper">
          <table className="diff-table">
            <tbody>
              {diffData.diffLines.map((line, idx) => {
                let rowClass = "diff-row-normal";
                let prefix = " ";
                if (line.type === "delete") {
                  rowClass = "diff-row-delete";
                  prefix = "-";
                } else if (line.type === "insert") {
                  rowClass = "diff-row-insert";
                  prefix = "+";
                }

                return (
                  <tr key={idx} className={rowClass}>
                    <td className="diff-line-number diff-line-old">
                      {line.oldLine !== null ? line.oldLine : ""}
                    </td>
                    <td className="diff-line-number diff-line-new">
                      {line.newLine !== null ? line.newLine : ""}
                    </td>
                    <td className="diff-line-prefix">{prefix}</td>
                    <td className="diff-line-content">
                      <code>{line.content}</code>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <div className="diff-footer">
        <button className="btn btn-secondary" onClick={onClose}>中止</button>
        <button className="btn btn-primary" onClick={async () => {
          try {
            await invoke("submit_user_choice", { id, choice: "commit" });
          } catch (e) {
            console.error("Failed to submit commit choice:", e);
          }
          onClose();
        }}>コミット</button>
      </div>
    </div>
  );
};
