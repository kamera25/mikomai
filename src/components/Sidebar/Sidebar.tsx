import React, { useState, useRef } from 'react';
import { HistoryItem, Message } from '../../types';
import './Sidebar.css';

interface SidebarProps {
  isSidebarOpen: boolean;
  history: HistoryItem[];
  activeSessionId: string;
  messages: Message[];
  createNewFolder: () => void;
  createNewSession: () => void;
  toggleFolder: (folderId: string) => void;
  switchSession: (sessionId: string) => void;
  onTimelineItemClick?: (taskId: string) => void;
  formatDate?: (dateString: string) => string;
  renameSession: (sessionId: string, newTitle: string) => void;
  deleteSession: (sessionId: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  isSidebarOpen,
  history,
  activeSessionId,
  messages,
  createNewSession,
  toggleFolder,
  switchSession,
  onTimelineItemClick,
  renameSession,
  deleteSession
}) => {
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState<string>("");
  const isComposingSidebar = useRef(false);

  const handleSaveRename = (sessionId: string) => {
    if (editingTitle.trim()) {
      renameSession(sessionId, editingTitle.trim());
    }
    setEditingSessionId(null);
  };

  const renderSessionTimeline = () => {
    const timelineEvents = messages.filter(m => !m.isHidden);
    if (timelineEvents.length === 0) return null;

    return (
      <div className="sidebar-timeline">
        <div className="timeline-items">
          {timelineEvents.map((m, i) => (
            <div 
              key={m.task_id || i} 
              className={`sidebar-timeline-item ${m.role} ${m.status?.toLowerCase() || ""} ${m.event_type?.toLowerCase() || ""}`}
              onClick={(e) => {
                e.stopPropagation();
                if (m.task_id && onTimelineItemClick) {
                  onTimelineItemClick(m.task_id);
                }
              }}
            >
              <div className="sidebar-timeline-icon">
                {m.role === 'user' ? (
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path><circle cx="12" cy="7" r="4"></circle></svg>
                ) : m.event_type === 'ToolExecution' ? (
                  m.tool_id === 'query_nw_db' || m.tool_id === 'network_query_nw_db' ? (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1-2.5-2.5Z"></path><path d="M8 2v20"></path></svg>
                  ) : (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>
                  )
                ) : (
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
                )}
              </div>
              <div className="sidebar-timeline-content">
                <span className="sidebar-timeline-label">
                  {m.role === 'user' ? (
                    m.content
                  ) : m.event_type === 'ToolExecution' ? (
                    <>
                      <span className="timeline-action-name">{m.action_name}</span>
                      {m.args && <span className="sidebar-timeline-args">{JSON.stringify(m.args)}</span>}
                    </>
                  ) : (
                    m.summary_text || m.content
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
    return items.map(item => {
      if (item.type === 'folder') {
        return (
          <div key={item.id} className="folder-container">
            <div
              className="folder-item"
              style={{ paddingLeft: `${level * 12 + 12}px` }}
              onClick={() => toggleFolder(item.id)}
            >
              <div className="folder-icon">
                <svg className={`chevron ${item.isOpen ? 'open' : ''}`} width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
              </div>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ marginRight: 4, color: 'var(--accent-color)' }}><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
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
              className={`session-item ${isActive ? 'active' : ''}`}
              style={{ paddingLeft: `${level * 12 + 28}px` }}
              onClick={() => {
                if (!isEditing) {
                  switchSession(item.id);
                }
              }}
              onDoubleClick={(e) => {
                e.stopPropagation();
                if (!isEditing) {
                  setEditingSessionId(item.id);
                  setEditingTitle(item.title);
                }
              }}
            >
              {isEditing ? (
                <input
                  className="session-title-input"
                  value={editingTitle}
                  onChange={(e) => setEditingTitle(e.target.value)}
                  onBlur={() => handleSaveRename(item.id)}
                  onCompositionStart={() => { isComposingSidebar.current = true; }}
                  onCompositionEnd={() => {
                    setTimeout(() => { isComposingSidebar.current = false; }, 150);
                  }}
                  onKeyDown={(e) => {
                    const isComp = isComposingSidebar.current || e.nativeEvent.isComposing || e.keyCode === 229;
                    if (isComp) {
                      return;
                    }
                    if (e.key === 'Enter') {
                      handleSaveRename(item.id);
                    } else if (e.key === 'Escape') {
                      setEditingSessionId(null);
                    }
                  }}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <>
                  <span className="session-title">{item.title}</span>
                  <div className={`session-actions ${isMenuOpen ? 'menu-open' : ''}`} onClick={(e) => e.stopPropagation()}>
                    <button
                      className="session-action-trigger"
                      onClick={() => setOpenMenuId(isMenuOpen ? null : item.id)}
                      title="メニュー"
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="1.5"></circle><circle cx="6" cy="12" r="1.5"></circle><circle cx="18" cy="12" r="1.5"></circle></svg>
                    </button>
                    {isMenuOpen && (
                      <>
                        <div className="session-menu-backdrop" onClick={() => setOpenMenuId(null)} />
                        <div className="session-menu">
                          <button
                            className="session-menu-item"
                            onClick={() => {
                              setEditingSessionId(item.id);
                              setEditingTitle(item.title);
                              setOpenMenuId(null);
                            }}
                          >
                            リネーム
                          </button>
                          <button
                            className="session-menu-item delete"
                            onClick={() => {
                              deleteSession(item.id);
                              setOpenMenuId(null);
                            }}
                          >
                            削除
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
    <aside className={`sidebar ${isSidebarOpen ? '' : 'collapsed'}`}>
      <div className="sidebar-header">
        <h2>履歴</h2>
        <div className="header-actions">
          <button className="icon-button" title="新規チャット" onClick={createNewSession}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          </button>
        </div>
      </div>

      <div className="sidebar-scroll-area">
        <div className="session-list">
          {renderHistoryItems(history)}
        </div>
      </div>
    </aside>
  );
};
