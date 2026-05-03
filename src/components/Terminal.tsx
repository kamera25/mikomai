import React, { useState } from 'react';
import './Terminal.css';

interface TerminalProps {
  content: string;
}

export const Terminal: React.FC<TerminalProps> = ({ content }) => {
  const [copied, setCopied] = useState(false);

  const isMac = navigator.userAgent.toLowerCase().includes('mac');
  const platform = isMac ? 'mac' : 'win'; // Simplified for window button styles

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy: ', err);
    }
  };

  return (
    <div className={`terminal-container platform-${platform}`}>
      <div className="terminal-header">
        {isMac ? (
          <div className="terminal-dots">
            <div className="terminal-dot red"></div>
            <div className="terminal-dot yellow"></div>
            <div className="terminal-dot green"></div>
          </div>
        ) : (
          <div className="terminal-title">Terminal</div>
        )}
        
        <div className="terminal-actions">
          {!isMac && (
            <div className="win-buttons">
              <div className="win-button minimize"></div>
              <div className="win-button maximize"></div>
              <div className="win-button close"></div>
            </div>
          )}
          <button 
            className={`copy-button ${copied ? 'copied' : ''}`} 
            onClick={handleCopy}
            title="クリップボードにコピー"
          >
            {copied ? (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12"></polyline>
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
            )}
          </button>
        </div>
      </div>
      <pre className="terminal-content">
        <code>{content}</code>
      </pre>
    </div>
  );
};
