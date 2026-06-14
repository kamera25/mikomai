import React, { RefObject } from 'react';
import { isGlobalIP } from '../../utils/ipUtils';

interface HostSuggestion {
  hostname: string;
  ip: string;
}

interface SuggestionsListProps {
  showSuggestions: boolean;
  filteredSuggestions: HostSuggestion[];
  suggestionIndex: number;
  handleSelectSuggestion: (host: HostSuggestion) => void;
  suggestionListRef: RefObject<HTMLDivElement | null>;
}

export const SuggestionsList: React.FC<SuggestionsListProps> = ({
  showSuggestions,
  filteredSuggestions,
  suggestionIndex,
  handleSelectSuggestion,
  suggestionListRef,
}) => {
  if (!showSuggestions || filteredSuggestions.length === 0) return null;

  return (
    <div ref={suggestionListRef} className="suggestion-list">
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
  );
};
