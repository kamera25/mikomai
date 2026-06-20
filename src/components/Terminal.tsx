import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { CopyIcon, CheckIcon } from "./Icons";
import "./Terminal.css";

interface TerminalProps {
  content: string;
}

function highlightConfigLine(line: string): React.ReactNode[] {
  // 1. Check if the line is a comment (ensure it's not a CLI command prompt starting with '#')
  const isComment = /^\s*[!]/.test(line) || 
    (/^\s*[#]/.test(line) && 
     !/^\s*#\s*$/.test(line) && 
     !/^\s*#\s*(?:interface|ip|no|shutdown|router|vlan|switchport|description|set|delete|commit|rollback|configure|exit|end|write|system|protocols|routing-options|policy-options|security|firewall|show)\b/i.test(line));

  if (isComment) {
    return [<span key="comment" className="terminal-comment">{line}</span>];
  }

  // 2. Check if the line is a diff line
  let isDiffAdd = false;
  let isDiffRemove = false;
  let cleanLine = line;

  if (line.startsWith("+ ")) {
    isDiffAdd = true;
    cleanLine = line.substring(2);
  } else if (line.startsWith("- ")) {
    isDiffRemove = true;
    cleanLine = line.substring(2);
  } else if (line.startsWith("+") && line.length > 1 && !line.substring(1).trim().startsWith("+")) {
    isDiffAdd = true;
    cleanLine = line.substring(1);
  } else if (line.startsWith("-") && line.length > 1 && !line.substring(1).trim().startsWith("-")) {
    isDiffRemove = true;
    cleanLine = line.substring(1);
  }

  // Define regex for highlighting: double-quoted strings, MAC addresses, IPv6, IPv4, interfaces, slash-numbers, FQDNs, protocols/ports, keywords, statuses, numbers
  const tokenRegex = /("[^"]*"|\b(?:(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}|[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4})\b|\b(?:[0-9a-fA-F]{1,4}::?){1,7}[0-9a-fA-F]{1,4}\b|\b::1\b|\b::\b|\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b|\b(?:GigabitEthernet|FastEthernet|Ethernet|Vlan|Loopback|Tunnel|Port-channel|ge-|xe-|et-|lan|wan)(?:\d+(?:\/\d+)*(?:\.\d+)*)?\b|\/\d+|\b[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+\b|\b(?:udp|tcp|smtp|www|http|https|ssh|dns|ftp|telnet|dhcp|tftp|ntp|snmp|domain|pop3)\b|\b(?:interface|ip address|no|shutdown|router|vlan|ip route|switchport|description|set|delete|commit|rollback|configure|exit|end|write|system|protocols|routing-options|policy-options|security|firewall|show)\b|\b(?:up|down)\b|\b\d+\b)/gi;

  const parts = cleanLine.split(tokenRegex);
  const nodes: React.ReactNode[] = [];

  for (let i = 0; i < parts.length; i++) {
    const val = parts[i];
    if (!val) continue;

    if (i % 2 === 1) {
      // Matched token
      if (val.startsWith('"') && val.endsWith('"')) {
        nodes.push(<span key={`str-${i}`} className="terminal-string">{val}</span>);
      } else if (/^(?:(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}|[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4}\.[0-9A-Fa-f]{4})$/.test(val)) {
        nodes.push(<span key={`mac-${i}`} className="terminal-mac">{val}</span>);
      } else if (/^(?:(?:[0-9a-fA-F]{1,4}::?){1,7}[0-9a-fA-F]{1,4}|::1|::)$/i.test(val)) {
        nodes.push(<span key={`ip6-${i}`} className="terminal-ip">{val}</span>);
      } else if (/^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(val)) {
        nodes.push(<span key={`ip-${i}`} className="terminal-ip">{val}</span>);
      } else if (/^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+$/.test(val)) {
        nodes.push(<span key={`fqdn-${i}`} className="terminal-fqdn">{val}</span>);
      } else if (/^(?:GigabitEthernet|FastEthernet|Ethernet|Vlan|Loopback|Tunnel|Port-channel|ge-|xe-|et-|lan|wan)/i.test(val)) {
        nodes.push(<span key={`if-${i}`} className="terminal-interface">{val}</span>);
      } else if (/^\/\d+$/.test(val)) {
        nodes.push(<span key={`slashnum-${i}`} className="terminal-slash-number">{val}</span>);
      } else if (/^(?:udp|tcp|smtp|www|http|https|ssh|dns|ftp|telnet|dhcp|tftp|ntp|snmp|domain|pop3)$/i.test(val)) {
        nodes.push(<span key={`proto-${i}`} className="terminal-protocol">{val}</span>);
      } else if (/^(?:interface|ip address|no|shutdown|router|vlan|ip route|switchport|description|set|delete|commit|rollback|configure|exit|end|write|system|protocols|routing-options|policy-options|security|firewall|show)$/i.test(val)) {
        nodes.push(<span key={`kw-${i}`} className="terminal-keyword">{val}</span>);
      } else if (/^up$/i.test(val)) {
        nodes.push(<span key={`up-${i}`} className="terminal-status-up">{val}</span>);
      } else if (/^down$/i.test(val)) {
        nodes.push(<span key={`down-${i}`} className="terminal-status-down">{val}</span>);
      } else if (/^\d+$/.test(val)) {
        nodes.push(<span key={`num-${i}`} className="terminal-number">{val}</span>);
      } else {
        nodes.push(<span key={`tok-${i}`}>{val}</span>);
      }
    } else {
      // Plain text
      nodes.push(<span key={`text-${i}`}>{val}</span>);
    }
  }

  // Wrap in diff span if applicable
  if (isDiffAdd) {
    return [
      <span key="diff-prefix" className="terminal-diff-prefix add">+ </span>,
      <span key="diff-content" className="terminal-diff-add">{nodes}</span>
    ];
  } else if (isDiffRemove) {
    return [
      <span key="diff-prefix" className="terminal-diff-prefix remove">- </span>,
      <span key="diff-content" className="terminal-diff-remove">{nodes}</span>
    ];
  }

  return nodes;
}

function parseAnsi(text: string): React.ReactNode[] {
  // Matches ESC [ <codes> m
  const ansiRegex = /\x1b\[([0-9;]*)m/g;
  const parts = text.split(ansiRegex);

  if (parts.length === 1) {
    return highlightConfigLine(text);
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
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy: ", err);
    }
  };

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
        <span className="terminal-title">Raw Output</span>
        <button
          className={`terminal-copy-button ${copied ? "copied" : ""}`}
          onClick={handleCopy}
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
