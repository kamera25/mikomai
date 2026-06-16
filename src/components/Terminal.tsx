import React from "react";
import "./Terminal.css";

interface TerminalProps {
  content: string;
}

function parseAnsi(text: string): React.ReactNode[] {
  // Matches ESC [ <codes> m
  const ansiRegex = /\x1b\[([0-9;]*)m/g;
  const parts = text.split(ansiRegex);

  if (parts.length === 1) {
    return [text];
  }

  const nodes: React.ReactNode[] = [];
  let currentStyle: React.CSSProperties = {};

  for (let i = 0; i < parts.length; i++) {
    if (i % 2 === 1) {
      // It's an ANSI code like "1;31"
      const codes = parts[i].split(";").map(Number);
      if (codes.length === 0 || codes[0] === 0) {
        currentStyle = {};
      } else {
        codes.forEach((code) => {
          if (code === 1) {
            currentStyle.fontWeight = "bold";
          } else if (code === 4) {
            currentStyle.textDecoration = "underline";
          } else if (code >= 30 && code <= 37) {
            const colors = [
              "black",
              "red",
              "green",
              "yellow",
              "blue",
              "magenta",
              "cyan",
              "white",
            ];
            currentStyle.color = `var(--terminal-${colors[code - 30]})`;
          } else if (code >= 90 && code <= 97) {
            const colors = [
              "black",
              "red",
              "green",
              "yellow",
              "blue",
              "magenta",
              "cyan",
              "white",
            ];
            currentStyle.color = `var(--terminal-bright-${colors[code - 90]})`;
          } else if (code >= 40 && code <= 47) {
            const colors = [
              "black",
              "red",
              "green",
              "yellow",
              "blue",
              "magenta",
              "cyan",
              "white",
            ];
            currentStyle.backgroundColor = `var(--terminal-${colors[code - 40]})`;
          }
        });
      }
    } else {
      // It's text
      const val = parts[i];
      if (val) {
        nodes.push(
          <span key={i} style={{ ...currentStyle }}>
            {val}
          </span>
        );
      }
    }
  }

  return nodes;
}

export const Terminal: React.FC<TerminalProps> = ({ content }) => {
  // Split content into lines, filter out empty trailing lines
  const rawLines = content.split("\n");
  const lines = rawLines.length > 1 && rawLines[rawLines.length - 1] === "" 
    ? rawLines.slice(0, -1) 
    : rawLines;

  return (
    <div className="terminal-container">
      <div className="terminal-header">
        <div className="terminal-dots">
          <span className="terminal-dot red"></span>
          <span className="terminal-dot yellow"></span>
          <span className="terminal-dot green"></span>
        </div>
        <span className="terminal-title">Terminal Output</span>
      </div>
      <pre className="terminal-content">
        <code>
          {lines.map((line, idx) => (
            <div key={idx} className="terminal-line">
              <span className="terminal-line-number">{idx + 1}</span>
              <span className="terminal-line-text">{parseAnsi(line)}</span>
            </div>
          ))}
        </code>
      </pre>
    </div>
  );
};
