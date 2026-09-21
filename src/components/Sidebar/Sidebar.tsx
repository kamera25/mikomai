import React, { useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { HistoryItem, Message } from "../../types";
import { dedupeTimelineMessages } from "../../features/chat/chatReducer";
import { useGuiEvent } from "../../gui/events";
import { UserIcon, BookIcon, TerminalIcon, MessageIcon, ChevronIcon, FolderIcon, MenuDotsIcon, PlusIcon } from "../Icons";
import "./Sidebar.css";

interface SidebarProps {
  isSidebarOpen: boolean;
  history: HistoryItem[];
  activeSessionId: string;
  messages: Message[];
  style?: React.CSSProperties;
  isResizing?: boolean;
}

export const Sidebar: React.FC<SidebarProps> = React.memo(({
  isSidebarOpen,
  history,
  activeSessionId,
  messages,
  style,
  isResizing,
}) => {
  const { t } = useTranslation();
  const emit = useGuiEvent();
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState<string>("");
  const isComposingSidebar = useRef(false);

  const handleSaveRename = (sessionId: string) => {
    if (editingTitle.trim()) {
      emit({ type: "session.rename", id: sessionId, title: editingTitle.trim() });
    }
    setEditingSessionId(null);
  };

  const renderSessionTimeline = () => {
    const timelineEvents = dedupeTimelineMessages(messages).filter((m) => !m.isHidden);
    if (timelineEvents.length === 0) return null;

    return (
      <div className="sidebar-timeline">
        <div className="timeline-items">
          {timelineEvents.map((m, i) => (
            <div
              key={`${m.task_id || "event"}-${i}`}
              className={`sidebar-timeline-item ${m.role} ${m.status?.toLowerCase() || ""} ${m.event_type?.toLowerCase() || ""}`}
              onClick={(e) => {
                e.stopPropagation();
                if (m.task_id) {
                  emit({ type: "timeline.scroll", taskId: m.task_id });
                }
              }}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  if (m.task_id) {
                    emit({ type: "timeline.scroll", taskId: m.task_id });
                  }
                }
              }}
            >
              <div className="sidebar-timeline-icon">
                {m.role === "user" ? (
                  <UserIcon size={12} strokeWidth={2.5} />
                ) : m.event_type === "ToolExecution" ? (
                  m.tool_id === "query_nw_db" || m.tool_id === "network_query_nw_db" ? (
                    <BookIcon size={12} strokeWidth={3} />
                  ) : (
                    <TerminalIcon size={12} strokeWidth={3} />
                  )
                ) : (
                  <MessageIcon size={12} strokeWidth={2.5} />
                )}
              </div>
              <div className="sidebar-timeline-content">
                <span className="sidebar-timeline-label">
                  {m.role === "user" ? (
                    m.content
                  ) : m.event_type === "ToolExecution" ? (
                    <>
                      <span className="timeline-action-name">{m.action_name}</span>
                      {m.args && (
                        <span className="sidebar-timeline-args">{JSON.stringify(m.args)}</span>
                      )}
                    </>
                  ) : (
                    m.summary_text ||
                    (m.content?.includes("agent-step") || m.content?.includes("agent-decision")
                      ? "エージェントによる解析を開始"
                      : m.content)
                  )}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  };

  const renderHistoryItems = (items: HistoryItem[], level = 0) => {
    return items.map((item) => {
      if (item.type === "folder") {
        return (
          <div key={item.id} className="folder-container">
            <div
              className="folder-item"
              style={{ paddingLeft: `${level * 12 + 12}px` }}
              onClick={() => emit({ type: "session.folder.toggle", id: item.id })}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  emit({ type: "session.folder.toggle", id: item.id });
                }
              }}
            >
              <div className="folder-icon">
                <ChevronIcon direction={item.isOpen ? "down" : "right"} size={12} strokeWidth={3} className={`chevron ${item.isOpen ? "open" : ""}`} />
              </div>
              <FolderIcon size={14} style={{ marginRight: 4, color: "var(--accent-color)" }} />
              <span className="folder-name">{item.name}</span>
            </div>
            {item.isOpen && renderHistoryItems(item.items, level + 1)}
          </div>
        );
      } else {
        const isActive = activeSessionId === item.id;
        const isMenuOpen = openMenuId === item.id;
        const isEditing = editingSessionId === item.id;
        return (
          <div key={item.id} className="session-container">
            <div
              className={`session-item ${isActive ? "active" : ""}`}
              style={{ paddingLeft: `${level * 12 + 28}px` }}
              onClick={() => {
                if (!isEditing) {
                  emit({ type: "session.select", id: item.id });
                }
              }}
              onDoubleClick={(e) => {
                e.stopPropagation();
                if (!isEditing) {
                  setEditingSessionId(item.id);
                  setEditingTitle(item.title);
                }
              }}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  if (!isEditing) {
                    emit({ type: "session.select", id: item.id });
                  }
                }
              }}
            >
              {isEditing ? (
                <input
                  className="session-title-input"
                  value={editingTitle}
                  onChange={(e) => setEditingTitle(e.target.value)}
                  onBlur={() => handleSaveRename(item.id)}
                  onCompositionStart={() => {
                    isComposingSidebar.current = true;
                  }}
                  onCompositionEnd={() => {
                    setTimeout(() => {
                      isComposingSidebar.current = false;
                    }, 150);
                  }}
                  onKeyDown={(e) => {
                    const isComp =
                      isComposingSidebar.current || e.nativeEvent.isComposing || e.keyCode === 229;
                    if (isComp) {
                      return;
                    }
                    if (e.key === "Enter") {
                      handleSaveRename(item.id);
                    } else if (e.key === "Escape") {
                      setEditingSessionId(null);
                    }
                  }}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <>
                  <span className="session-title">{item.title}</span>
                  <div
                    className={`session-actions ${isMenuOpen ? "menu-open" : ""}`}
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      className="session-action-trigger"
                      onClick={() => setOpenMenuId(isMenuOpen ? null : item.id)}
                      title={t("sidebar.menu_title")}
                    >
                      <MenuDotsIcon size={14} />
                    </button>
                    {isMenuOpen && (
                      <>
                        <div
                          className="session-menu-backdrop"
                          onClick={() => setOpenMenuId(null)}
                        />
                        <div className="session-menu">
                          <button
                            className="session-menu-item"
                            onClick={() => {
                              setEditingSessionId(item.id);
                              setEditingTitle(item.title);
                              setOpenMenuId(null);
                            }}
                          >
                            {t("common.rename")}
                          </button>
                          <button
                            className="session-menu-item delete"
                            onClick={() => {
                              emit({ type: "session.delete", id: item.id });
                              setOpenMenuId(null);
                            }}
                          >
                            {t("common.delete")}
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                </>
              )}
            </div>
            {isActive && renderSessionTimeline()}
          </div>
        );
      }
    });
  };

  return (
    <aside
      className={`sidebar ${isSidebarOpen ? "" : "collapsed"} ${isResizing ? "resizing" : ""}`}
      style={isSidebarOpen && style?.width !== undefined ? style : undefined}
    >
      <div className="sidebar-header">
        <h2>{t("sidebar.history_title")}</h2>
        <div className="header-actions">
          <button className="icon-button" title={t("sidebar.btn_new_chat")} onClick={() => emit({ type: "session.create" })}>
            <PlusIcon size={14} />
          </button>
        </div>
      </div>

      <div className="sidebar-scroll-area">
        <div className="session-list">{renderHistoryItems(history)}</div>
      </div>
    </aside>
  );
});

Sidebar.displayName = "Sidebar";
