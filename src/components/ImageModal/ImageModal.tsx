import React, { useEffect } from "react";
import { CrossIcon } from "../Icons";
import "./ImageModal.css";

interface ImageModalProps {
  src: string;
  alt?: string;
  onClose: () => void;
}

export const ImageModal: React.FC<ImageModalProps> = ({ src, alt, onClose }) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="image-modal-overlay" onClick={onClose} data-testid="image-modal-overlay">
      <div className="image-modal-container" onClick={(e) => e.stopPropagation()}>
        <div className="image-modal-header">
          {alt && <div className="image-modal-title">{alt}</div>}
          <button
            type="button"
            className="image-modal-close-btn"
            onClick={onClose}
            aria-label="閉じる"
            title="閉じる"
            data-testid="image-modal-close-btn"
          >
            <CrossIcon size={18} />
          </button>
        </div>
        <img src={src} alt={alt || "拡大画像"} className="image-modal-img" />
      </div>
    </div>
  );
};
