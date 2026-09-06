import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ipc } from "../../platform";
import "./KeyringAccessModal.css";

export interface KeyringAccessModalProps {
  forceOpen?: boolean;
}

export const KeyringAccessModal: React.FC<KeyringAccessModalProps> = ({ forceOpen }) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState<boolean>(false);

  useEffect(() => {
    let unlistenStart: (() => void) | undefined;
    let unlistenEnd: (() => void) | undefined;
    let isMounted = true;

    const setupListeners = async () => {
      try {
        const startPromise = ipc.subscribe("keyring-access-start", () => {
          if (isMounted) setIsOpen(true);
        });
        const endPromise = ipc.subscribe("keyring-access-end", () => {
          if (isMounted) setIsOpen(false);
        });

        const [startUnsub, endUnsub] = await Promise.all([startPromise, endPromise]);
        if (isMounted) {
          unlistenStart = startUnsub;
          unlistenEnd = endUnsub;
        } else {
          startUnsub();
          endUnsub();
        }
      } catch (err) {
        console.error("Failed to setup keyring event listeners:", err);
      }
    };

    void setupListeners();

    return () => {
      isMounted = false;
      if (unlistenStart) unlistenStart();
      if (unlistenEnd) unlistenEnd();
    };
  }, []);

  const showModal = forceOpen !== undefined ? forceOpen : isOpen;

  if (!showModal) {
    return null;
  }

  return (
    <div
      className="keyring-modal-overlay"
      data-testid="keyring-access-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="keyring-modal-title"
    >
      <div className="keyring-modal-container">
        {/* Header Section */}
        <div className="keyring-modal-header">
          <div className="keyring-modal-icon-wrapper">
            <svg
              className="keyring-modal-icon"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
              <path d="M7 11V7a5 5 0 0 1 10 0v4" />
            </svg>
          </div>
          <div className="keyring-modal-title-group">
            <h2 id="keyring-modal-title" className="keyring-modal-title">
              {t("keyring_modal.title", "macOS キーチェーンへのアクセス")}
            </h2>
            <p className="keyring-modal-subtitle">
              {t("keyring_modal.subtitle", "認証情報の保護と暗号化処理")}
            </p>
          </div>
        </div>

        {/* Status Indicator */}
        <div className="keyring-modal-status-bar">
          <span className="keyring-modal-status-spinner" />
          <span className="keyring-modal-status-text">
            {t("keyring_modal.waiting_status", "システム認証ダイアログの応答を待機中...")}
          </span>
        </div>

        {/* Content Points */}
        <div className="keyring-modal-cards">
          {/* Item 1: Purpose */}
          <div className="keyring-modal-card">
            <div className="keyring-modal-card-icon purpose-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
              </svg>
            </div>
            <div className="keyring-modal-card-content">
              <h3 className="keyring-modal-card-title">
                {t("keyring_modal.section_purpose_title", "認証情報の安全な保護")}
              </h3>
              <p className="keyring-modal-card-desc">
                {t(
                  "keyring_modal.section_purpose_desc",
                  "mikomaiは、登録された接続先機器のパスワードや秘密鍵のパスフレーズを安全に暗号化・復号化するために、macOSのキーチェーン（キーリング）を利用しています。"
                )}
              </p>
            </div>
          </div>

          {/* Item 2: Password entry confirmation */}
          <div className="keyring-modal-card highlight-card">
            <div className="keyring-modal-card-icon password-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="7.5" cy="15.5" r="4.5" />
                <path d="m21 3-9.5 9.5" />
                <path d="m15.5 7.5 3 3" />
              </svg>
            </div>
            <div className="keyring-modal-card-content">
              <h3 className="keyring-modal-card-title">
                {t("keyring_modal.section_prompt_title", "パスワード入力のご案内")}
              </h3>
              <p className="keyring-modal-card-desc">
                {t(
                  "keyring_modal.section_prompt_desc",
                  "macOSのセキュリティ確認ダイアログが表示された場合は、内容をご確認の上、意図した操作であればMacのログインパスワード（キーチェーンパスワード）を入力し、「許可」または「常に許可」をクリックしてください。"
                )}
              </p>
            </div>
          </div>

          {/* Item 3: Security prompt on startup/access */}
          <div className="keyring-modal-card">
            <div className="keyring-modal-card-icon security-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div className="keyring-modal-card-content">
              <h3 className="keyring-modal-card-title">
                {t("keyring_modal.section_security_title", "起動時・アクセス時の確認について")}
              </h3>
              <p className="keyring-modal-card-desc">
                {t(
                  "keyring_modal.section_security_desc",
                  "macOSのセキュリティ保護機能により、安全なデータへのアクセス時やアプリケーション起動時にシステムから確認が求められます。"
                )}
              </p>
            </div>
          </div>
        </div>

        {/* Footer info note */}
        <div className="keyring-modal-footer">
          <p className="keyring-modal-footer-note">
            {t(
              "keyring_modal.footer_note",
              "※システムダイアログでの操作が完了すると、この画面は自動的に閉じます。"
            )}
          </p>
        </div>
      </div>
    </div>
  );
};
