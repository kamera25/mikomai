import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Terminal } from "../Terminal";
import { CheckIcon, CopyIcon, BoxIcon, ChevronIcon, BookIcon, TerminalIcon, CrossIcon, SpeechIcon, RobotIcon, FileTextIcon, FolderIcon, DownloadIcon } from "../Icons";
import { Message } from "../../types";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { ImageModal } from "../ImageModal/ImageModal";
import { defaultFilename, isChoiceTool, isNetworkDatabaseTool, messageContainerClass } from "./timelineModel";
import { CiscoValidationEvent } from "./CiscoValidationEvent";


interface TimelineEventProps {
  msg: Message;
  formatMessageTime: (isoString?: string) => string;
  sendMessage?: (text?: string) => Promise<void>;
}

export const TimelineEvent = React.memo(({ msg, formatMessageTime, sendMessage }: TimelineEventProps) => {
  const { t } = useTranslation();
  const isNwDb = isNetworkDatabaseTool(msg.tool_id);
  const isChoice = isChoiceTool(msg.tool_id);
  const defaultExpanded = false;
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [copied, setCopied] = useState(false);
  const [pathCopied, setPathCopied] = useState(false);
  const [selectedImage, setSelectedImage] = useState<{ src: string; alt?: string } | null>(null);

  const [fileFetched, setFileFetched] = useState(false);
  const isMac = typeof navigator !== "undefined" && /Macintosh|Mac OS X/i.test(navigator.userAgent);
  const openFileManagerLabel = isMac ? t("common.open_in_finder") : t("common.open_in_explorer");

  const handleDeviceRetrievalClick = () => {
    if (sendMessage) {
      sendMessage("実機から情報を取得してください");
    }
  };


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

  const handleOpenPathInFileManager = async (path: string) => {
    try {
      await invoke("open_path_in_file_manager", { path });
    } catch (err) {
      console.error("Failed to open path in file manager: ", err);
    }
  };

  const handleFetchFileClick = async (savedPath: string) => {
    try {
      const filename = defaultFilename(savedPath);

      const selectedPath = await save({
        defaultPath: filename,
        title: t("common.fetch_file"),
      });

      if (selectedPath) {
        await invoke("copy_file_to_destination", {
          srcPath: savedPath,
          destPath: selectedPath,
        });
        setFileFetched(true);
        setTimeout(() => setFileFetched(false), 2000);
      }
    } catch (err) {
      console.error("Failed to fetch file: ", err);
    }
  };


  if (msg.isHidden) return null;

  const getContainerClass = () => messageContainerClass(msg);

  if (msg.event_type === "ToolExecution" && msg.tool_id === "validate_cisco_config") {
    return <CiscoValidationEvent msg={msg} />;
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
                {msg.status === "Running" && <div className="codex-pulse-indicator status-spinner-small"></div>}
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
                <span className={`summary-content ${msg.status === "Running" ? "codex-wave-text" : ""}`}>
                  {msg.summary_text}
                </span>
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

            {msg.saved_path && (
              <div className="timeline-saved-path-wrapper">
                <div className="timeline-saved-path-info">
                  <BoxIcon size={14} className="box-icon" />
                  <span>
                    {msg.is_cached
                      ? t("common.updated_at_cached", { time: msg.cache_time || "" })
                      : t("common.log_saved")}
                  </span>
                </div>
                <div className="timeline-saved-path-actions">
                  <button
                    className="open-path-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleOpenPathInFileManager(msg.saved_path || "");
                    }}
                    title={openFileManagerLabel}
                  >
                    <FolderIcon size={12} />
                    <span>{openFileManagerLabel}</span>
                  </button>
                  <button
                    className={`fetch-file-btn ${fileFetched ? "copied" : ""}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleFetchFileClick(msg.saved_path || "");
                    }}
                    title={t("common.fetch_file")}
                  >
                    {fileFetched ? (
                      <>
                        <CheckIcon size={12} strokeWidth={3} />
                        <span>{t("common.file_fetched")}</span>
                      </>
                    ) : (
                      <>
                        <DownloadIcon size={12} />
                        <span>{t("common.fetch_file")}</span>
                      </>
                    )}
                  </button>
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

          {(!hasThought || remainingContent !== "") && (() => {
            // Normalize agent-step blocks:
            // Find highest step number and update the first agent-step in-place while removing subsequent ones.
            let processedContent = remainingContent;
            const stepMatches = Array.from(
              remainingContent.matchAll(/```agent-step[\s\S]*?\bstep:\s*(\d+)\b[\s\S]*?```/gi)
            );

            if (stepMatches.length > 0) {
              const highestStep = stepMatches.reduce(
                (max, m) => Math.max(max, parseInt(m[1], 10)),
                1
              );

              let isFirst = true;
              processedContent = remainingContent.replace(
                /```agent-step[\s\S]*?```\n?/gi,
                () => {
                  if (isFirst) {
                    isFirst = false;
                    return `\`\`\`agent-step\nphase: planning\nstep: ${highestStep}\n\`\`\`\n`;
                  }
                  return "";
                }
              );
            }

            return (
              <div className={`message-bubble markdown-body ${msg.status === "Pending" ? "pending" : ""}`}>
                {msg.role === "ai" &&
                (remainingContent.includes("考え中") ||
                  remainingContent.includes("画像の読み取り") ||
                  remainingContent === t("chat.thinking") ||
                  remainingContent === t("chat.analyzing") ||
                  remainingContent === t("chat.reading_image") ||
                  (msg.isToolLoading && (remainingContent === "" || remainingContent === t("chat.thinking") || remainingContent === t("chat.reading_image")))) ? (
                  <div className="thinking-indicator">
                    <div className="codex-pulse-indicator status-spinner-small"></div>
                    <span className="codex-wave-text">{remainingContent || t("chat.thinking")}</span>
                  </div>
                ) : (
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm, remarkMath]}
                    rehypePlugins={[rehypeKatex]}
                    components={{
                      pre({ children }) {
                        const codeElement = React.Children.toArray(children)[0];
                        if (React.isValidElement(codeElement) && codeElement.props) {
                          const className = ((codeElement.props as any).className as string) || "";
                          const codeText = String((codeElement.props as any).children || "").replace(/\n$/, "");

                          if (className.includes("language-agent-step")) {
                            const stepMatch = codeText.match(/step:\s*(\d+)/i);
                            const step = stepMatch ? stepMatch[1] : "1";

                            const hasFinishedDecision = /action:\s*FINISH\b/i.test(remainingContent);
                            const isDone = !msg.isToolLoading || hasFinishedDecision;

                            if (isDone) {
                              return (
                                <div className="codex-agent-step done">
                                  <div className="codex-step-header">
                                    <span className="codex-step-done-icon">
                                      <CheckIcon size={12} strokeWidth={3} />
                                    </span>
                                    <span className="codex-step-badge done">STEP {step}</span>
                                    <span className="codex-step-done-text">完了</span>
                                  </div>
                                </div>
                              );
                            }

                            return (
                              <div className="codex-agent-step active">
                                <div className="codex-step-header">
                                  <span className="codex-pulse-indicator"></span>
                                  <span className="codex-step-badge">STEP {step}</span>
                                  <span className="codex-wave-text">思考・計画中... (Planning)</span>
                                </div>
                              </div>
                            );
                          }

                          if (className.includes("language-agent-decision")) {
                            const stepMatch = codeText.match(/step:\s*([^\n]+)/i);
                            const actionMatch = codeText.match(/action:\s*([^\n]+)/i);
                            const objectiveMatch = codeText.match(/objective:\s*([^\n]+)/i);
                            const reasonMatch = codeText.match(/reason:\s*([^\n]+)/i);

                            const step = stepMatch ? stepMatch[1].trim() : "1";
                            const action = actionMatch ? actionMatch[1].trim() : "Decision";
                            const objective = objectiveMatch ? objectiveMatch[1].trim() : "";
                            const reason = reasonMatch ? reasonMatch[1].trim() : "";

                            return (
                              <div className="codex-agent-decision">
                                <div className="codex-decision-header">
                                  <span className="codex-decision-tag">{action}</span>
                                  <span className="codex-decision-step">Step {step}</span>
                                </div>
                                <div className="codex-decision-body">
                                  {objective && (
                                    <div className="codex-decision-row">
                                      <span className="codex-decision-label">Objective:</span>
                                      <span className="codex-decision-val">{objective}</span>
                                    </div>
                                  )}
                                  {reason && (
                                    <div className="codex-decision-row">
                                      <span className="codex-decision-label">Reason:</span>
                                      <span className="codex-decision-val">{reason}</span>
                                    </div>
                                  )}
                                </div>
                              </div>
                            );
                          }

                          if (className.includes("language-agent-warning")) {
                            const msgMatch = codeText.match(/message:\s*([^\n]+)/i);
                            const warnMsg = msgMatch ? msgMatch[1].trim() : codeText;
                            return (
                              <div className="codex-agent-warning">
                                <span className="codex-warning-icon">⚠️</span>
                                <span className="codex-warning-text">{warnMsg}</span>
                              </div>
                            );
                          }

                          return <Terminal content={codeText} />;
                        }
                        return <pre>{children}</pre>;
                      },
                      img({ src, alt }) {
                        return (
                          <img
                            src={src}
                            alt={alt}
                            style={{ cursor: "pointer", maxWidth: "100%" }}
                            onClick={() => src && setSelectedImage({ src, alt })}
                            title="クリックして拡大"
                          />
                        );
                      }
                    }}
                  >
                    {processedContent}
                  </ReactMarkdown>
                )}
              </div>
            );
          })()}
          {msg.role === "user" && msg.event_type === "UserInput" && msg.attachments && msg.attachments.length > 0 && (
            <div className="message-attachments-container">
              {msg.attachments.map((att, idx) => {
                if (att.type === "image") {
                  return (
                    <div key={idx} className="message-attachment-image-wrapper">
                      <img
                        src={att.content}
                        alt={att.name}
                        className="message-attachment-image"
                        onClick={() => setSelectedImage({ src: att.content, alt: att.name })}
                        title="クリックして拡大"
                        style={{ cursor: "pointer" }}
                      />
                      <span className="message-attachment-name">{att.name}</span>
                    </div>
                  );
                } else {
                  return (
                    <div key={idx} className="message-attachment-text-wrapper">
                      <div className="message-attachment-text-header" title={att.path || att.name}>
                        <FileTextIcon size={14} />
                        <span className="message-attachment-name">{att.name}</span>
                        {att.path && (
                          <span
                            className="message-attachment-path"
                            style={{
                              fontSize: "0.75rem",
                              opacity: 0.65,
                              marginLeft: "6px",
                              maxWidth: "220px",
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                            title={att.path}
                          >
                            ({att.path})
                          </span>
                        )}
                      </div>
                      <div className="message-attachment-text-body">
                        <pre>{att.content}</pre>
                      </div>
                    </div>
                  );
                }
              })}
            </div>
          )}
          {msg.role === "ai" && (remainingContent || msg.content).includes("追加の検索キーワードを指示するか、実機から情報を取得しますか？") && (
            <div className="suggestion-bubble-container">
              <button className="suggestion-bubble-btn" onClick={handleDeviceRetrievalClick}>
                <span>実機から情報を取得する</span>
              </button>
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
      {selectedImage && (
        <ImageModal
          src={selectedImage.src}
          alt={selectedImage.alt}
          onClose={() => setSelectedImage(null)}
        />
      )}
    </div>
  );
});

TimelineEvent.displayName = "TimelineEvent";
