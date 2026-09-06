import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { invoke } from "@tauri-apps/api/core";
import { Terminal } from "../Terminal";
import { CheckIcon, CrossIcon } from "../Icons";
import { Message } from "../../types";
import { messageContainerClass } from "./timelineModel";

export function CiscoValidationEvent({ msg }: { msg: Message }) {
if (msg.event_type === "ToolExecution" && msg.tool_id === "validate_cisco_config") {
  const handleCommitChoice = async (choice: "commit" | "cancelled") => {
    try {
      await invoke("submit_user_choice", { id: msg.task_id, choice });
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
    <div className={messageContainerClass(msg)} id={msg.task_id}>
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


  return null;
}
