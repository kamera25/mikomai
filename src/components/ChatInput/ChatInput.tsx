import React, { forwardRef, useRef, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { SuggestionsList } from "./SuggestionsList";
import { RefreshIcon, GearIcon, SendIcon, PaperclipIcon, CrossIcon, FileTextIcon } from "../Icons";
import { Attachment } from "../../types";
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
  handleSend: (text?: string, attachments?: Attachment[]) => void;
  handleLoadModel: () => void;
  setIsSettingsOpen: (value: boolean) => void;
  cursorPos: number;
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
      cursorPos,
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
    const [attachments, setAttachments] = useState<Attachment[]>([]);
    const [isDragging, setIsDragging] = useState(false);
    const fileInputRef = useRef<HTMLInputElement>(null);

    const handleFileAttach = (files: FileList | null) => {
      if (!files) return;
      Array.from(files).forEach((file) => {
        const isImage = file.type.startsWith("image/");
        const isText = file.type.startsWith("text/") || 
                       /\.(txt|md|json|csv|log|yaml|yml)$/i.test(file.name);
        
        if (!isImage && !isText) {
          return;
        }

        const reader = new FileReader();
        reader.onload = (e) => {
          const content = e.target?.result as string;
          if (content) {
            setAttachments((prev) => {
              if (prev.some((a) => a.name === file.name)) return prev;
              return [
                ...prev,
                {
                  name: file.name,
                  type: isImage ? "image" : "text",
                  content: content,
                },
              ];
            });
          }
        };

        if (isImage) {
          reader.readAsDataURL(file);
        } else {
          reader.readAsText(file);
        }
      });
    };

    const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (e.clipboardData.files.length > 0) {
        e.preventDefault();
        handleFileAttach(e.clipboardData.files);
      }
    };

    useEffect(() => {
      const preventDefault = (e: DragEvent) => {
        e.preventDefault();
      };
      window.addEventListener("dragover", preventDefault);
      window.addEventListener("drop", preventDefault);

      let unlistenDrop: (() => void) | undefined;
      let unlistenOver: (() => void) | undefined;
      let unlistenLeave: (() => void) | undefined;

      const setupTauriDnd = async () => {
        try {
          unlistenOver = await listen("tauri://drag-over", () => {
            setIsDragging(true);
          });
          unlistenLeave = await listen("tauri://drag-leave", () => {
            setIsDragging(false);
          });
          unlistenDrop = await listen<{ paths: string[] } | any>("tauri://drag-drop", async (event) => {
            setIsDragging(false);
            const payload = event.payload;
            const paths: string[] = payload?.paths || (Array.isArray(payload) ? payload : []);
            if (paths && paths.length > 0) {
              try {
                const newAtts = await invoke<Attachment[]>("read_files_as_attachments", { paths });
                if (newAtts && newAtts.length > 0) {
                  setAttachments((prev) => {
                    const existingNames = new Set(prev.map((a) => a.name));
                    const filtered = newAtts.filter((a) => !existingNames.has(a.name));
                    return [...prev, ...filtered];
                  });
                }
              } catch (err) {
                console.error("Failed to read dropped files as attachments:", err);
              }
            }
          });
        } catch (e) {
          console.warn("Tauri drag-drop listener setup failed or non-Tauri environment:", e);
        }
      };

      setupTauriDnd();

      return () => {
        window.removeEventListener("dragover", preventDefault);
        window.removeEventListener("drop", preventDefault);
        if (unlistenDrop) unlistenDrop();
        if (unlistenOver) unlistenOver();
        if (unlistenLeave) unlistenLeave();
      };
    }, []);

    const handleDragEnter = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = "copy";
      }
      setIsDragging(true);
    };

    const handleDragLeave = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
    };

    const handleDragOver = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = "copy";
      }
      setIsDragging(true);
    };

    const handleDrop = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
      if (e.dataTransfer && e.dataTransfer.files.length > 0) {
        handleFileAttach(e.dataTransfer.files);
      }
    };

    const onSend = () => {
      if (input.trim() || attachments.length > 0) {
        handleSend(input.trim(), attachments);
        setAttachments([]);
      }
    };

    useEffect(() => {
      if (!showSuggestions) return;

      const textBeforeCursor = input.slice(0, cursorPos);
      const atIndex = textBeforeCursor.lastIndexOf("@");

      if (atIndex !== -1) {
        const query = textBeforeCursor.slice(atIndex + 1);
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
          if (combined.length === 0) {
            setShowSuggestions(false);
            setSuggestionIndex(0);
          } else {
            setSuggestionIndex((prev) => Math.min(prev, combined.length - 1));
          }
        }
      }
    }, [availableHosts, recentIPs, showSuggestions, input, cursorPos, t, setFilteredSuggestions, setShowSuggestions, setSuggestionIndex]);

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
          onSend();
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
        <div
          className={`input-container ${isDragging ? "dragging" : ""}`}
          onDragEnter={handleDragEnter}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          <SuggestionsList
            showSuggestions={showSuggestions}
            filteredSuggestions={filteredSuggestions}
            suggestionIndex={suggestionIndex}
            handleSelectSuggestion={handleSelectSuggestion}
            suggestionListRef={suggestionListRef}
          />
          {attachments.length > 0 && (
            <div className="attachments-preview">
              {attachments.map((att, idx) => (
                <div key={idx} className="attachment-preview-item">
                  {att.type === "image" ? (
                    <img src={att.content} alt={att.name} className="attachment-thumb" />
                  ) : (
                    <div className="attachment-text-file">
                      <FileTextIcon size={16} />
                      <span className="attachment-file-name">{att.name}</span>
                    </div>
                  )}
                  <button
                    type="button"
                    className="remove-attachment-btn"
                    onClick={() => {
                      setAttachments((prev) => prev.filter((_, i) => i !== idx));
                    }}
                  >
                    <CrossIcon size={10} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <div className="input-wrapper">
            <input
              type="file"
              ref={fileInputRef}
              style={{ display: "none" }}
              multiple
              accept="image/*,text/*,.txt,.md,.json,.csv,.log,.yaml,.yml"
              onChange={(e) => handleFileAttach(e.target.files)}
            />
            <button
              type="button"
              className="attach-button"
              onClick={() => fileInputRef.current?.click()}
              disabled={modelStatus !== "Loaded"}
              title="ファイルを添付 (画像・テキスト)"
            >
              <PaperclipIcon size={16} />
            </button>
            <textarea
              ref={ref}
              className="chat-input"
              placeholder={
                modelStatus === "Loaded" ? t("chat_input.placeholder") : t("chat_input.waiting_model")
              }
              value={input}
              onChange={handleInputChange}
              onPaste={handlePaste}
              rows={1}
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
              onClick={onSend}
              disabled={modelStatus !== "Loaded" || (!input.trim() && attachments.length === 0)}
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
