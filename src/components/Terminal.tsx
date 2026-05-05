import React from 'react';
import './Terminal.css';

interface TerminalProps {
  content: string;
}

export const Terminal: React.FC<TerminalProps> = ({ content }) => {
  return (
    <div className="terminal-container">
      <pre className="terminal-content">
        <code>{content}</code>
      </pre>
    </div>
  );
};
