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

  if (msg.event_type === "ToolExecution") {
    return (
      <div className={`timeline-block tool-execution ${msg.status?.toLowerCase()}`}>
        <div
          className="timeline-summary"
          onClick={() => msg.status !== "Running" && setIsExpanded(!isExpanded)}
          style={{ cursor: msg.status === "Running" ? "default" : "pointer" }}
        >
          <div className="timeline-status-icon">
            {msg.status === "Running" && <div className="status-spinner-small"></div>}
            {msg.status === "Success" && <span className="icon-success">✅</span>}
            {msg.status === "Failed" && <span className="icon-failed">❌</span>}
          </div>
          <div className="timeline-summary-text">
            <strong>{msg.action_name}</strong>: {msg.summary_text}
          </div>
          {msg.status !== "Running" && (
            <div className={`timeline-chevron ${isExpanded ? "open" : ""}`}>
              ▼
            </div>
          )}
        </div>

        {isExpanded && msg.raw_data && (
          <div className="timeline-raw-data">
            <Terminal content={msg.raw_data} />
          </div>
        )}
      </div>
    );
  }

  // Handle standard User/AI messages or fallback
  return (
    <div className={`message-container ${msg.role}`}>
      {msg.role === 'user' && (
        <div className="message-header">
          <div className="header-line"></div>
          <span className="message-time">{formatMessageTime(msg.timestamp)}</span>
        </div>
      )}
      <div className={`message ${msg.role}`}>
        <div className="message-bubble markdown-body">
          {msg.content.split(/(```[\s\S]*?```)/).map((part, i) => {
            if (part.startsWith("```")) {
              const isTerminal = part.startsWith("```terminal");
              const content = part.replace(/```(\w+)?\n?/, "").replace(/```$/, "");

              if (isTerminal) {
                return <Terminal key={i} content={content} />;
              }

              return <pre key={i} className="code-block"><code>{content}</code></pre>;
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
