import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Terminal } from "../Terminal";
import { CheckIcon, CopyIcon, BoxIcon, ChevronIcon, BookIcon, TerminalIcon, CrossIcon, SpeechIcon, RobotIcon } from "../Icons";
import { Message } from "../../types";
import { invoke } from "@tauri-apps/api/core";

interface TimelineEventProps {
  msg: Message;
  formatMessageTime: (isoString?: string) => string;
}

export const TimelineEvent = ({ msg, formatMessageTime }: TimelineEventProps) => {
  const { t } = useTranslation();
  const isNwDb = msg.tool_id === "query_nw_db" || msg.tool_id === "network_query_nw_db";
  const isChoice = msg.tool_id === "ask_user_choice" || msg.tool_id === "ask_interface_choice";
  const defaultExpanded = msg.event_type === "ToolExecution" && !isNwDb && !isChoice;
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [copied, setCopied] = useState(false);
  const [pathCopied, setPathCopied] = useState(false);

  const handleCopy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy: ", err);
    }
  };

  const handleCopyPath = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setPathCopied(true);
      setTimeout(() => setPathCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy: ", err);
    }
  };

  if (msg.isHidden) return null;

  const getContainerClass = () => {
    let classes = `message-container ${msg.role}`;
    if (msg.event_type) classes += ` ${msg.event_type.toLowerCase()}`;
    if (msg.status) classes += ` ${msg.status.toLowerCase()}`;
    return classes;
  };

  if (msg.event_type === "ToolExecution" && msg.tool_id === "validate_cisco_config") {
    const handleCommitChoice = async (choice: "commit" | "cancelled") => {
      try {
        await invoke("submit_user_choice", { choice });
      } catch (err) {
        console.error(`Failed to submit choice ${choice}:`, err);
      }
    };

    let cardClass = "cisco-validation-card";
    if (msg.isToolLoading) {
      if (msg.waitingForApproval) {
        cardClass += " waiting-approval";
      } else {
        cardClass += " running";
      }
    } else {
      cardClass += msg.status === "Success" ? " success" : " failed";
    }

    return (
      <div className={getContainerClass()} id={msg.task_id}>
        <div className="timeline-node"></div>
        <div className="message ai" style={{ width: "100%" }}>
          <div className={cardClass}>
            <div className="cisco-validation-header">
              {msg.isToolLoading ? (
                msg.waitingForApproval ? (
                  <div className="pulsing-dot green"></div>
                ) : (
                  <div className="status-spinner-small"></div>
                )
              ) : msg.status === "Success" ? (
                <span className="icon-success"><CheckIcon size={18} strokeWidth={3} /></span>
              ) : (
                <span className="icon-failed"><CrossIcon size={18} strokeWidth={3} /></span>
              )}
              <span className="cisco-validation-title">
                {msg.isToolLoading
                  ? msg.waitingForApproval
                    ? "承認待ち"
                    : "Configのチェック中"
                  : msg.status === "Success"
                  ? "Cisco Config 検証成功"
                  : "Cisco Config 検証失敗 / キャンセル"}
              </span>
            </div>

            <div className="cisco-validation-desc">
              {msg.isToolLoading ? (
                msg.waitingForApproval ? (
                  "コミットを承認しますか？"
                ) : (
                  "Ciscoの構成ファイルを検証しています。しばらくお待ちください..."
                )
              ) : msg.status === "Success" ? (
                "Cisco Config の検証およびコミット承認が完了しました。"
              ) : (
                "検証でエラーが検出されたか、ユーザーによってキャンセルされました。"
              )}
            </div>

            {msg.isToolLoading && msg.waitingForApproval && (
              <div className="cisco-validation-actions">
                <button
                  className="cisco-validation-btn-commit"
                  onClick={() => handleCommitChoice("commit")}
                >
                  コミット
                </button>
                <button
                  className="cisco-validation-btn-cancel"
                  onClick={() => handleCommitChoice("cancelled")}
                >
                  中止
                </button>
              </div>
            )}

            {!msg.isToolLoading && msg.raw_data && (
              <div style={{ marginTop: "12px", borderTop: "1px solid rgba(255, 255, 255, 0.1)", paddingTop: "12px" }}>
                <ReactMarkdown
                  remarkPlugins={[remarkGfm, remarkMath]}
                  rehypePlugins={[rehypeKatex]}
                  components={{
                    pre({ children }) {
                      const codeElement = React.Children.toArray(children)[0];
                      if (React.isValidElement(codeElement) && codeElement.props) {
                        const codeText = String((codeElement.props as any).children || "").replace(/\n$/, "");
                        return <Terminal content={codeText} />;
                      }
                      return <pre>{children}</pre>;
                    }
                  }}
                >
                  {msg.raw_data}
                </ReactMarkdown>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (msg.event_type === "ToolExecution") {
    return (
      <div className={getContainerClass()} id={msg.task_id}>
        <div className="timeline-node"></div>
        <div className="message ai">
          <div className={`timeline-block tool-execution ${msg.status?.toLowerCase()}`}>
            <div
              className="timeline-summary"
              onClick={() => msg.status !== "Running" && setIsExpanded(!isExpanded)}
              style={{ cursor: msg.status === "Running" ? "default" : "pointer" }}
              role="button"
              tabIndex={msg.status === "Running" ? -1 : 0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  if (msg.status !== "Running") {
                    e.preventDefault();
                    setIsExpanded(!isExpanded);
                  }
                }
              }}
            >
              <div className="timeline-status-icon">
                {msg.status === "Running" && <div className="status-spinner-small"></div>}
                {msg.status === "Success" && (
                  <span className="icon-success">
                    <CheckIcon size={14} strokeWidth={3} />
                  </span>
                )}
                {msg.status === "Failed" && (
                  <span className="icon-failed">
                    <CrossIcon size={14} strokeWidth={3} />
                  </span>
                )}
              </div>
              <div className="timeline-summary-text">
                <div className="timeline-type-icon">
                  {isNwDb ? (
                    <BookIcon size={14} />
                  ) : isChoice ? (
                    <SpeechIcon size={14} />
                  ) : (
                    <TerminalIcon size={14} />
                  )}
                </div>
                <span className="action-label">{msg.action_name}</span>
                <span className="summary-content">{msg.summary_text}</span>
              </div>
              {msg.status !== "Running" && (
                <div className="timeline-expand-icon">
                  <ChevronIcon direction={isExpanded ? "up" : "down"} size={16} />
                </div>
              )}
            </div>

            {isExpanded && msg.raw_data && (
              <div className="timeline-raw-data-wrapper">
                <div className="timeline-raw-data">
                  {msg.tool_id === "self_network_nwdiag" ? (
                    (() => {
                      const match = msg.raw_data.match(/!\[.*?\]\((.*?)\)/);
                      const src = match ? match[1] : msg.raw_data;
                      return (
                        <div
                          className="nwdiag-preview-container"
                          style={{
                            padding: "16px",
                            background: "#ffffff",
                            borderRadius: "6px",
                            display: "flex",
                            justifyContent: "center",
                            alignItems: "center",
                            border: "1px solid #2d2d2d",
                            marginTop: "8px",
                          }}
                        >
                          <img
                            src={src}
                            alt="Network Diagram"
                            style={{
                              maxWidth: "100%",
                              maxHeight: "500px",
                              height: "auto",
                              borderRadius: "4px",
                            }}
                          />
                        </div>
                      );
                    })()
                  ) : (
                    <Terminal content={msg.raw_data} />
                  )}
                </div>
              </div>
            )}

            {isExpanded && msg.saved_path && (
              <div className="timeline-saved-path-wrapper">
                <div className="timeline-saved-path-inner">
                  <div className="timeline-saved-path-header">
                    <BoxIcon size={14} className="box-icon" />
                    <span>
                      {msg.is_cached
                        ? t("common.updated_at_cached", { time: msg.cache_time || "" })
                        : t("common.log_saved")}
                    </span>
                    <button
                      className={`copy-path-btn ${pathCopied ? "copied" : ""}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        handleCopyPath(msg.saved_path || "");
                      }}
                      title={t("common.path_copied")}
                    >
                    {pathCopied ? (
                      <>
                        <CheckIcon size={12} strokeWidth={3} />
                        <span>{t("common.copied")}</span>
                      </>
                    ) : (
                      <>
                        <CopyIcon size={12} />
                        <span>{t("common.save_path_copied")}</span>
                      </>
                    )}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  }

  // Handle standard User/AI messages as timeline events
  let thoughtContent = "";
  let remainingContent = msg.content;
  let hasThought = false;

  if (msg.role === "ai" && msg.content.startsWith("<thought>")) {
    const thoughtIndex = msg.content.indexOf("</thought>");
    if (thoughtIndex !== -1) {
      thoughtContent = msg.content.substring(9, thoughtIndex);
      remainingContent = msg.content.substring(thoughtIndex + 10).trim();
      hasThought = true;
    } else {
      thoughtContent = msg.content.substring(9);
      remainingContent = "";
      hasThought = true;
    }
  }

  return (
    <div className={getContainerClass()} id={msg.task_id}>
      <div className="timeline-node"></div>
      <div className="message-header">
        <span className="message-time">{formatMessageTime(msg.timestamp)}</span>
      </div>
      <div className={`message ${msg.role}`}>
        <div
          className="message-content-wrapper"
          style={{
            display: "flex",
            flexDirection: "column",
            width: "100%",
            alignItems: msg.role === "user" ? "flex-end" : "flex-start",
          }}
        >
          {hasThought && (
            <div className="thought-container">
              <div className="thought-icon" title="Thinking process">
                <RobotIcon size={16} />
              </div>
              <div className="thought-bubble">
                <ReactMarkdown
                  remarkPlugins={[remarkGfm, remarkMath]}
                  rehypePlugins={[rehypeKatex]}
                >
                  {thoughtContent}
                </ReactMarkdown>
              </div>
            </div>
          )}

          {(!hasThought || remainingContent !== "") && (
            <div className={`message-bubble markdown-body ${msg.status === "Pending" ? "pending" : ""}`}>
              <ReactMarkdown
                remarkPlugins={[remarkGfm, remarkMath]}
                rehypePlugins={[rehypeKatex]}
                components={{
                  pre({ children }) {
                    const codeElement = React.Children.toArray(children)[0];
                    if (React.isValidElement(codeElement) && codeElement.props) {
                      const codeText = String((codeElement.props as any).children || "").replace(/\n$/, "");
                      return <Terminal content={codeText} />;
                    }
                    return <pre>{children}</pre>;
                  }
                }}
              >
                {remainingContent}
              </ReactMarkdown>
            </div>
          )}
          {msg.role === "user" && msg.status === "Pending" && (
            <div className="message-pending-indicator">
              <span className="status-spinner-small"></span>
              <span>{t("chat.pending")}</span>
            </div>
          )}
          {msg.role === "ai" && (
            <div className="message-actions">
              <button
                className="message-action-btn"
                title={t("common.copy")}
                onClick={() => handleCopy(remainingContent || msg.content)}
              >
                {copied ? (
                  <CheckIcon size={16} style={{ color: "var(--success)" }} />
                ) : (
                  <CopyIcon size={16} />
                )}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
