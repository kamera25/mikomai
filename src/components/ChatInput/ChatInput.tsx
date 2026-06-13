import React, { forwardRef, useRef } from 'react';
import { isGlobalIP } from '../../utils/ipUtils';

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

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (showSuggestions && filteredSuggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSuggestionIndex(prev => {
          const next = (prev + 1) % filteredSuggestions.length;
          // Scroll into view logic
          const list = document.querySelector('.suggestion-list');
          const items = list?.querySelectorAll('.suggestion-item');
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
          const list = document.querySelector('.suggestion-list');
          const items = list?.querySelectorAll('.suggestion-item');
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
      if (isComposing.current || (e.nativeEvent as any).isComposing || e.keyCode === 229) {
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
        // Combine available hosts and recent IPs
        const combined = [{ hostname: "localhost", ip: "このコンピュータ" }];
        availableHosts.forEach(h => {
          if (h.hostname !== "localhost") {
            combined.push(h);
          }
        });
        recentIPs.forEach(ip => {
          if (!combined.some(h => h.ip === ip)) {
            combined.push({ hostname: `${ip}`, ip: "過去に投入したIPアドレス" });
          }
        });

        const filtered = combined.filter(h =>
          h.hostname.toLowerCase().includes(query.toLowerCase()) ||
          h.ip.includes(query)
        );
        setFilteredSuggestions(filtered);
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
        {showSuggestions && filteredSuggestions.length > 0 && (
          <div className="suggestion-list">
            {filteredSuggestions.map((hostObj, idx) => (
              <div
                key={`${hostObj.hostname}-${hostObj.ip}`}
                className={`suggestion-item ${idx === suggestionIndex ? 'selected' : ''}`}
                onClick={() => handleSelectSuggestion(hostObj)}
              >
                {hostObj.hostname === 'localhost' ? (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{marginRight: 8}}>
                    <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                    <line x1="8" y1="21" x2="16" y2="21"></line>
                    <line x1="12" y1="17" x2="12" y2="21"></line>
                  </svg>
                ) : hostObj.ip === "過去に投入したIPアドレス" && isGlobalIP(hostObj.hostname) ? (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{marginRight: 8}}>
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="2" y1="12" x2="22" y2="12"></line>
                    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
                  </svg>
                ) : (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{marginRight: 8}}>
                    <polygon points="2 10 5 6 19 6 22 10"></polygon>
                    <rect x="2" y="10" width="20" height="9" rx="1"></rect>
                    <line x1="6" y1="14" x2="6.01" y2="14"></line>
                    <line x1="9" y1="14" x2="9.01" y2="14"></line>
                    <line x1="12" y1="14" x2="12.01" y2="14"></line>
                    <line x1="15" y1="14" x2="15.01" y2="14"></line>
                    <line x1="18" y1="13" x2="18.01" y2="13"></line>
                    <line x1="18" y1="16" x2="18.01" y2="16"></line>
                  </svg>
                )}
                <span className="suggestion-hostname">{hostObj.hostname}</span>
                <span className="suggestion-ip">({hostObj.ip})</span>
              </div>
            ))}
          </div>
        )}
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
