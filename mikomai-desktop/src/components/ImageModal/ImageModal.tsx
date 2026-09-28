import React, { useEffect } from "react";
import { CrossIcon } from "../Icons";
import "./ImageModal.css";

interface ImageModalProps {
  src: string;
  alt?: string;
  onClose: () => void;
}

function useImageModalPresenter({ src, alt, onClose }: ImageModalProps) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return { src, alt, onClose };
}

function ImageModalView({ src, alt, onClose }: ReturnType<typeof useImageModalPresenter>) {
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
}

export const ImageModal: React.FC<ImageModalProps> = (props) => {
  return <ImageModalView {...useImageModalPresenter(props)} />;
};
