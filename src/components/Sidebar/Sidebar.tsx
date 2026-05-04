import React from 'react';
import { HistoryItem } from '../../types';

interface SidebarProps {
  isSidebarOpen: boolean;
  history: HistoryItem[];
  activeSessionId: string;
  createNewFolder: () => void;
  createNewSession: () => void;
  toggleFolder: (folderId: string) => void;
  switchSession: (sessionId: string) => void;
  formatDate?: (dateString: string) => string;
}

export const Sidebar: React.FC<SidebarProps> = ({
  isSidebarOpen,
  history,
  activeSessionId,
  createNewFolder,
  createNewSession,
  toggleFolder,
  switchSession
}) => {
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
        return (
          <div
            key={item.id}
            className={`session-item ${activeSessionId === item.id ? 'active' : ''}`}
            style={{ paddingLeft: `${level * 12 + 28}px` }}
            onClick={() => switchSession(item.id)}
          >
            <span className="session-title">{item.title}</span>
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
          <button className="icon-button" title="新規フォルダ" onClick={createNewFolder}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
          </button>
          <button className="icon-button" title="新規チャット" onClick={createNewSession}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          </button>
        </div>
      </div>

      <div className="session-list">
        {renderHistoryItems(history)}
      </div>
    </aside>
  );
};
