import React, { RefObject } from "react";
import { isGlobalIP } from "../../utils/ipUtils";
import { MonitorIcon, GlobeIcon, SwitchIcon } from "../Icons";

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
    <div ref={suggestionListRef} className="suggestion-list" role="listbox">
      {filteredSuggestions.map((hostObj, idx) => (
        <div
          key={`${hostObj.hostname}-${hostObj.ip}`}
          className={`suggestion-item ${idx === suggestionIndex ? "selected" : ""}`}
          onClick={() => handleSelectSuggestion(hostObj)}
          role="option"
          aria-selected={idx === suggestionIndex}
          tabIndex={-1}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleSelectSuggestion(hostObj);
            }
          }}
        >
          {hostObj.hostname === "localhost" ? (
            <MonitorIcon size={14} style={{ marginRight: 8 }} />
          ) : hostObj.ip === "過去に投入したIPアドレス" && isGlobalIP(hostObj.hostname) ? (
            <GlobeIcon size={14} style={{ marginRight: 8 }} />
          ) : (
            <SwitchIcon size={14} style={{ marginRight: 8 }} />
          )}
          <span className="suggestion-hostname">{hostObj.hostname}</span>
          <span className="suggestion-ip">({hostObj.ip})</span>
        </div>
      ))}
    </div>
  );
};
