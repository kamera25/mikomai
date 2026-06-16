import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Terminal } from "../Terminal";
import { CheckIcon, CopyIcon, BoxIcon, ChevronIcon, BookIcon, TerminalIcon } from "../Icons";
import { Message } from "../../types";

interface TimelineEventProps {
  msg: Message;
  formatMessageTime: (isoString?: string) => string;
}

export const TimelineEvent = ({ msg, formatMessageTime }: TimelineEventProps) => {
  const { t } = useTranslation();
  const isNwDb = msg.tool_id === "query_nw_db" || msg.tool_id === "network_query_nw_db";
  const defaultExpanded = msg.event_type === "ToolExecution" && !isNwDb;
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
            >
              <div className="timeline-status-icon">
                {msg.status === "Running" && <div className="status-spinner-small"></div>}
                {msg.status === "Success" && (
                  <span className="icon-success">
                    <svg
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="3"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                  </span>
                )}
                {msg.status === "Failed" && (
                  <span className="icon-failed">
                    <svg
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="3"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </span>
                )}
              </div>
              <div className="timeline-summary-text">
                <div className="timeline-type-icon">
                  {isNwDb ? (
                    <BookIcon size={14} />
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
                <div className="raw-data-header">
                  <span>RAW OUTPUT</span>
                  <button
                    className={`raw-data-copy-button ${copied ? "copied" : ""}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCopy(msg.raw_data || "");
                    }}
                    title={t("common.copy")}
                  >
                    {copied ? (
                      <>
                        <CheckIcon size={12} strokeWidth={3} />
                        <span>{t("common.copied")}</span>
                      </>
                    ) : (
                      <>
                        <CopyIcon size={12} strokeWidth={2.5} />
                        <span>{t("common.copy")}</span>
                      </>
                    )}
                  </button>
                </div>
                <div className="timeline-raw-data">
                  <Terminal content={msg.raw_data} />
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
          <div className="message-bubble markdown-body">
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
              {msg.content}
            </ReactMarkdown>
          </div>
          {msg.role === "ai" && (
            <div className="message-actions">
              <button
                className="message-action-btn"
                title={t("common.copy")}
                onClick={() => handleCopy(msg.content)}
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
