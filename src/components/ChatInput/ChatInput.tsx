import React, { forwardRef, useRef } from 'react';
import { SuggestionsList } from './SuggestionsList';
import './ChatInput.css';

interface ChatInputProps {
  modelStatus: string;
  modelPath: string | null;
  input: string;
  setInput: (value: string) => void;
  showSuggestions: boolean;
  setShowSuggestions: (value: boolean) => void;
  filteredSuggestions: {hostname: string, ip: string}[];
  suggestionIndex: number;
  setSuggestionIndex: React.Dispatch<React.SetStateAction<number>>;
  handleSelectSuggestion: (host: {hostname: string, ip: string}) => void;
  handleSend: () => void;
  handleLoadModel: () => void;
  setIsSettingsOpen: (value: boolean) => void;
  setCursorPos: (value: number) => void;
  availableHosts: {hostname: string, ip: string}[];
  recentIPs: string[];
  setFilteredSuggestions: (value: {hostname: string, ip: string}[]) => void;
}

export const ChatInput = forwardRef<HTMLTextAreaElement, ChatInputProps>(({
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
  setFilteredSuggestions
}, ref) => {

  const isComposing = useRef(false);
  const suggestionListRef = useRef<HTMLDivElement>(null);

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const isComp = isComposing.current || e.nativeEvent.isComposing || e.keyCode === 229;
    if (isComp) {
      return;
    }

    if (showSuggestions && filteredSuggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSuggestionIndex(prev => {
          const next = (prev + 1) % filteredSuggestions.length;
          // Scroll into view logic
          const items = suggestionListRef.current?.querySelectorAll('.suggestion-item');
          if (items && items[next]) {
            (items[next] as HTMLElement).scrollIntoView({ block: 'nearest' });
          }
          return next;
        });
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSuggestionIndex(prev => {
          const next = (prev - 1 + filteredSuggestions.length) % filteredSuggestions.length;
          // Scroll into view logic
          const items = suggestionListRef.current?.querySelectorAll('.suggestion-item');
          if (items && items[next]) {
            (items[next] as HTMLElement).scrollIntoView({ block: 'nearest' });
          }
          return next;
        });
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        handleSelectSuggestion(filteredSuggestions[suggestionIndex]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setShowSuggestions(false);
        return;
      }
    }

    if (e.key === 'Enter') {
      if (isComp) {
        return;
      }
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
    const atIndex = textBeforeCursor.lastIndexOf('@');

    if (atIndex !== -1) {
      const query = textBeforeCursor.slice(atIndex + 1);
      // Check if there's space between @ and cursor
      if (!query.includes(' ')) {
        const queryLower = query.toLowerCase();
        const combined: { hostname: string; ip: string }[] = [];
        const seenIPs = new Set<string>();

        // localhost
        if (
          "localhost".includes(queryLower) ||
          "このコンピュータ".includes(query)
        ) {
          combined.push({ hostname: "localhost", ip: "このコンピュータ" });
          seenIPs.add("127.0.0.1");
          seenIPs.add("localhost");
        }

        // Available hosts
        availableHosts.forEach(h => {
          if (h.hostname !== "localhost") {
            if (
              h.hostname.toLowerCase().includes(queryLower) ||
              h.ip.includes(query)
            ) {
              combined.push(h);
            }
          }
          seenIPs.add(h.ip);
        });

        // Recent IPs
        recentIPs.forEach(ip => {
          if (
            ip.toLowerCase().includes(queryLower) ||
            "過去に投入したIPアドレス".includes(query)
          ) {
            if (!seenIPs.has(ip)) {
              combined.push({ hostname: ip, ip: "過去に投入したIPアドレス" });
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
            {modelStatus === "NotLoaded" && "AIモデルが読み込まれていません。モデルを読み込んでください。"}
            {modelStatus === "Loading" && "AIモデルを読み込み中です。しばらくお待ちください..."}
            {modelStatus === "Error" && "AIモデルの読み込みに失敗しました。設定を確認してください。"}
          </span>
          {(modelStatus === "NotLoaded" || modelStatus === "Error") && (
            <div className="banner-actions">
              {modelPath && (
                <button className="banner-button primary" onClick={handleLoadModel}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: '6px' }}><polyline points="1 4 1 10 7 10"></polyline><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path></svg>
                  モデルの読み込み
                </button>
              )}
              <button className="banner-button" onClick={() => setIsSettingsOpen(true)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: '6px' }}><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                設定
              </button>
            </div>
          )}
        </div>
      )}
      <div className={`input-container ${modelStatus !== "Loaded" ? 'disabled' : ''}`}>
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
            placeholder={modelStatus === "Loaded" ? "mikomaiに質問する..." : "モデルの準備を待っています..."}
            value={input}
            onChange={handleInputChange}
            rows={1}
            disabled={modelStatus !== "Loaded"}
            onCompositionStart={() => { isComposing.current = true; }}
            onCompositionEnd={() => {
              setTimeout(() => { isComposing.current = false; }, 150);
            }}
            onKeyDown={handleInputKeyDown}
          />
          <button
            className="send-button"
            onClick={handleSend}
            disabled={modelStatus !== "Loaded" || !input.trim()}
          >
            <svg viewBox="0 0 24 24">
              <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"></path>
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
});

ChatInput.displayName = 'ChatInput';
