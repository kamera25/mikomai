import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { useGuiEvent } from "../../gui/events";
import { SidebarIcon, ServerIcon, DiffIcon } from "../Icons";

interface ChatHeaderProps {
  isSidebarOpen: boolean;
  isConfigDiffOpen: boolean;
  isEditing: boolean;
  draftTitle: string;
  sessionTitle: string;
  hostLabel?: string;
}

export function ChatHeader({
  isSidebarOpen,
  isConfigDiffOpen,
  isEditing,
  draftTitle,
  sessionTitle,
  hostLabel,
}: ChatHeaderProps) {
  const { t } = useTranslation();
  const emit = useGuiEvent();
  const isComposing = useRef(false);

  return (
    <header className="chat-header">
      <div className="header-left">
        <button
          className="sidebar-toggle-button"
          onClick={() => emit({ type: "sidebar.toggle" })}
          title={isSidebarOpen ? t("app.sidebar_close") : t("app.sidebar_open")}
        >
          <SidebarIcon size={20} />
        </button>
        {isEditing ? (
          <input
            className="header-title-input"
            value={draftTitle}
            onChange={(event) => emit({ type: "header.change", title: event.target.value })}
            onBlur={() => emit({ type: "header.save" })}
            onCompositionStart={() => {
              isComposing.current = true;
            }}
            onCompositionEnd={() => {
              setTimeout(() => {
                isComposing.current = false;
              }, 150);
            }}
            onKeyDown={(event) => {
              if (isComposing.current || event.nativeEvent.isComposing || event.keyCode === 229)
                return;
              if (event.key === "Enter") emit({ type: "header.save" });
              else if (event.key === "Escape") emit({ type: "header.cancel" });
            }}
            autoFocus
          />
        ) : (
          <h1
            className="header-title clickable"
            onDoubleClick={() => emit({ type: "header.edit" })}
            title={t("app.double_click_rename")}
          >
            {sessionTitle}
          </h1>
        )}
        {hostLabel && (
          <div style={{ display: "flex", alignItems: "center" }}>
            <ServerIcon size={12} style={{ marginRight: "4px" }} />
            <span className="header-hostname">{hostLabel}</span>
          </div>
        )}
      </div>
      <div className="header-right">
        <button
          className={`sidebar-toggle-button ${isConfigDiffOpen ? "active" : ""}`}
          onClick={() => emit({ type: "diff.toggle" })}
          title={isConfigDiffOpen ? t("app.diff_close") : t("app.diff_open")}
        >
          <DiffIcon size={20} />
        </button>
      </div>
    </header>
  );
}
