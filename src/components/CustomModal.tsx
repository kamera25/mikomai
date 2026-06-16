import React, { useState, useEffect, useRef } from "react";
import "./CustomModal.css";

interface CustomModalProps {
  isOpen: boolean;
  type: "confirm" | "prompt";
  title: string;
  message: string;
  placeholder?: string;
  initialValue?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: (value?: string) => void;
  onCancel: () => void;
}

export const CustomModal: React.FC<CustomModalProps> = ({
  isOpen,
  type,
  title,
  message,
  placeholder = "",
  initialValue = "",
  confirmLabel = "確定",
  cancelLabel = "キャンセル",
  onConfirm,
  onCancel,
}) => {
  const [inputValue, setInputValue] = useState(initialValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setInputValue(initialValue);
      // Autofocus the input if it's a prompt
      setTimeout(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      }, 50);
    }
  }, [isOpen, initialValue]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (type === "prompt" && !inputValue.trim()) return;
    onConfirm(type === "prompt" ? inputValue : undefined);
  };

  return (
    <div className="custom-modal-overlay" onClick={onCancel}>
      <div className="custom-modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="custom-modal-header">
          <h3 className="custom-modal-title">{title}</h3>
          <button className="custom-modal-close" onClick={onCancel} aria-label="Close">
            &times;
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="custom-modal-body">
            <p className="custom-modal-message">{message}</p>
            {type === "prompt" && (
              <input
                ref={inputRef}
                type="text"
                className="custom-modal-input"
                placeholder={placeholder}
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
              />
            )}
          </div>
          <div className="custom-modal-footer">
            <button type="button" className="custom-modal-btn cancel" onClick={onCancel}>
              {cancelLabel}
            </button>
            <button
              type="submit"
              className={`custom-modal-btn confirm ${type === "confirm" ? "danger" : "primary"}`}
              disabled={type === "prompt" && !inputValue.trim()}
            >
              {confirmLabel}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
