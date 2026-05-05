import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import { Terminal } from '../Terminal';
import { Message } from '../../types';

interface TimelineEventProps {
  msg: Message;
  formatMessageTime: (isoString?: string) => string;
}

export const TimelineEvent = ({ msg, formatMessageTime }: TimelineEventProps) => {
  const [isExpanded, setIsExpanded] = useState(false);

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
                {msg.status === "Success" && <span className="icon-success">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
                </span>}
                {msg.status === "Failed" && <span className="icon-failed">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                </span>}
              </div>
              <div className="timeline-summary-text">
                <span className="action-label">{msg.action_name}</span>
                <span className="summary-content">{msg.summary_text}</span>
              </div>
              {msg.status !== "Running" && (
                <div className={`timeline-chevron ${isExpanded ? "open" : ""}`}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                </div>
              )}
            </div>

            {isExpanded && msg.raw_data && (
              <div className="timeline-raw-data-wrapper">
                <div className="raw-data-header">
                  <span>RAW OUTPUT</span>
                </div>
                <div className="timeline-raw-data">
                  <Terminal content={msg.raw_data} />
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
        <div className="message-bubble markdown-body">
          {msg.content.split(/(```[\s\S]*?```)/).map((part, i) => {
            if (part.startsWith("```")) {
              const isTerminal = part.startsWith("```terminal");
              const content = part.replace(/```(\w+)?\n?/, "").replace(/```$/, "");

              if (isTerminal) {
                return <Terminal key={i} content={content} />;
              }

              return (
                <div key={i} className="code-block-wrapper">
                  <div className="code-block-header">
                    <span>CODE</span>
                  </div>
                  <pre className="code-block"><code>{content}</code></pre>
                </div>
              );
            }
            return (
              <ReactMarkdown
                key={i}
                remarkPlugins={[remarkGfm, remarkMath]}
                rehypePlugins={[rehypeKatex]}
              >
                {part}
              </ReactMarkdown>
            );
          })}
        </div>
      </div>
    </div>
  );
};
