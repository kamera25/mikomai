import React, { useState } from "react";
import "./ApprovalModal.css";
import { WarningIcon } from "./Icons";

interface ApprovalModalProps {
  isOpen: boolean;
  onClose: () => void;
  onApprove: () => void;
  commands: string[];
  rationale: string;
  diffText?: string;
}

export const ApprovalModal: React.FC<ApprovalModalProps> = ({
  isOpen,
  onClose,
  onApprove,
  commands,
  rationale,
  diffText,
}) => {
  const [isArmed, setIsArmed] = useState(false);

  if (!isOpen) return null;

  const handleApprove = () => {
    if (!isArmed) {
      setIsArmed(true);
      return;
    }
    onApprove();
    setIsArmed(false);
  };

  const handleCancel = () => {
    setIsArmed(false);
    onClose();
  };

  return (
    <div className="modal-overlay">
      <div className="modal-content">
        <div className="modal-header">
          <h2 className="modal-title">
            <WarningIcon
              size={24}
              style={{ color: "var(--warning)", marginRight: "10px" }}
            />
            Pending Network Modification
          </h2>
          <button className="close-button" onClick={handleCancel}>
            &times;
          </button>
        </div>

        <div className="modal-body">
          <div className="section rationale-section">
            <h3>AI Rationale</h3>
            <p>{rationale}</p>
          </div>

          <div className="section commands-section">
            <h3>Commands to Execute</h3>
            <pre className="code-block">
              {commands.map((cmd, idx) => (
                <div key={idx} className="command-line">
                  <span className="prompt">&gt;</span> {cmd}
                </div>
              ))}
            </pre>
          </div>

          {diffText && (
            <div className="section diff-section">
              <h3>Configuration Diff</h3>
              <pre className="diff-block">
                {diffText.split("\n").map((line, idx) => {
                  let className = "diff-line";
                  if (line.startsWith("+")) className += " diff-add";
                  else if (line.startsWith("-")) className += " diff-remove";
                  return (
                    <div key={idx} className={className}>
                      {line}
                    </div>
                  );
                })}
              </pre>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={handleCancel}>
            Cancel
          </button>
          <button className={`btn btn-primary ${isArmed ? "armed" : ""}`} onClick={handleApprove}>
            {isArmed ? "Confirm Execution" : "Approve Changes"}
          </button>
        </div>
      </div>
    </div>
  );
};
