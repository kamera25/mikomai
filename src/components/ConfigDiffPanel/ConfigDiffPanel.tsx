import React, { useState, useEffect, useRef } from "react";
import "./ConfigDiffPanel.css";
import { ConfigFileIcon, CheckIcon, SwitchIcon } from "../Icons";
import { useUIContext, ConfigDiffData } from "../../contexts/UIContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ConfigDiffPanelProps {
  id: string | null;
  isOpen: boolean;
  onClose: () => void;
}

export const ConfigDiffPanel: React.FC<ConfigDiffPanelProps> = ({ id, isOpen, onClose }) => {
  const { state: uiState } = useUIContext();
  const proposedDiffData = uiState.configDiffData;

  const [phase, setPhase] = useState<"idle" | "fetching_before" | "deploying" | "verifying" | "success" | "failed">("idle");
  const [statusMessage, setStatusMessage] = useState<string>("");
  const [commitLogs, setCommitLogs] = useState<string[]>([]);
  const [verifiedDiffData, setVerifiedDiffData] = useState<ConfigDiffData | null>(null);
  const [activeTab, setActiveTab] = useState<"diff" | "logs">("diff");
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Reset state when id changes or panel opens
  useEffect(() => {
    setPhase("idle");
    setStatusMessage("");
    setCommitLogs([]);
    setVerifiedDiffData(null);
    setActiveTab("diff");
  }, [id, proposedDiffData]);

  // Listen to Tauri events from Rust backend
  useEffect(() => {
    if (!isOpen) return;

    const unlistenStatus = listen<any>("commit-status", (event) => {
      const { id: eventId, phase: newPhase, message } = event.payload;
      if (id && eventId && eventId !== id) return;

      if (newPhase) setPhase(newPhase);
      if (message) {
        setStatusMessage(message);
        setCommitLogs((prev) => [...prev, `[STATUS] ${message}`]);
      }
      if (newPhase === "deploying" || newPhase === "fetching_before") {
        setActiveTab("logs");
      }
    });

    const unlistenLog = listen<any>("commit-log", (event) => {
      const { line } = event.payload;
      if (line !== undefined && line !== null) {
        setCommitLogs((prev) => [...prev, line]);
      }
    });

    const unlistenDiffResult = listen<any>("commit-diff-result", (event) => {
      const { id: eventId, fileName, additions, deletions, diffLines, hostname, ip, status, message } = event.payload;
      if (id && eventId && eventId !== id) return;

      const formattedLines = (diffLines || []).map((l: any) => ({
        type: l.type,
        oldLine: l.old_line !== undefined ? l.old_line : l.oldLine,
        newLine: l.new_line !== undefined ? l.new_line : l.newLine,
        content: l.content,
      }));

      setVerifiedDiffData({
        fileName: fileName || "running-config",
        additions: additions || 0,
        deletions: deletions || 0,
        diffLines: formattedLines,
        hostname,
        ip,
      });

      setPhase(status === "success" ? "success" : "failed");
      if (message) setStatusMessage(message);
      setActiveTab("diff");
    });

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenLog.then((fn) => fn());
      unlistenDiffResult.then((fn) => fn());
    };
  }, [isOpen, id]);

  // Auto scroll logs
  useEffect(() => {
    if (activeTab === "logs" && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [commitLogs, activeTab]);

  const handleCommit = async () => {
    try {
      setPhase("fetching_before");
      setStatusMessage("現状のConfigを取得中...");
      setCommitLogs(["[SYSTEM] コミット要求を送信しました..."]);
      setActiveTab("logs");
      await invoke("submit_user_choice", { id, choice: "commit" });
    } catch (e) {
      console.error("Failed to submit commit choice:", e);
      setPhase("failed");
      setStatusMessage(`コミット起動エラー: ${e}`);
    }
  };

  const diffData = verifiedDiffData || proposedDiffData;

  if (!diffData && phase === "idle") {
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

  const renderStatusBadge = () => {
    switch (phase) {
      case "fetching_before":
        return <span className="diff-phase-badge info">🔄 現状Config取得中...</span>;
      case "deploying":
        return <span className="diff-phase-badge warning">🚀 Netmiko投入中...</span>;
      case "verifying":
        return <span className="diff-phase-badge info">🔍 Diff検証中...</span>;
      case "success":
        return <span className="diff-phase-badge success">✅ 投入&Diff検証完了</span>;
      case "failed":
        return <span className="diff-phase-badge error">❌ エラー発生</span>;
      default:
        return null;
    }
  };

  return (
    <div className={`config-diff-panel ${isOpen ? "open" : "collapsed"}`}>
      <div className="diff-header">
        <div className="diff-header-left">
          <span className="diff-title">{verifiedDiffData ? "反映後の検証差分" : "変更箇所"}</span>
          {diffData && (
            <span className="diff-count-badge">+{diffData.additions} -{diffData.deletions}</span>
          )}
          {renderStatusBadge()}
        </div>
        <div className="diff-header-right">
          <button className="close-btn" onClick={onClose} aria-label="Close diff panel">
            &times;
          </button>
        </div>
      </div>

      <div className="diff-content">
        <div className="diff-tabs">
          <button
            className={`diff-tab-btn ${activeTab === "diff" ? "active" : ""}`}
            onClick={() => setActiveTab("diff")}
          >
            {verifiedDiffData ? "検証Diff" : "変更箇所 (Diff)"}
          </button>
          <button
            className={`diff-tab-btn ${activeTab === "logs" ? "active" : ""}`}
            onClick={() => setActiveTab("logs")}
          >
            投入ログ ({commitLogs.length})
          </button>
        </div>

        {activeTab === "diff" && diffData && (
          <>
            <div className="diff-file-header" style={{ flexDirection: "column", alignItems: "flex-start", gap: "4px", padding: "12px 16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", width: "100%", alignItems: "center" }}>
                <div className="file-info">
                  <span className="file-icon" style={{ display: "flex", alignItems: "center", color: "var(--text-secondary)" }}>
                    <ConfigFileIcon size={16} />
                  </span>
                  <span className="file-name">
                    {diffData.fileName === "cisco.conf" ? "running-config" : diffData.fileName}
                  </span>
                </div>
                <div className="file-actions">
                  <span className="diff-stat-addition">+{diffData.additions}</span>
                  <span className="diff-stat-deletion">-{diffData.deletions}</span>
                </div>
              </div>
              {(diffData.hostname || diffData.ip) && (
                <div className="file-device-info" style={{ fontSize: "0.85em", color: "var(--text-secondary)", paddingLeft: "24px" }}>
                  {diffData.hostname || ""}{diffData.ip ? ` (${diffData.ip})` : ""}
                </div>
              )}
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
          </>
        )}

        {activeTab === "logs" && (
          <div className="diff-logs-wrapper">
            <div className="diff-logs-content">
              {commitLogs.length === 0 ? (
                <div style={{ color: "#6b7280", fontStyle: "italic" }}>投入ログがここにリアルタイム表示されます...</div>
              ) : (
                commitLogs.map((log, index) => {
                  let className = "diff-log-line";
                  if (log.startsWith("[STATUS]")) className = "diff-log-line diff-log-status";
                  else if (log.startsWith("[SYSTEM]")) className = "diff-log-line diff-log-system";
                  else if (log.includes("Error") || log.includes("FAILED")) className = "diff-log-line diff-log-error";

                  return (
                    <div key={index} className={className}>
                      {log}
                    </div>
                  );
                })
              )}
              <div ref={logsEndRef} />
            </div>
          </div>
        )}
      </div>

      <div className="diff-footer">
        {phase === "idle" ? (
          <>
            <button className="btn btn-secondary" onClick={onClose}>中止</button>
            <button className="btn btn-primary" onClick={handleCommit}>
              <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
                <span>コミット</span>
                <div style={{ position: "relative", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
                  <SwitchIcon size={16} />
                  <CheckIcon
                    size={10}
                    strokeWidth={4}
                    style={{
                      position: "absolute",
                      top: "-4px",
                      right: "-4px",
                      backgroundColor: "#8becccff",
                      color: "#ffffff",
                      borderRadius: "50%",
                      padding: "1px",
                      border: "1.5px solid var(--bg-primary, #1e1e1e)",
                      boxSizing: "content-box"
                    }}
                  />
                </div>
              </div>
            </button>
          </>
        ) : phase === "success" || phase === "failed" ? (
          <div style={{ display: "flex", justifyContent: "space-between", width: "100%", alignItems: "center" }}>
            <span style={{ fontSize: "0.85rem", color: phase === "success" ? "var(--success)" : "var(--danger)" }}>
              {statusMessage || (phase === "success" ? "コミット完了" : "エラー")}
            </span>
            <button className="btn btn-secondary" onClick={onClose}>閉じる</button>
          </div>
        ) : (
          <div style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
            <div className="spinner" style={{ width: "16px", height: "16px", border: "2px solid rgba(255,255,255,0.3)", borderTopColor: "var(--primary)", borderRadius: "50%", animation: "spin 1s linear infinite" }} />
            <span style={{ fontSize: "0.85rem", color: "var(--text-secondary)", flexGrow: 1 }}>{statusMessage}</span>
            <button className="btn btn-secondary" disabled style={{ opacity: 0.6 }}>処理中...</button>
          </div>
        )}
      </div>
    </div>
  );
};

