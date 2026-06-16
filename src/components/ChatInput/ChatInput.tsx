import React, { forwardRef, useRef } from "react";
import { useTranslation } from "react-i18next";
import { SuggestionsList } from "./SuggestionsList";
import { RefreshIcon, GearIcon, SendIcon } from "../Icons";
import "./ChatInput.css";

interface ChatInputProps {
  modelStatus: string;
  modelPath: string | null;
  input: string;
  setInput: (value: string) => void;
  showSuggestions: boolean;
  setShowSuggestions: (value: boolean) => void;
  filteredSuggestions: { hostname: string; ip: string }[];
  suggestionIndex: number;
  setSuggestionIndex: React.Dispatch<React.SetStateAction<number>>;
  handleSelectSuggestion: (host: { hostname: string; ip: string }) => void;
  handleSend: () => void;
  handleLoadModel: () => void;
  setIsSettingsOpen: (value: boolean) => void;
  setCursorPos: (value: number) => void;
  availableHosts: { hostname: string; ip: string }[];
  recentIPs: string[];
  setFilteredSuggestions: (value: { hostname: string; ip: string }[]) => void;
}

export const ChatInput = forwardRef<HTMLTextAreaElement, ChatInputProps>(
  (
    {
      modelStatus,
      modelPath,
      input,
      setInput,
      showSuggestions,
      setShowSuggestions,
      filteredSuggestions,
      suggestionIndex,
      setSuggestionIndex,
      handleSelectSuggestion,
      handleSend,
      handleLoadModel,
      setIsSettingsOpen,
      setCursorPos,
      availableHosts,
      recentIPs,
      setFilteredSuggestions,
    },
    ref
  ) => {
    const { t } = useTranslation();
    const isComposing = useRef(false);
    const suggestionListRef = useRef<HTMLDivElement>(null);

    const handleInputKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      const isComp = isComposing.current || e.nativeEvent.isComposing || e.keyCode === 229;
      if (isComp) {
        return;
      }

      if (showSuggestions && filteredSuggestions.length > 0) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          setSuggestionIndex((prev) => {
            const next = (prev + 1) % filteredSuggestions.length;
            // Scroll into view logic
            const items = suggestionListRef.current?.querySelectorAll(".suggestion-item");
            if (items && items[next]) {
              (items[next] as HTMLElement).scrollIntoView({ block: "nearest" });
            }
            return next;
          });
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          setSuggestionIndex((prev) => {
            const next = (prev - 1 + filteredSuggestions.length) % filteredSuggestions.length;
            // Scroll into view logic
            const items = suggestionListRef.current?.querySelectorAll(".suggestion-item");
            if (items && items[next]) {
              (items[next] as HTMLElement).scrollIntoView({ block: "nearest" });
            }
            return next;
          });
          return;
        }
        if (e.key === "Enter" || e.key === "Tab") {
          e.preventDefault();
          handleSelectSuggestion(filteredSuggestions[suggestionIndex]);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          setShowSuggestions(false);
          return;
        }
      }

      if (e.key === "Enter") {
        if (!e.shiftKey && modelStatus === "Loaded") {
          e.preventDefault();
          handleSend();
        }
      }
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      const pos = e.target.selectionStart;
      setInput(newValue);
      setCursorPos(pos);

      // Detect @
      const textBeforeCursor = newValue.slice(0, pos);
      const atIndex = textBeforeCursor.lastIndexOf("@");

      if (atIndex !== -1) {
        const query = textBeforeCursor.slice(atIndex + 1);
        // Check if there's space between @ and cursor
        if (!query.includes(" ")) {
          const queryLower = query.toLowerCase();
          const combined: { hostname: string; ip: string }[] = [];
          const seenIPs = new Set<string>();

          // localhost
          if ("localhost".includes(queryLower) || t("chat_input.localhost").includes(query)) {
            combined.push({ hostname: "localhost", ip: t("chat_input.localhost") });
            seenIPs.add("127.0.0.1");
            seenIPs.add("localhost");
          }

          // Available hosts
          availableHosts.forEach((h) => {
            if (h.hostname !== "localhost") {
              if (h.hostname.toLowerCase().includes(queryLower) || h.ip.includes(query)) {
                combined.push(h);
              }
            }
            seenIPs.add(h.ip);
          });

          // Recent IPs
          recentIPs.forEach((ip) => {
            if (
              ip.toLowerCase().includes(queryLower) ||
              t("chat_input.past_ips").includes(query)
            ) {
              if (!seenIPs.has(ip)) {
                combined.push({ hostname: ip, ip: t("chat_input.past_ips") });
                seenIPs.add(ip);
              }
            }
          });

          setFilteredSuggestions(combined);
          setShowSuggestions(true);
          setSuggestionIndex(0);
        } else {
          setShowSuggestions(false);
        }
      } else {
        setShowSuggestions(false);
      }
    };

    return (
      <div className="input-area">
        {modelStatus !== "Loaded" && (
          <div className={`model-status-banner ${modelStatus.toLowerCase()}`}>
            {modelStatus === "Loading" && <div className="status-spinner"></div>}
            <span>
              {modelStatus === "NotLoaded" &&
                t("chat_input.error_no_model")}
              {modelStatus === "Loading" && t("chat_input.status_loading_model")}
              {modelStatus === "Error" &&
                t("chat_input.status_failed_model")}
            </span>
            {(modelStatus === "NotLoaded" || modelStatus === "Error") && (
              <div className="banner-actions">
                {modelPath && (
                  <button className="banner-button primary" onClick={handleLoadModel}>
                    <RefreshIcon size={14} style={{ marginRight: "6px" }} />
                    {t("chat_input.btn_load_model")}
                  </button>
                )}
                <button className="banner-button" onClick={() => setIsSettingsOpen(true)}>
                  <GearIcon size={14} style={{ marginRight: "6px" }} />
                  {t("chat_input.btn_settings")}
                </button>
              </div>
            )}
          </div>
        )}
        <div className={`input-container ${modelStatus !== "Loaded" ? "disabled" : ""}`}>
          <SuggestionsList
            showSuggestions={showSuggestions}
            filteredSuggestions={filteredSuggestions}
            suggestionIndex={suggestionIndex}
            handleSelectSuggestion={handleSelectSuggestion}
            suggestionListRef={suggestionListRef}
          />
          <div className="input-wrapper">
            <textarea
              ref={ref}
              className="chat-input"
              placeholder={
                modelStatus === "Loaded" ? t("chat_input.placeholder") : t("chat_input.waiting_model")
              }
              value={input}
              onChange={handleInputChange}
              rows={1}
              disabled={modelStatus !== "Loaded"}
              onCompositionStart={() => {
                isComposing.current = true;
              }}
              onCompositionEnd={() => {
                setTimeout(() => {
                  isComposing.current = false;
                }, 150);
              }}
              onKeyDown={handleInputKeyDown}
            />
            <button
              className="send-button"
              onClick={handleSend}
              disabled={modelStatus !== "Loaded" || !input.trim()}
            >
              <SendIcon size={16} />
            </button>
          </div>
        </div>
      </div>
    );
  }
);

ChatInput.displayName = "ChatInput";
