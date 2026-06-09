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
  const isNwDb = msg.tool_id === 'query_nw_db' || msg.tool_id === 'network_query_nw_db';
  const defaultExpanded = msg.event_type === "ToolExecution" && !isNwDb;
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [copied, setCopied] = useState(false);

  const handleCopy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy: ', err);
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
                {msg.status === "Success" && <span className="icon-success">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
                </span>}
                {msg.status === "Failed" && <span className="icon-failed">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                </span>}
              </div>
              <div className="timeline-summary-text">
                <div className="timeline-type-icon">
                  {msg.tool_id === 'query_nw_db' || msg.tool_id === 'network_query_nw_db' ? (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1-2.5-2.5Z"></path><path d="M8 2v20"></path></svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>
                  )}
                </div>
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
                  <button 
                    className={`raw-data-copy-button ${copied ? 'copied' : ''}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCopy(msg.raw_data || '');
                    }}
                    title="クリップボードにコピー"
                  >
                    {copied ? (
                      <>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                        <span>コピー済み</span>
                      </>
                    ) : (
                      <>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                        <span>コピー</span>
                      </>
                    )}
                  </button>
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
              const content = part.replace(/```(\w+)?\n?/, "").replace(/```$/, "");
              return <Terminal key={i} content={content} />;
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
