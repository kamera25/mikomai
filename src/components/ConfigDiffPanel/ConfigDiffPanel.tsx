import React from "react";
import "./ConfigDiffPanel.css";
import { ConfigFileIcon } from "../Icons";

interface ConfigDiffPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ConfigDiffPanel: React.FC<ConfigDiffPanelProps> = ({ isOpen, onClose }) => {
  // Mock diff data structure
  const diffLines = [
    { type: "normal", oldLine: 1, newLine: 1, content: "{" },
    { type: "delete", oldLine: 2, newLine: null, content: '  "model": "gpt-4-turbo",' },
    { type: "insert", oldLine: null, newLine: 2, content: '  "model": "gpt-5-codex-pro",' },
    { type: "delete", oldLine: 3, newLine: null, content: '  "temperature": 0.7,' },
    { type: "insert", oldLine: null, newLine: 3, content: '  "temperature": 0.2,' },
    { type: "normal", oldLine: 4, newLine: 4, content: '  "max_tokens": 4096,' },
    { type: "delete", oldLine: 5, newLine: null, content: '  "system_prompt": "You are a helpful coding assistant designed to write clean code."' },
    { type: "insert", oldLine: null, newLine: 5, content: '  "system_prompt": "You are Antigravity, a premium agentic AI coding assistant designed by Google DeepMind."' },
    { type: "normal", oldLine: 6, newLine: 6, content: "}" }
  ];

  return (
    <div className={`config-diff-panel ${isOpen ? "open" : "collapsed"}`}>
      <div className="diff-header">
        <div className="diff-header-left">
          <span className="diff-title">変更箇所</span>
          <span className="diff-count-badge">+3 -3</span>
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
            <span className="file-name">running-config</span>
          </div>
          <div className="file-actions">
            <span className="diff-stat-addition">+3</span>
            <span className="diff-stat-deletion">-3</span>
          </div>
        </div>

        <div className="diff-viewer-wrapper">
          <table className="diff-table">
            <tbody>
              {diffLines.map((line, idx) => {
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
        <button className="btn btn-primary" onClick={onClose}>コミット</button>
      </div>
    </div>
  );
};
