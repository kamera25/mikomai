import React, { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { CutIcon, CopyIcon, PasteIcon, SelectAllIcon, RefreshIcon } from "../Icons";
import "./ContextMenu.css";

interface MenuPosition {
  x: number;
  y: number;
}

interface TargetInfo {
  isInputOrTextarea: boolean;
  hasSelection: boolean;
  selectedText: string;
  isEditable: boolean;
  targetElement: HTMLElement | null;
}

export const ContextMenu: React.FC = () => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState<MenuPosition>({ x: 0, y: 0 });
  const [targetInfo, setTargetInfo] = useState<TargetInfo>({
    isInputOrTextarea: false,
    hasSelection: false,
    selectedText: "",
    isEditable: false,
    targetElement: null,
  });

  const menuRef = useRef<HTMLDivElement>(null);
  const isMac =
    typeof navigator !== "undefined" &&
    /Macintosh|Mac OS X/i.test(navigator.userAgent);
  const modKey = isMac ? "⌘" : "Ctrl+";

  const closeMenu = useCallback(() => {
    setIsOpen(false);
  }, []);

  const handleContextMenu = useCallback((e: MouseEvent) => {
    e.preventDefault();

    const target = e.target as HTMLElement | null;
    const isInputOrTextarea =
      target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
    const isContentEditable = target?.isContentEditable ?? false;
    const isEditable =
      (isInputOrTextarea &&
        !(target as HTMLInputElement | HTMLTextAreaElement).readOnly &&
        !(target as HTMLInputElement | HTMLTextAreaElement).disabled) ||
      isContentEditable;

    let selectedText = "";
    let hasSelection = false;

    if (isInputOrTextarea) {
      const inputEl = target as HTMLInputElement | HTMLTextAreaElement;
      const start = inputEl.selectionStart ?? 0;
      const end = inputEl.selectionEnd ?? 0;
      if (start !== end) {
        hasSelection = true;
        selectedText = inputEl.value.substring(start, end);
      }
    } else {
      const selection = window.getSelection();
      if (selection && selection.toString().length > 0) {
        hasSelection = true;
        selectedText = selection.toString();
      }
    }

    setTargetInfo({
      isInputOrTextarea,
      hasSelection,
      selectedText,
      isEditable,
      targetElement: target,
    });

    // Calculate smart positioning within viewport
    const menuWidth = 190;
    const menuHeight = 210;
    const windowWidth = window.innerWidth;
    const windowHeight = window.innerHeight;

    let x = e.clientX;
    let y = e.clientY;

    if (x + menuWidth > windowWidth) {
      x = Math.max(10, windowWidth - menuWidth - 10);
    }
    if (y + menuHeight > windowHeight) {
      y = Math.max(10, windowHeight - menuHeight - 10);
    }

    setPosition({ x, y });
    setIsOpen(true);
  }, []);

  useEffect(() => {
    window.addEventListener("contextmenu", handleContextMenu);

    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        closeMenu();
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        closeMenu();
      }
    };

    const handleScrollOrResize = () => {
      closeMenu();
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("resize", handleScrollOrResize);
    window.addEventListener("scroll", handleScrollOrResize, true);

    return () => {
      window.removeEventListener("contextmenu", handleContextMenu);
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("resize", handleScrollOrResize);
      window.removeEventListener("scroll", handleScrollOrResize, true);
    };
  }, [handleContextMenu, closeMenu]);

  const handleCut = async () => {
    closeMenu();
    const target = targetInfo.targetElement;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      const start = target.selectionStart ?? 0;
      const end = target.selectionEnd ?? 0;
      const val = target.value;
      const textToCut = val.substring(start, end);
      if (textToCut) {
        try {
          await navigator.clipboard.writeText(textToCut);
        } catch {
          document.execCommand("cut");
        }
        target.value = val.substring(0, start) + val.substring(end);
        target.setSelectionRange(start, start);
        target.dispatchEvent(new Event("input", { bubbles: true }));
      }
    } else {
      document.execCommand("cut");
    }
  };

  const handleCopy = async () => {
    closeMenu();
    if (targetInfo.selectedText) {
      try {
        await navigator.clipboard.writeText(targetInfo.selectedText);
      } catch {
        document.execCommand("copy");
      }
    } else {
      document.execCommand("copy");
    }
  };

  const handlePaste = async () => {
    closeMenu();
    const target = targetInfo.targetElement;
    try {
      const text = await navigator.clipboard.readText();
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
        const start = target.selectionStart ?? target.value.length;
        const end = target.selectionEnd ?? target.value.length;
        const val = target.value;
        target.value = val.substring(0, start) + text + val.substring(end);
        const newCursorPos = start + text.length;
        target.setSelectionRange(newCursorPos, newCursorPos);
        target.dispatchEvent(new Event("input", { bubbles: true }));
        target.focus();
      } else {
        document.execCommand("paste");
      }
    } catch {
      document.execCommand("paste");
    }
  };

  const handleSelectAll = () => {
    closeMenu();
    const target = targetInfo.targetElement;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      target.focus();
      target.select();
    } else {
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(document.body);
      selection?.removeAllRanges();
      selection?.addRange(range);
    }
  };

  const handleReload = () => {
    closeMenu();
    window.location.reload();
  };

  if (!isOpen) return null;

  const canCut = targetInfo.isEditable && targetInfo.hasSelection;
  const canCopy = targetInfo.hasSelection;
  const canPaste = targetInfo.isEditable;

  return (
    <div
      ref={menuRef}
      className="custom-context-menu"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
      }}
      role="menu"
      aria-label="Context Menu"
    >
      <button
        className="custom-context-menu-item"
        onClick={handleCut}
        disabled={!canCut}
        role="menuitem"
      >
        <span className="custom-context-menu-item-content">
          <span className="custom-context-menu-item-icon">
            <CutIcon size={14} />
          </span>
          <span>{t("context_menu.cut")}</span>
        </span>
        <span className="custom-context-menu-shortcut">{modKey}X</span>
      </button>

      <button
        className="custom-context-menu-item"
        onClick={handleCopy}
        disabled={!canCopy}
        role="menuitem"
      >
        <span className="custom-context-menu-item-content">
          <span className="custom-context-menu-item-icon">
            <CopyIcon size={14} />
          </span>
          <span>{t("context_menu.copy")}</span>
        </span>
        <span className="custom-context-menu-shortcut">{modKey}C</span>
      </button>

      <button
        className="custom-context-menu-item"
        onClick={handlePaste}
        disabled={!canPaste}
        role="menuitem"
      >
        <span className="custom-context-menu-item-content">
          <span className="custom-context-menu-item-icon">
            <PasteIcon size={14} />
          </span>
          <span>{t("context_menu.paste")}</span>
        </span>
        <span className="custom-context-menu-shortcut">{modKey}V</span>
      </button>

      <div className="custom-context-menu-divider" />

      <button
        className="custom-context-menu-item"
        onClick={handleSelectAll}
        role="menuitem"
      >
        <span className="custom-context-menu-item-content">
          <span className="custom-context-menu-item-icon">
            <SelectAllIcon size={14} />
          </span>
          <span>{t("context_menu.select_all")}</span>
        </span>
        <span className="custom-context-menu-shortcut">{modKey}A</span>
      </button>

      <div className="custom-context-menu-divider" />

      <button
        className="custom-context-menu-item"
        onClick={handleReload}
        role="menuitem"
      >
        <span className="custom-context-menu-item-content">
          <span className="custom-context-menu-item-icon">
            <RefreshIcon size={14} />
          </span>
          <span>{t("context_menu.reload")}</span>
        </span>
        <span className="custom-context-menu-shortcut">{modKey}R</span>
      </button>
    </div>
  );
};
