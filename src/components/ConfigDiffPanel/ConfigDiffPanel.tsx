import React from "react";
import "./ConfigDiffPanel.css";
import { ConfigFileIcon, CheckIcon, SwitchIcon, RefreshIcon, FlaskIcon, RocketIcon, SearchIcon, AlertCircleIcon, ChevronIcon } from "../Icons";
import { useUIContext } from "../../contexts/UIContext";
import { useConfigDiffExecution } from "./useConfigDiffExecution";

interface ConfigDiffPanelProps {
  id: string | null;
  isOpen: boolean;
  onClose: () => void;
  style?: React.CSSProperties;
  isResizing?: boolean;
}

export const ConfigDiffPanel: React.FC<ConfigDiffPanelProps> = React.memo(({ id, isOpen, onClose, style, isResizing }) => {
  const { state: uiState } = useUIContext();
  const proposedDiffData = uiState.configDiffData;

  const { phase, statusMessage, commitLogs, verifiedDiffData, activeTab, setActiveTab, forceCommitReq, operationPlan, logsEndRef, steps, collapsedSteps, currentTime, toggleStepCollapse, handleCommit, handleForceCommitChoice } = useConfigDiffExecution({ id, isOpen, proposedDiffData });
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
        return <span className="diff-phase-badge info"><RefreshIcon size={14} className="spinning" /> 現状Config取得中...</span>;
      case "dry_running":
        return <span className="diff-phase-badge warning"><FlaskIcon size={14} /> 自動Dry-run検証中...</span>;
      case "deploying":
        return <span className="diff-phase-badge warning"><RocketIcon size={14} /> Netmiko投入中...</span>;
      case "verifying":
        return <span className="diff-phase-badge info"><SearchIcon size={14} /> Diff検証中...</span>;
      case "success":
        return <span className="diff-phase-badge success"><CheckIcon size={14} /> 投入&Diff検証完了</span>;
      case "failed":
        return <span className="diff-phase-badge error"><AlertCircleIcon size={14} /> エラー発生</span>;
      default:
        return null;
    }
  };

  return (
    <div
      className={`config-diff-panel ${isOpen ? "open" : "collapsed"} ${isResizing ? "resizing" : ""}`}
      style={isOpen && style?.width !== undefined ? style : undefined}
    >
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

        {operationPlan && (
          <div style={{ padding: "8px 16px", fontSize: "0.8rem", color: "var(--text-secondary)" }}>
            承認済み変更計画: {operationPlan.id}（{operationPlan.approvalStatus}）
          </div>
        )}

        {forceCommitReq && (
          <div
            style={{
              margin: "12px 16px",
              padding: "12px",
              backgroundColor: "rgba(239, 68, 68, 0.15)",
              border: "1px solid #ef4444",
              borderRadius: "6px",
              color: "#f87171",
            }}
          >
            <div style={{ fontWeight: "bold", marginBottom: "6px" }}>
              ⚠️ Dry-run検証でエラーが検出されました
            </div>
            <div style={{ fontSize: "0.85rem", marginBottom: "8px" }}>
              {forceCommitReq.message}
            </div>
            <ul style={{ fontSize: "0.8rem", paddingLeft: "20px", margin: "0 0 10px 0" }}>
              {forceCommitReq.errors.map((err, idx) => (
                <li key={idx}>
                  <strong>{err.line}</strong>: {err.error}
                </li>
              ))}
            </ul>
            <div style={{ display: "flex", gap: "10px", justifyContent: "flex-end" }}>
              <button
                onClick={() => handleForceCommitChoice("cancel")}
                style={{
                  padding: "6px 12px",
                  fontSize: "0.85rem",
                  borderRadius: "4px",
                  border: "1px solid #6b7280",
                  backgroundColor: "transparent",
                  color: "#d1d5db",
                  cursor: "pointer",
                }}
              >
                キャンセル (中断)
              </button>
              <button
                onClick={() => handleForceCommitChoice("commit_force")}
                style={{
                  padding: "6px 12px",
                  fontSize: "0.85rem",
                  borderRadius: "4px",
                  border: "none",
                  backgroundColor: "#ef4444",
                  color: "#ffffff",
                  fontWeight: "bold",
                  cursor: "pointer",
                }}
              >
                エラーを無視して強制投入
              </button>
            </div>
          </div>
        )}

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
              {steps.length === 0 && commitLogs.length === 0 ? (
                <div style={{ color: "#6b7280", fontStyle: "italic" }}>投入ログがここにリアルタイム表示されます...</div>
              ) : steps.length > 0 ? (
                steps.map((step) => {
                  const isCollapsed = collapsedSteps[step.key] ?? false;
                  const elapsedSeconds = step.startTime
                    ? Math.floor(((step.endTime || currentTime) - step.startTime) / 1000)
                    : 0;

                  return (
                    <div key={step.key} className={`log-step-group ${step.status}`}>
                      <div
                        className="log-step-header"
                        onClick={() => toggleStepCollapse(step.key)}
                      >
                        <ChevronIcon
                          size={14}
                          direction={isCollapsed ? "right" : "down"}
                          className="log-step-chevron"
                        />
                        <div className="log-step-icon">
                          {step.status === "running" && (
                            <div className="log-step-spinner" />
                          )}
                          {step.status === "success" && (
                            <CheckIcon size={14} className="log-step-success-icon" />
                          )}
                          {step.status === "failed" && (
                            <AlertCircleIcon size={14} className="log-step-failed-icon" />
                          )}
                          {step.status === "pending" && (
                            <div className="log-step-pending-dot" />
                          )}
                        </div>
                        <span className="log-step-title">{step.title}</span>
                        <span className="log-step-duration">{elapsedSeconds}s</span>
                      </div>

                      {!isCollapsed && (
                        <div className="log-step-body">
                          {step.logs.map((log, index) => {
                            let className = "diff-log-line";
                            if (log.startsWith("[STATUS]")) className = "diff-log-line diff-log-status";
                            else if (log.startsWith("[SYSTEM]")) className = "diff-log-line diff-log-system";
                            else if (log.includes("Error") || log.includes("FAILED") || log.startsWith("[DRY-RUN ERROR]")) className = "diff-log-line diff-log-error";
                            else if (log.startsWith("[DRY-RUN OK]")) className = "diff-log-line diff-log-success";

                            return (
                              <div key={index} className={className}>
                                {log}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  );
                })
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
                <span>承認して実行</span>
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
});

ConfigDiffPanel.displayName = "ConfigDiffPanel";
